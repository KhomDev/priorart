//! Citation anchor. Opens the target tree and checks that a cited source file,
//! line, and function exist.
//!
//! This is the first check in this tool that reads the target. It does not
//! decide that the bug is real. A citation that does not resolve cannot score
//! as if it had been checked, and it cannot come back CLEAR.
//!
//! Limits, on purpose:
//!
//! * Fenced code blocks are not read. A path inside a PoC command is the
//!   harness, not the claim.
//! * Paths under test, script, lib, or dependency directories are not
//!   anchored. Those are the PoC's own files.
//! * A quoted snippet is not compared byte for byte, and a function is not
//!   proved reachable. The gate says the location exists at the pin.
//! * Without `--target` nothing is opened. Unknown is not a pass, and the
//!   prose score is unchanged because there is nothing to contradict it.
//! * Without `--commit` a location that exists in the working tree is still
//!   not a pass. Unpinned is not the audited revision.

use crate::model::{Evidence, Finding, GateStatus, Verdict};
use regex::Regex;
use std::path::{Component, Path};
use std::process::Command;
use std::sync::LazyLock;

/// Rule id of the hard finding a bad citation produces. Not a claim heuristic:
/// funnel mode still kills it.
pub const CITE_RULE: &str = "CITE";

const CITE_WEIGHT: u32 = 40;

/// How many bytes of one file are read. A cited line past this is a failure
/// to anchor, not a pass.
const FILE_CAP: usize = 2_000_000;

const SOURCE_EXTS: &[&str] = &[
    "sol", "vy", "move", "rs", "cairo", "go", "ts", "js", "yul", "huff", "py", "tact", "fc",
    "func", "tolk", "sway", "clar", "cpp", "cc", "hpp", "c", "h",
];

const HARNESS_DIRS: &[&str] = &[
    "test",
    "tests",
    "script",
    "scripts",
    "lib",
    "node_modules",
    "out",
    "cache",
    "artifacts",
    "broadcast",
    "dependencies",
    "deps",
];

const SKIP_DIRS: &[&str] = &[
    ".git",
    ".hg",
    ".svn",
    "node_modules",
    "target",
    "out",
    "cache",
    "dist",
    "build",
];

static URLS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b[a-z][a-z0-9+.-]*://\S+").expect("url pattern"));

static PATH_CITE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?P<path>(?:[A-Za-z0-9_.\-]+/)*[A-Za-z0-9_\-][A-Za-z0-9_.\-]*\.(?P<ext>sol|vy|move|rs|cairo|go|ts|js|yul|huff|py|tact|fc|func|tolk|sway|clar|cpp|cc|hpp|c|h))\b(?::L?(?P<line>\d{1,7})(?:\s*-\s*L?(?P<end>\d{1,7}))?|#L(?P<hline>\d{1,7})(?:-L(?P<hend>\d{1,7}))?)?",
    )
    .expect("path citation")
});

static PROSE_LINE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\blines?\s+(\d{1,7})(?:\s*[-–]\s*(\d{1,7}))?").expect("prose line")
});

static FUNCTION_WORD: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b(?:function|fn|def|fun)\s+([A-Za-z_][A-Za-z0-9_]*)").expect("function word")
});

static FUNCTION_CALL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"`([A-Za-z_][A-Za-z0-9_]*)\(\)`").expect("function call"));

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Citation {
    pub path: String,
    pub line: Option<u32>,
    pub line_end: Option<u32>,
    pub function: Option<String>,
}

#[derive(Debug)]
pub struct AnchorAssessment {
    pub status: GateStatus,
    pub detail: String,
    pub finding: Option<Finding>,
}

struct LoadedFile {
    /// Repo-relative path that satisfied the citation.
    rel: String,
    text: String,
}

/// Citations in the prose, fenced blocks removed, harness paths removed.
pub fn extract_citations(text: &str) -> Vec<Citation> {
    let prose = outside_fences(text);
    let prose = URLS.replace_all(&prose, " ");
    let mut out = Vec::new();
    for sentence in split_rough(&prose) {
        let mut found: Vec<Citation> = Vec::new();
        for caps in PATH_CITE.captures_iter(&sentence) {
            let path = caps.name("path").unwrap().as_str().replace('\\', "/");
            let path = path.trim_start_matches("./").to_string();
            if !path_is_anchorable(&path) {
                continue;
            }
            let ext = caps.name("ext").unwrap().as_str();
            if !SOURCE_EXTS.contains(&ext) {
                continue;
            }
            let line = caps
                .name("line")
                .or_else(|| caps.name("hline"))
                .and_then(|m| m.as_str().parse().ok());
            let line_end = caps
                .name("end")
                .or_else(|| caps.name("hend"))
                .and_then(|m| m.as_str().parse().ok());
            found.push(Citation {
                path,
                line,
                line_end,
                function: None,
            });
        }
        if found.is_empty() {
            continue;
        }
        let (prose_line, prose_end) = PROSE_LINE
            .captures(&sentence)
            .map(|c| {
                (
                    c.get(1).and_then(|m| m.as_str().parse().ok()),
                    c.get(2).and_then(|m| m.as_str().parse().ok()),
                )
            })
            .unwrap_or((None, None));
        let function = FUNCTION_WORD
            .captures(&sentence)
            .or_else(|| FUNCTION_CALL.captures(&sentence))
            .map(|c| c.get(1).unwrap().as_str().to_string());
        for cite in &mut found {
            if cite.line.is_none() {
                cite.line = prose_line;
                cite.line_end = prose_end;
            }
            if cite.function.is_none() {
                cite.function = function.clone();
            }
        }
        for cite in found {
            if !out.contains(&cite) {
                out.push(cite);
            }
        }
    }
    out
}

/// Run the anchor. `root == None` does not open anything.
///
/// A bad `--commit` or a missing git checkout is an error. A cited location
/// that is not in the tree is a failed assessment, not an error.
pub fn assess_tree(
    text: &str,
    root: Option<&Path>,
    commit: Option<&str>,
) -> Result<AnchorAssessment, String> {
    let cites = extract_citations(text);
    let Some(root) = root else {
        let detail = if cites.is_empty() {
            "no target tree supplied, and the finding cites no source location".to_string()
        } else {
            format!(
                "no target tree supplied - {} cited location(s) were not opened. Unknown is not a pass",
                cites.len()
            )
        };
        return Ok(AnchorAssessment {
            status: GateStatus::Unknown,
            detail,
            finding: None,
        });
    };
    if !root.is_dir() {
        return Err(format!("--target {}: not a directory", root.display()));
    }
    if let Some(commit) = commit {
        // Reject a bad pin before deciding there is nothing to open. A value
        // that is not a hex SHA must not be ignored just because this draft
        // cites no file.
        let _ = resolve_commit(root, commit)?;
    }
    if cites.is_empty() {
        return Ok(AnchorAssessment {
            status: GateStatus::Unknown,
            detail: "the finding cites no source file, line, or function to anchor. Naming none is not a pass".into(),
            finding: None,
        });
    }
    let (pin, files) = if let Some(commit) = commit {
        load_git(root, commit, &cites)?
    } else {
        (None, load_disk(root, &cites)?)
    };
    Ok(judge(&cites, &files, pin.as_deref()))
}

fn judge(cites: &[Citation], files: &[Option<LoadedFile>], pin: Option<&str>) -> AnchorAssessment {
    let mut problems = Vec::new();
    for (cite, loaded) in cites.iter().zip(files.iter()) {
        let label = cite_label(cite);
        let Some(file) = loaded else {
            problems.push(format!("{label} does not exist in the target tree"));
            continue;
        };
        if let Some(line) = cite.line {
            if line == 0 {
                problems.push(format!("{label} is not a line number"));
                continue;
            }
            let end = cite.line_end.unwrap_or(line);
            if end < line {
                problems.push(format!("{label} has a backwards line range"));
                continue;
            }
            let n = file.text.lines().count() as u32;
            if end > n {
                problems.push(format!(
                    "{label} is past the end of {} ({n} lines) at {}",
                    file.rel,
                    pin.unwrap_or("the working tree")
                ));
            }
        }
        if let Some(name) = &cite.function {
            if !function_in_file(&file.text, name) {
                problems.push(format!(
                    "function {name} is not in {} at {}",
                    file.rel,
                    pin.unwrap_or("the working tree")
                ));
            }
        }
    }
    if !problems.is_empty() {
        let detail = problems.join("; ");
        return AnchorAssessment {
            status: GateStatus::Fail,
            finding: Some(cite_finding(&detail)),
            detail,
        };
    }
    let listed = cites.iter().map(cite_label).collect::<Vec<_>>().join(", ");
    match pin {
        Some(sha) => AnchorAssessment {
            status: GateStatus::Pass,
            detail: format!("pinned {sha}: {listed}"),
            finding: None,
        },
        None => AnchorAssessment {
            status: GateStatus::Unknown,
            detail: format!(
                "{listed} exist in the working tree, but no --commit pin was given. Unpinned is not a pass"
            ),
            finding: None,
        },
    }
}

fn cite_finding(detail: &str) -> Finding {
    Finding {
        rule: CITE_RULE.into(),
        title: "Cited location is not in the target tree".into(),
        group: "E".into(),
        verdict: Verdict::Hard,
        weight: CITE_WEIGHT,
        evidence: vec![crate::engine::collapse(detail)],
        matched_sentences: vec![crate::engine::collapse(detail)],
        fix: "Point --target at the checkout and --commit at the audited SHA. \
              Every cited source file, line, and function has to exist there. \
              A location that does not resolve is not a complete finding."
            .into(),
        doc: "rules/gates.yaml".into(),
        evidence_kind: Evidence::None,
        origin_rows: 0,
    }
}

fn cite_label(c: &Citation) -> String {
    let mut s = c.path.clone();
    if let Some(line) = c.line {
        s.push(':');
        s.push_str(&line.to_string());
        if let Some(end) = c.line_end {
            if end != line {
                s.push('-');
                s.push_str(&end.to_string());
            }
        }
    }
    if let Some(name) = &c.function {
        s.push(' ');
        s.push_str(name);
        s.push_str("()");
    }
    s
}

fn function_in_file(text: &str, name: &str) -> bool {
    let escaped = regex::escape(name);
    Regex::new(&format!(
        r"\b(?:function|fn|def|fun)\s+{escaped}\b|{escaped}\s*\("
    ))
    .is_ok_and(|re| re.is_match(text))
}

fn path_is_anchorable(path: &str) -> bool {
    if path.is_empty() || path.starts_with('-') || path.starts_with('/') {
        return false;
    }
    let parts: Vec<&str> = path.split('/').collect();
    if parts
        .iter()
        .any(|p| p.is_empty() || *p == "." || *p == "..")
    {
        return false;
    }
    !parts.iter().any(|p| HARNESS_DIRS.contains(p))
}

fn load_disk(root: &Path, cites: &[Citation]) -> Result<Vec<Option<LoadedFile>>, String> {
    let names = list_rel(root)?;
    let mut out = Vec::with_capacity(cites.len());
    for cite in cites {
        match resolve_name(&cite.path, &names) {
            Resolve::None => out.push(None),
            Resolve::Ambiguous(which) => {
                return Err(format!(
                    "--target: {} matches more than one file ({})",
                    cite.path,
                    which.join(", ")
                ));
            }
            Resolve::One(rel) => {
                let abs = root.join(&rel);
                let text = read_capped(&abs)?;
                out.push(Some(LoadedFile { rel, text }));
            }
        }
    }
    Ok(out)
}

fn load_git(
    root: &Path,
    commit: &str,
    cites: &[Citation],
) -> Result<(Option<String>, Vec<Option<LoadedFile>>), String> {
    let sha = resolve_commit(root, commit)?;
    let listed = git(
        root,
        &["ls-tree", "-r", "--name-only", "--end-of-options", &sha],
    )?;
    let names: Vec<String> = listed
        .lines()
        .map(|l| l.trim().replace('\\', "/"))
        .filter(|l| !l.is_empty())
        .collect();
    let mut files = Vec::with_capacity(cites.len());
    for cite in cites {
        match resolve_name(&cite.path, &names) {
            Resolve::None => files.push(None),
            Resolve::Ambiguous(which) => {
                return Err(format!(
                    "--commit {sha}: {} matches more than one file ({})",
                    cite.path,
                    which.join(", ")
                ));
            }
            Resolve::One(rel) => {
                let spec = format!("{sha}:{rel}");
                let text = match git(root, &["show", "--end-of-options", &spec]) {
                    Ok(body) => cap_text(body),
                    Err(_) => {
                        files.push(None);
                        continue;
                    }
                };
                files.push(Some(LoadedFile { rel, text }));
            }
        }
    }
    Ok((Some(sha), files))
}

enum Resolve {
    None,
    One(String),
    Ambiguous(Vec<String>),
}

fn resolve_name(cited: &str, names: &[String]) -> Resolve {
    let cited = cited.trim_start_matches("./");
    if let Some(exact) = names.iter().find(|n| n.as_str() == cited) {
        return Resolve::One(exact.clone());
    }
    let suffix = format!("/{cited}");
    let hits: Vec<String> = names
        .iter()
        .filter(|n| n.ends_with(&suffix) || n.as_str() == cited)
        .cloned()
        .collect();
    match hits.len() {
        0 => Resolve::None,
        1 => Resolve::One(hits.into_iter().next().unwrap()),
        _ => Resolve::Ambiguous(hits),
    }
}

fn list_rel(root: &Path) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let rd =
            std::fs::read_dir(&dir).map_err(|e| format!("cannot read {}: {e}", dir.display()))?;
        for entry in rd.flatten() {
            let path = entry.path();
            let Ok(meta) = std::fs::symlink_metadata(&path) else {
                continue;
            };
            if meta.file_type().is_symlink() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if meta.is_dir() {
                if !SKIP_DIRS.contains(&name.as_str()) {
                    stack.push(path);
                }
            } else if meta.is_file() {
                let rel = path.strip_prefix(root).unwrap_or(&path);
                let mut clean = String::new();
                let mut bad = false;
                for c in rel.components() {
                    match c {
                        Component::Normal(s) => {
                            if !clean.is_empty() {
                                clean.push('/');
                            }
                            clean.push_str(&s.to_string_lossy());
                        }
                        _ => bad = true,
                    }
                }
                if !bad && !clean.is_empty() {
                    out.push(clean);
                }
            }
        }
    }
    out.sort();
    Ok(out)
}

fn read_capped(path: &Path) -> Result<String, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    Ok(cap_text(text))
}

fn cap_text(text: String) -> String {
    let mut cut = text.len().min(FILE_CAP);
    while cut > 0 && !text.is_char_boundary(cut) {
        cut -= 1;
    }
    text[..cut].to_string()
}

/// A pin is a hex SHA. Refnames and option-shaped strings are rejected before
/// git sees them.
pub fn resolve_commit(root: &Path, commit: &str) -> Result<String, String> {
    if !(7..=64).contains(&commit.len()) || !commit.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(
            "--commit must be a 7 to 64 digit hex SHA. A refname or a value starting \
             with '-' is not a pin, and it is not passed to git."
                .into(),
        );
    }
    let sha = git(
        root,
        &[
            "rev-parse",
            "--verify",
            "--end-of-options",
            &format!("{commit}^{{commit}}"),
        ],
    )?;
    let sha = sha.trim();
    if sha.len() != 40 || !sha.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!("--commit {commit:?} did not resolve to a commit"));
    }
    Ok(sha.to_string())
}

fn git(root: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .map_err(|e| format!("cannot run git: {e}"))?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        let err = err.trim();
        return Err(format!("git {}: {err}", args.join(" ")));
    }
    String::from_utf8(output.stdout).map_err(|_| "git output was not UTF-8".to_string())
}

fn outside_fences(text: &str) -> String {
    let mut out = String::new();
    let mut fence: Option<(char, usize)> = None;
    for line in text.split_inclusive('\n') {
        if let Some((c, n)) = fence_marker(line) {
            match fence {
                None => {
                    fence = Some((c, n));
                    out.push('\n');
                }
                Some((oc, on)) if oc == c && n >= on && fence_closer(line, c) => {
                    fence = None;
                    out.push('\n');
                }
                Some(_) => out.push('\n'),
            }
        } else if fence.is_some() {
            out.push('\n');
        } else {
            out.push_str(line);
        }
    }
    out
}

fn fence_marker(line: &str) -> Option<(char, usize)> {
    let t = line.trim_start_matches(' ');
    if line.len() - t.len() > 3 {
        return None;
    }
    let c = t.chars().next()?;
    if c != '`' && c != '~' {
        return None;
    }
    let n = t.chars().take_while(|x| *x == c).count();
    (n >= 3).then_some((c, n))
}

fn fence_closer(line: &str, c: char) -> bool {
    line.trim().trim_start_matches(c).trim().is_empty()
}

/// Sentence-ish splits. A path's line number is read from the same chunk, so
/// a newline always ends one and '.' ends one before whitespace.
fn split_rough(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let chars: Vec<char> = text.chars().collect();
    for (i, c) in chars.iter().enumerate() {
        if *c == '\n' {
            if !cur.trim().is_empty() {
                out.push(std::mem::take(&mut cur));
            } else {
                cur.clear();
            }
            continue;
        }
        cur.push(*c);
        if matches!(c, '.' | '!' | '?') {
            let next = chars.get(i + 1).copied();
            let boundary = match next {
                None => true,
                Some(n) => n.is_whitespace() || matches!(n, ')' | '"' | '\'' | '*' | '_' | ']'),
            };
            if boundary && !cur.trim_end().ends_with("e.g.") && !cur.trim_end().ends_with("i.e.") {
                if !cur.trim().is_empty() {
                    out.push(std::mem::take(&mut cur));
                } else {
                    cur.clear();
                }
            }
        }
    }
    if !cur.trim().is_empty() {
        out.push(cur);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn files(pairs: &[(&str, &str)]) -> Vec<Option<LoadedFile>> {
        pairs
            .iter()
            .map(|(rel, text)| {
                Some(LoadedFile {
                    rel: (*rel).to_string(),
                    text: (*text).to_string(),
                })
            })
            .collect()
    }

    #[test]
    fn a_missing_file_fails_and_names_the_line() {
        let cites = extract_citations(
            "An unprivileged attacker drains 4 tokens via a rounding bug in `src/Vault.sol` at line 214, in function deposit.\n",
        );
        assert_eq!(cites.len(), 1);
        assert_eq!(cites[0].path, "src/Vault.sol");
        assert_eq!(cites[0].line, Some(214));
        assert_eq!(cites[0].function.as_deref(), Some("deposit"));
        let loaded = vec![None];
        let got = judge(&cites, &loaded, Some("abc"));
        assert_eq!(got.status, GateStatus::Fail);
        let finding = got.finding.expect("score-bearing finding");
        assert_eq!(finding.rule, "CITE");
        assert_eq!(finding.verdict, Verdict::Hard);
        assert!(finding.weight > 0);
        assert!(got.detail.contains("src/Vault.sol"));
        assert!(got.detail.contains("214"));
    }

    #[test]
    fn a_line_past_eof_fails() {
        let cites = extract_citations("The bug is in src/Vault.sol:214.\n");
        let loaded = files(&[("src/Vault.sol", "function deposit() {}\n")]);
        let got = judge(&cites, &loaded, Some("deadbeef"));
        assert_eq!(got.status, GateStatus::Fail);
        assert!(got.detail.contains("past the end"));
    }

    #[test]
    fn a_real_line_and_function_pass_only_when_pinned() {
        let body = "contract V {\n    function deposit() external {}\n}\n";
        let cites = extract_citations(
            "See src/Vault.sol:2, function deposit. An unprivileged attacker drains 4 tokens.\n",
        );
        assert_eq!(cites[0].line, Some(2));
        assert_eq!(cites[0].function.as_deref(), Some("deposit"));
        let loaded = files(&[("src/Vault.sol", body)]);
        let pinned = judge(
            &cites,
            &loaded,
            Some("0123456789abcdef0123456789abcdef01234567"),
        );
        assert_eq!(pinned.status, GateStatus::Pass, "{}", pinned.detail);
        let loose = judge(&cites, &loaded, None);
        assert_eq!(loose.status, GateStatus::Unknown);
        assert!(loose.finding.is_none());
    }

    #[test]
    fn a_missing_function_fails_even_when_the_line_exists() {
        let cites =
            extract_citations("src/Vault.sol:1 function withdraw does not zero the balance.\n");
        let loaded = files(&[("src/Vault.sol", "function deposit() external {}\n")]);
        let got = judge(&cites, &loaded, Some("abc"));
        assert_eq!(got.status, GateStatus::Fail);
        assert!(got.detail.contains("withdraw"));
    }

    #[test]
    fn harness_paths_and_fences_are_not_citations() {
        let text = "# Finding\n\nThe root cause is described below.\n\n```\nforge test --match-path test/Exploit.t.sol\n```\n\nThe harness is test/Exploit.t.sol:10.\n";
        assert!(
            extract_citations(text).is_empty(),
            "{:?}",
            extract_citations(text)
        );
    }

    #[test]
    fn a_hash_line_fragment_is_a_line() {
        let cites = extract_citations("Fixed in src/Vault.sol#L12-L14.\n");
        assert_eq!(cites.len(), 1);
        assert_eq!(cites[0].line, Some(12));
        assert_eq!(cites[0].line_end, Some(14));
    }

    #[test]
    fn commit_pins_reject_option_shaped_values() {
        let err = resolve_commit(Path::new("."), "--version").unwrap_err();
        assert!(err.contains("hex SHA"), "{err}");
        let err = resolve_commit(Path::new("."), "HEAD").unwrap_err();
        assert!(err.contains("hex SHA"), "{err}");
    }
}
