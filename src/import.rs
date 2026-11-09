//! `vg import` — turn a local engagement file, a saved program page, or a
//! prior-audit report into the ledger, scope, and known-issues inputs the
//! gates read.
//!
//! Offline. Nothing here fetches a URL. A program page is a file the user
//! already saved. A PDF is read from disk, with `pdftotext` as a fallback when
//! the file's text is compressed and the uncompressed scrape finds nothing.
//!
//! The transcription is mechanical and incomplete on purpose. A clause is
//! copied verbatim and a regex is not invented for it. An unstated floor stays
//! unstated. The NOTES file lists what was not found, because a guessed input
//! is worse than a missing one.

use crate::index;
use crate::init::yaml_quote;
use crate::model::*;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::LazyLock;

/// Clause-class ids from the vendored bounty-economics snapshot. Import will
/// not invent a class, and it will not write one the schema does not list.
const CLAUSE_CLASSES_YAML: &str = include_str!("../data/bounty-economics/data/clause-classes.yaml");
const TIER_SHAPES_YAML: &str = include_str!("../data/bounty-economics/data/tier-shapes.yaml");

#[derive(serde::Deserialize)]
struct IdList {
    #[serde(default)]
    classes: Vec<IdRow>,
    #[serde(default)]
    shapes: Vec<IdRow>,
}

#[derive(serde::Deserialize)]
struct IdRow {
    id: String,
}

fn load_ids(yaml: &str, field: &str) -> BTreeSet<String> {
    let parsed: IdList = serde_saphyr::from_str(yaml)
        .unwrap_or_else(|e| panic!("vendored taxonomy ({field}) did not parse: {e}"));
    let rows = if field == "classes" {
        parsed.classes
    } else {
        parsed.shapes
    };
    rows.into_iter().map(|r| r.id).collect()
}

fn clause_classes() -> &'static BTreeSet<String> {
    static IDS: LazyLock<BTreeSet<String>> =
        LazyLock::new(|| load_ids(CLAUSE_CLASSES_YAML, "classes"));
    &IDS
}

fn tier_shapes() -> &'static BTreeSet<String> {
    static IDS: LazyLock<BTreeSet<String>> = LazyLock::new(|| load_ids(TIER_SHAPES_YAML, "shapes"));
    &IDS
}

#[derive(Clone, Copy)]
pub enum Kind {
    Audit,
    Program,
    Prior,
}

pub fn run(kind: Kind, input: &Path, out_dir: &Path, force: bool) -> Result<ExitCode, String> {
    if !input.exists() {
        return Err(format!("no such file: {}", input.display()));
    }
    fs::create_dir_all(out_dir).map_err(|e| format!("cannot create {}: {e}", out_dir.display()))?;
    let notes = match kind {
        Kind::Audit => import_audit(input, out_dir, force)?,
        Kind::Program => import_program(input, out_dir, force)?,
        Kind::Prior => import_prior(input, out_dir, force)?,
    };
    let notes_path = out_dir.join("IMPORT-NOTES.md");
    write_new(&notes_path, &notes, force)?;
    println!("wrote {}", out_dir.display());
    println!("  read {}", notes_path.display());
    Ok(ExitCode::SUCCESS)
}

struct Bundle {
    ledger: String,
    scope: String,
    known: String,
    index: String,
    notes: Vec<String>,
}

fn import_audit(path: &Path, out: &Path, force: bool) -> Result<String, String> {
    let text = read_to_string(path)?;
    let eng: Engagement = serde_saphyr::from_str(&text)
        .map_err(|e| format!("{}: not an engagement file: {e}", path.display()))?;
    if let Some(v) = eng.version {
        if v != 1 {
            return Err(format!(
                "{}: version {v} is not supported (this build reads version 1)",
                path.display()
            ));
        }
    }
    let bundle = engagement_to_bundle(&eng);
    write_bundle(out, &bundle, force)?;
    Ok(render_notes("audit.yaml", &bundle.notes))
}

fn import_program(path: &Path, out: &Path, force: bool) -> Result<String, String> {
    let raw = read_to_string(path)?;
    let text = if looks_like_html(&raw) {
        html_to_text(&raw)
    } else {
        raw
    };
    let eng = engagement_from_page(&text, path);
    let bundle = engagement_to_bundle(&eng);
    write_bundle(out, &bundle, force)?;
    Ok(render_notes("saved program page", &bundle.notes))
}

fn import_prior(path: &Path, out: &Path, force: bool) -> Result<String, String> {
    let mut docs: Vec<(String, String)> = Vec::new();
    if path.is_dir() {
        let mut files = Vec::new();
        collect_docs(path, &mut files)?;
        files.sort();
        for f in files {
            let id = f
                .strip_prefix(path)
                .unwrap_or(&f)
                .to_string_lossy()
                .replace('\\', "/");
            let text = read_doc(&f)?;
            docs.push((id, text));
        }
    } else {
        let text = read_doc(path)?;
        docs.push((
            path.file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "prior".into()),
            text,
        ));
    }
    if docs.is_empty() {
        return Err(format!(
            "no markdown, text, or pdf under {}",
            path.display()
        ));
    }
    let mut entries = Vec::new();
    let mut notes = Vec::new();
    for (id, text) in &docs {
        // A report that already has several findings is split on headings that
        // look like issue titles. A single-finding file stays one row.
        let parts = split_findings(text);
        if parts.len() == 1 {
            entries.push(index::index_document(id, id, text));
        } else {
            for (n, part) in parts.iter().enumerate() {
                let pid = format!("{id}#{n}");
                entries.push(index::index_document(&pid, id, part));
            }
            notes.push(format!(
                "{id}: split into {} finding(s) on issue-style headings",
                parts.len()
            ));
        }
    }
    let with_ids = entries.iter().filter(|e| e.identifiers.len() >= 2).count();
    notes.push(format!(
        "{} row(s) indexed, {} with at least two strong identifiers. Rows with fewer cannot match: word overlap is not scored.",
        entries.len(),
        with_ids
    ));
    notes.push(
        "Quotes in the index are short excerpts of the source. Do not commit an index of reports you do not have the rights to redistribute.".into(),
    );
    let body = render_index(&entries);
    write_new(&out.join("index.yaml"), &body, force)?;
    Ok(render_notes("prior findings", &notes))
}

fn engagement_to_bundle(eng: &Engagement) -> Bundle {
    let mut notes = Vec::new();
    if let Some(p) = eng.platform.as_deref().filter(|s| !s.trim().is_empty()) {
        notes.push(format!(
            "platform field {p:?} was recorded and is not a detection signal"
        ));
    }
    let (tier_shape, floor_severity, floor_status, floor_note) = floor_of(eng);
    notes.push(floor_note);
    if eng.source_url.as_deref().unwrap_or("").trim().is_empty() {
        notes.push(
            "no source_url. G1 fails an uncited floor on purpose. Paste the page you read.".into(),
        );
    }
    if eng.retrieved.as_deref().unwrap_or("").trim().is_empty() {
        notes.push(
            "no retrieved date. G1 fails an undated floor. Date the page you saved, do not date today by habit.".into(),
        );
    }
    if eng.scope.is_empty() {
        notes.push("scope is empty. G3 cannot trace a root cause into an empty list.".into());
    }
    if eng.exclusion_clauses.is_empty() {
        notes.push("no exclusion clauses were transcribed. G2 will have nothing to test.".into());
    }
    let mut patterned = 0usize;
    let mut classified = 0usize;
    for (i, c) in eng.exclusion_clauses.iter().enumerate() {
        if c.body().is_empty() {
            notes.push(format!("exclusion_clauses[{i}] has no text; skipped"));
            continue;
        }
        let class = c.class.clone().unwrap_or_default();
        if !clause_classes().contains(class.trim()) {
            notes.push(format!(
                "exclusion_clauses[{i}] has class {class:?}, which is not one of the {} clause classes in data/bounty-economics/data/clause-classes.yaml. It is not written into the ledger. Import does not invent a class.",
                clause_classes().len()
            ));
            continue;
        }
        classified += 1;
        if c.pattern.as_deref().unwrap_or("").trim().is_empty() {
            notes.push(format!(
                "exclusion_clauses[{i}] copied verbatim with no pattern. Import does not invent a regex. G2 will report it as not tested until you write one. A starting pattern for this class is in data/bounty-economics/data/clause-classes.yaml (suggested_pattern); it is a prompt, not a verdict."
            ));
        } else {
            patterned += 1;
        }
    }
    notes.push(format!(
        "{} exclusion clause(s), {} with a schema class, {} with a pattern you supplied. Ledger schema: data/bounty-economics/schemas/ledger.schema.json (bounty-economics v0.5.0).",
        eng.exclusion_clauses.len(),
        classified,
        patterned
    ));
    if let Some(shape) = eng
        .tier_shape
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        if !tier_shapes().contains(shape) {
            notes.push(format!(
                "tier_shape {shape:?} is not one of the shapes in data/bounty-economics/data/tier-shapes.yaml"
            ));
        }
    }

    let mut index_rows = Vec::new();
    let mut known_rows = Vec::new();
    for (i, issue) in eng.known_issues.iter().enumerate() {
        let body = issue.body();
        if body.is_empty() {
            notes.push(format!("known_issues[{i}] has no summary; skipped"));
            continue;
        }
        let id = issue.id.clone().unwrap_or_else(|| format!("KI-{}", i + 1));
        let mut patterns = issue.patterns.clone();
        // Identifiers the engagement already names become literal patterns.
        // That is a copy, not a paraphrase. A regex is still not invented
        // from the prose.
        for ident in &issue.identifiers {
            let lit = regex_literal(ident);
            if !lit.is_empty() && !patterns.iter().any(|p| p == &lit) {
                patterns.push(lit);
            }
        }
        if patterns.is_empty() {
            notes.push(format!(
                "{id}: no patterns, so it is not written into known-issues.yaml. D1's regex side would fail the gate on an empty pattern list. The row is in the index; it matches only on strong identifiers."
            ));
        }
        let mut row =
            index::index_document(&id, issue.source.as_deref().unwrap_or("engagement"), &body);
        for ident in &issue.identifiers {
            if let Some(n) = nonempty(ident) {
                if !row.identifiers.iter().any(|e| e == &n.to_lowercase()) {
                    row.identifiers.push(n.to_lowercase());
                }
            }
        }
        if row.identifiers.len() < 2 {
            notes.push(format!(
                "{id}: fewer than two strong identifiers. It will not rank against a draft."
            ));
        }
        index_rows.push(row);
        if !patterns.is_empty() {
            known_rows.push((id, body, issue.source.clone(), patterns));
        }
    }
    if eng.known_issues.is_empty() {
        notes.push("no known issues were transcribed.".into());
    }
    for rule in &eng.severity_rules {
        if let Some(t) = rule.text.as_deref().filter(|s| !s.trim().is_empty()) {
            notes.push(format!(
                "severity rule ({}) is not attached to a draft. Put it in finding.yaml as severity_rule_quoted, or quote it in the report. Text: {t}",
                rule.tier.as_deref().unwrap_or("untiered")
            ));
        }
    }

    let ledger = render_ledger(eng, &tier_shape, &floor_severity, &floor_status);
    let scope = eng
        .scope
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| format!("{s}\n"))
        .collect::<String>();
    let known = render_known(&known_rows);
    let index = render_index(&index_rows);
    Bundle {
        ledger,
        scope,
        known,
        index,
        notes,
    }
}

fn floor_of(eng: &Engagement) -> (String, String, String, String) {
    if let Some(shape) = eng
        .tier_shape
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        let sev = eng
            .severity_floor
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty() && *s != "none" && *s != "unstated")
            .unwrap_or("")
            .to_string();
        let status = eng
            .floor_status
            .clone()
            .unwrap_or_else(|| "documented".into());
        return (
            shape.to_string(),
            sev,
            status,
            format!("tier_shape {shape:?} taken from the engagement, not inferred"),
        );
    }
    let raw = eng
        .severity_floor
        .as_deref()
        .unwrap_or("")
        .trim()
        .to_lowercase();
    let (shape, sev, note) = match raw.as_str() {
        "critical" => (
            "critical_only",
            "critical",
            "severity_floor critical -> tier_shape critical_only".to_string(),
        ),
        "high" => (
            "high_and_above",
            "high",
            "severity_floor high -> tier_shape high_and_above".to_string(),
        ),
        "medium" => (
            "medium_and_above",
            "medium",
            "severity_floor medium -> tier_shape medium_and_above".to_string(),
        ),
        "low" => (
            "low_and_above",
            "low",
            "severity_floor low -> tier_shape low_and_above".to_string(),
        ),
        "none" | "unstated" | "" => (
            "unknown",
            "",
            "no severity floor in the engagement. tier_shape left unknown, which G1 treats as unread, not as permissive".to_string(),
        ),
        other => (
            "unknown",
            "",
            format!(
                "severity_floor {other:?} is not critical|high|medium|low|none. Left unstated."
            ),
        ),
    };
    let status = eng.floor_status.clone().unwrap_or_else(|| {
        if sev.is_empty() {
            "unstated".into()
        } else {
            "documented".into()
        }
    });
    (shape.into(), sev.into(), status, note)
}

fn engagement_from_page(text: &str, path: &Path) -> Engagement {
    let sections = page_sections(text);
    let mut exclusion_clauses = Vec::new();
    let mut known_issues = Vec::new();
    let mut scope = Vec::new();
    let mut severity_rules = Vec::new();
    let mut reward_text = String::new();
    for (heading, body) in &sections {
        let h = heading.to_lowercase();
        if is_exclusion_heading(&h) {
            for b in bullets(body) {
                exclusion_clauses.push(EngagementClause {
                    id: None,
                    class: Some("transcribed".into()),
                    text: Some(b),
                    clause: None,
                    pattern: None,
                });
            }
        } else if is_known_heading(&h) {
            for b in bullets(body) {
                known_issues.push(EngagementIssue {
                    id: None,
                    summary: Some(b),
                    text: None,
                    source: Some(path.display().to_string()),
                    patterns: vec![],
                    identifiers: vec![],
                });
            }
        } else if is_reward_heading(&h) {
            reward_text.push_str(body);
            reward_text.push('\n');
            for b in bullets(body) {
                if SEVERITY_WORD.is_match(&b) {
                    severity_rules.push(EngagementSeverityRule {
                        tier: None,
                        text: Some(b),
                    });
                }
            }
        } else if is_scope_heading(&h) {
            for b in bullets(body) {
                scope.push(b);
            }
        }
    }
    let floor = infer_floor(&reward_text);
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "program".into());
    Engagement {
        version: Some(1),
        id: stem,
        program: None,
        platform: detect_page_family(text),
        severity_floor: floor,
        floor_status: None,
        tier_shape: None,
        source_url: None,
        retrieved: None,
        exclusion_clauses,
        known_issues,
        scope,
        severity_rules,
        prior_audits: vec![],
    }
}

fn is_exclusion_heading(h: &str) -> bool {
    contains_any(
        h,
        &[
            "out of scope",
            "out-of-scope",
            "exclusion",
            "prohibited",
            "not eligible",
            "ineligible",
            "not in scope",
        ],
    )
}

fn is_known_heading(h: &str) -> bool {
    contains_any(
        h,
        &[
            "known issue",
            "previously known",
            "prior audit",
            "previous audit",
            "already known",
        ],
    )
}

fn is_scope_heading(h: &str) -> bool {
    // "out of scope" contains "scope". Exclusions are classified first.
    (h.contains("scope") || h.contains("assets in scope") || h.contains("in-scope"))
        && !is_exclusion_heading(h)
}

fn is_reward_heading(h: &str) -> bool {
    contains_any(
        h,
        &[
            "reward",
            "severity",
            "impacts in scope",
            "impact in scope",
            "payout",
            "bounty",
        ],
    )
}

fn contains_any(h: &str, needles: &[&str]) -> bool {
    needles.iter().any(|n| h.contains(n))
}

fn infer_floor(reward_text: &str) -> Option<String> {
    if reward_text.trim().is_empty() {
        return None;
    }
    let low = reward_text.to_lowercase();
    if low.contains("critical only")
        || low.contains("only critical")
        || low.contains("critical-only")
    {
        return Some("critical".into());
    }
    // A tier counts as paid when a currency amount sits near the word and the
    // same neighbourhood does not say the tier is unpaid.
    let mut paid = Vec::new();
    for (name, rank) in [("low", 0), ("medium", 1), ("high", 2), ("critical", 3)] {
        if tier_looks_paid(&low, name) {
            paid.push((rank, name));
        }
    }
    paid.sort_by_key(|p| p.0);
    paid.first().map(|p| p.1.to_string())
}

fn tier_looks_paid(text: &str, tier: &str) -> bool {
    // One bullet at a time. A window of characters would let "Medium: n/a"
    // on the next line mark High unpaid, or the reverse.
    for line in text.lines() {
        let low = line.to_lowercase();
        if !line_has_word(&low, tier) {
            continue;
        }
        let unpaid = low.contains("n/a")
            || low.contains("not eligible")
            || low.contains("no reward")
            || low.contains("ineligible")
            || low.contains("$0")
            || low.contains("0 usd");
        let paid = low.contains('$') || low.contains("usd");
        if paid && !unpaid {
            return true;
        }
    }
    false
}

fn line_has_word(line: &str, word: &str) -> bool {
    line.split(|c: char| !c.is_alphanumeric())
        .any(|t| t == word)
}

fn bullets(body: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in body.lines() {
        let t = line.trim();
        let item = t
            .strip_prefix("- ")
            .or_else(|| t.strip_prefix("* "))
            .or_else(|| t.strip_prefix("• "));
        if let Some(item) = item {
            let item = item.trim();
            if !item.is_empty() {
                out.push(item.to_string());
            }
        }
    }
    out
}

fn page_sections(text: &str) -> Vec<(String, String)> {
    let mut sections = Vec::new();
    let mut heading = String::from("preamble");
    let mut body = String::new();
    for line in text.lines() {
        if let Some(h) = line.trim().strip_prefix("# ") {
            if !body.trim().is_empty() || heading != "preamble" {
                sections.push((heading.clone(), body.clone()));
            }
            heading = h.trim().to_string();
            body.clear();
        } else {
            body.push_str(line);
            body.push('\n');
        }
    }
    sections.push((heading, body));
    sections
}

/// Which saved-page family this file looks like, from its own wording.
/// Recorded in NOTES. Not used as a rule signal.
fn detect_page_family(text: &str) -> Option<String> {
    let low = text.to_lowercase();
    // Checked as substrings of the saved page. The order is not a preference.
    if low.contains("immunefi") {
        Some("immunefi".into())
    } else if low.contains("hackenproof") {
        Some("hackenproof".into())
    } else if low.contains("cantina") {
        Some("cantina".into())
    } else {
        None
    }
}

pub fn looks_like_html(text: &str) -> bool {
    let head: String = text.chars().take(2000).collect::<String>().to_lowercase();
    head.contains("<html") || head.contains("<div") || head.contains("<p") || head.contains("<h1")
}

pub fn html_to_text(html: &str) -> String {
    let no_script = strip_paired(html, "script");
    let no_style = strip_paired(&no_script, "style");
    let mut out = String::new();
    let bytes = no_style.as_str();
    let mut rest = bytes;
    while let Some(start) = rest.find('<') {
        out.push_str(&decode_entities(&rest[..start]));
        rest = &rest[start + 1..];
        let Some(end) = rest.find('>') else {
            break;
        };
        let tag = &rest[..end];
        rest = &rest[end + 1..];
        let name = tag
            .split_whitespace()
            .next()
            .unwrap_or("")
            .trim_start_matches('/')
            .to_lowercase();
        match name.as_str() {
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => out.push_str("\n# "),
            "li" => out.push_str("\n- "),
            "p" | "div" | "br" | "tr" | "section" | "h1/" | "h2/" => out.push('\n'),
            _ if tag.starts_with('/')
                && matches!(
                    name.as_str(),
                    "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "p" | "li" | "div" | "tr"
                ) =>
            {
                out.push('\n');
            }
            _ => {}
        }
    }
    out.push_str(&decode_entities(rest));
    out
}

fn strip_paired(text: &str, tag: &str) -> String {
    let low = text.to_lowercase();
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let mut out = String::new();
    let mut i = 0usize;
    while let Some(rel) = low[i..].find(&open) {
        let at = i + rel;
        out.push_str(&text[i..at]);
        if let Some(end_rel) = low[at..].find(&close) {
            i = at + end_rel + close.len();
        } else {
            i = text.len();
            break;
        }
    }
    out.push_str(&text[i..]);
    out
}

fn decode_entities(s: &str) -> String {
    s.replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
}

static SEVERITY_WORD: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
    regex::Regex::new(r"(?i)\b(critical|high|medium|low)\b").expect("severity word")
});

fn split_findings(text: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut cur = String::new();
    let mut seen = false;
    for line in text.lines() {
        let is_issue = line.trim().starts_with('#') && heading_is_issue(line.trim());
        if is_issue {
            if seen && !cur.trim().is_empty() {
                parts.push(std::mem::take(&mut cur));
            } else {
                // The preamble before the first issue heading is not a finding.
                cur.clear();
            }
            seen = true;
        }
        cur.push_str(line);
        cur.push('\n');
    }
    if seen && !cur.trim().is_empty() {
        parts.push(cur);
    }
    if parts.len() >= 2 {
        parts
    } else {
        vec![text.to_string()]
    }
}

fn heading_is_issue(heading: &str) -> bool {
    let h = heading.trim().trim_start_matches('#').trim();
    let low = h.to_lowercase();
    low.starts_with("finding")
        || low.starts_with("issue ")
        || h.split(|c: char| !c.is_ascii_alphanumeric() && c != '-')
            .any(token_is_issue_id)
}

fn token_is_issue_id(token: &str) -> bool {
    let token = token.to_lowercase();
    let mut chars = token.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !"chmliqs".contains(first) {
        return false;
    }
    if chars.next() != Some('-') {
        return false;
    }
    let rest: String = chars.collect();
    (1..=3).contains(&rest.len()) && rest.chars().all(|c| c.is_ascii_digit())
}

fn collect_docs(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let rd = fs::read_dir(dir).map_err(|e| format!("cannot read {}: {e}", dir.display()))?;
    for entry in rd.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') || name == "node_modules" || name == "target" {
            continue;
        }
        let Ok(meta) = fs::symlink_metadata(&path) else {
            continue;
        };
        if meta.file_type().is_symlink() {
            continue;
        }
        if meta.is_dir() {
            collect_docs(&path, out)?;
        } else if is_doc(&path) {
            out.push(path);
        }
    }
    Ok(())
}

fn is_doc(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .as_deref(),
        Some("md" | "txt" | "markdown" | "pdf")
    )
}

fn read_doc(path: &Path) -> Result<String, String> {
    if path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("pdf"))
    {
        pdf_to_text(path)
    } else {
        read_to_string(path)
    }
}

pub fn pdf_to_text(path: &Path) -> Result<String, String> {
    let bytes = fs::read(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let scraped = scrape_pdf_literals(&bytes);
    let letters = scraped.chars().filter(|c| c.is_alphanumeric()).count();
    if letters > 40 {
        return Ok(scraped);
    }
    match std::process::Command::new("pdftotext")
        .arg("-layout")
        .arg("-q")
        .arg(path)
        .arg("-")
        .output()
    {
        Ok(out) if out.status.success() => {
            let text = String::from_utf8_lossy(&out.stdout).to_string();
            if text.trim().is_empty() {
                Err(format!(
                    "{}: pdftotext returned no text",
                    path.display()
                ))
            } else {
                Ok(text)
            }
        }
        Ok(out) => Err(format!(
            "{}: pdftotext failed ({})",
            path.display(),
            String::from_utf8_lossy(&out.stderr).trim()
        )),
        Err(_) => Err(format!(
            "{}: no extractable text (the uncompressed scrape found nothing and pdftotext is not installed). Convert it with `pdftotext -layout` and pass the .txt.",
            path.display()
        )),
    }
}

/// Pull literal strings out of an uncompressed PDF. Compressed streams are
/// left for `pdftotext`. This is enough for a text-showing PDF that was not
/// deflated, and it does not pretend to be a PDF parser.
pub fn scrape_pdf_literals(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    let mut out = String::new();
    let b = text.as_bytes();
    let mut i = 0usize;
    while i < b.len() {
        if b[i] == b'(' {
            let mut j = i + 1;
            let mut buf = String::new();
            let mut escaped = false;
            let mut depth = 1i32;
            while j < b.len() {
                let c = b[j] as char;
                if escaped {
                    buf.push(match c {
                        'n' => '\n',
                        'r' => '\r',
                        't' => '\t',
                        other => other,
                    });
                    escaped = false;
                    j += 1;
                    continue;
                }
                match c {
                    '\\' => escaped = true,
                    '(' => {
                        depth += 1;
                        buf.push(c);
                    }
                    ')' => {
                        depth -= 1;
                        if depth == 0 {
                            j += 1;
                            break;
                        }
                        buf.push(c);
                    }
                    _ => buf.push(c),
                }
                j += 1;
            }
            let after = text[j..].trim_start();
            if (after.starts_with("Tj") || after.starts_with('\'') || after.starts_with("TJ"))
                && !buf.trim().is_empty()
            {
                out.push_str(buf.trim());
                out.push('\n');
            }
            i = j;
        } else {
            i += 1;
        }
    }
    out
}

fn render_ledger(eng: &Engagement, tier: &str, floor: &str, status: &str) -> String {
    let mut s = String::from(
        "# Generated by `vg import`. Check it before you trust a gate.\n\
         # Shape: data/bounty-economics/schemas/ledger.schema.json\n\
         # (snapshot of https://github.com/KhomDev/bounty-economics v0.5.0, MIT).\n\
         # A missing source_url or retrieved date fails G1, and the schema, on purpose.\n\n",
    );
    s.push_str("- id: ");
    s.push_str(&yaml_quote(&eng.id));
    s.push('\n');
    s.push_str("  schema_version: 1\n");
    s.push_str("  tier_shape: ");
    s.push_str(&yaml_quote(tier));
    s.push('\n');
    s.push_str("  floor_status: ");
    s.push_str(&yaml_quote(status));
    s.push('\n');
    s.push_str("  floor_severity: ");
    if floor.is_empty() {
        s.push_str("null\n");
    } else {
        s.push_str(&yaml_quote(floor));
        s.push('\n');
    }
    s.push_str("  source_url: ");
    s.push_str(&yaml_quote(eng.source_url.as_deref().unwrap_or("")));
    s.push('\n');
    s.push_str("  retrieved: ");
    s.push_str(&yaml_quote(eng.retrieved.as_deref().unwrap_or("")));
    s.push('\n');
    s.push_str("  conflicts: []\n");
    let mut exclusions = String::new();
    let mut any = false;
    for c in &eng.exclusion_clauses {
        let body = c.body();
        if body.is_empty() {
            continue;
        }
        let class = c.class.clone().unwrap_or_default();
        if !clause_classes().contains(class.trim()) {
            continue;
        }
        any = true;
        exclusions.push_str("    - class: ");
        exclusions.push_str(&yaml_quote(&class));
        exclusions.push('\n');
        exclusions.push_str("      clause: ");
        exclusions.push_str(&yaml_quote(&body));
        exclusions.push('\n');
        if let Some(p) = c.pattern.as_deref().filter(|p| !p.trim().is_empty()) {
            exclusions.push_str("      pattern: ");
            exclusions.push_str(&yaml_quote(p));
            exclusions.push('\n');
        }
    }
    if any {
        s.push_str("  exclusions:\n");
        s.push_str(&exclusions);
    } else {
        s.push_str("  exclusions: []\n");
    }
    if eng.prior_audits.is_empty() {
        s.push_str("  prior_audits: []\n");
    } else {
        s.push_str("  prior_audits:\n");
    }
    for a in &eng.prior_audits {
        s.push_str("    - firm: ");
        s.push_str(&yaml_quote(a.firm.as_deref().unwrap_or("")));
        s.push('\n');
        s.push_str("      url: ");
        s.push_str(&yaml_quote(a.url.as_deref().unwrap_or("")));
        s.push('\n');
        s.push_str("      covers:\n");
        if a.covers.is_empty() {
            s.push_str("        []\n");
        }
        for c in &a.covers {
            s.push_str("        - ");
            s.push_str(&yaml_quote(c));
            s.push('\n');
        }
        s.push_str("      known_issues:\n");
        if a.known_issues.is_empty() {
            s.push_str("        []\n");
        }
        for k in &a.known_issues {
            s.push_str("        - ");
            s.push_str(&yaml_quote(k));
            s.push('\n');
        }
    }
    s.push_str("  post_audit_delta: \"\"\n");
    s
}

fn render_known(rows: &[(String, String, Option<String>, Vec<String>)]) -> String {
    let mut s = String::from(
        "# Generated by `vg import`. Patterns are only what the source already\n\
         # named. A row with no patterns is listed so you can see it was not dropped,\n\
         # and D1's regex side will say it cannot match that row.\n\nknown_issues:\n",
    );
    if rows.is_empty() {
        s.push_str("  []\n");
        return s;
    }
    for (id, summary, source, patterns) in rows {
        s.push_str("  - id: ");
        s.push_str(&yaml_quote(id));
        s.push('\n');
        s.push_str("    summary: ");
        s.push_str(&yaml_quote(summary));
        s.push('\n');
        if let Some(src) = source {
            s.push_str("    source: ");
            s.push_str(&yaml_quote(src));
            s.push('\n');
        }
        if patterns.is_empty() {
            s.push_str("    patterns: []\n");
        } else {
            s.push_str("    patterns:\n");
        }
        for p in patterns {
            s.push_str("      - ");
            s.push_str(&yaml_quote(p));
            s.push('\n');
        }
    }
    s
}

pub fn render_index(entries: &[IndexedFinding]) -> String {
    let mut s = String::from(
        "# Local prior-finding index. Generated by `vg import`.\n\
         # Matching uses identifiers and root-cause stems, not word overlap.\n\
         # version: 1\n\nversion: 1\nentries:\n",
    );
    if entries.is_empty() {
        s.push_str("  []\n");
        return s;
    }
    for e in entries {
        s.push_str("  - id: ");
        s.push_str(&yaml_quote(&e.id));
        s.push('\n');
        s.push_str("    source: ");
        s.push_str(&yaml_quote(&e.source));
        s.push('\n');
        s.push_str("    title: ");
        s.push_str(&yaml_quote(&e.title));
        s.push('\n');
        push_keyed_list(&mut s, "identifiers", &e.identifiers);
        push_keyed_list(&mut s, "root_causes", &e.root_causes);
        push_keyed_list(&mut s, "quotes", &e.quotes);
    }
    s
}

fn push_keyed_list(s: &mut String, key: &str, items: &[String]) {
    if items.is_empty() {
        s.push_str(&format!("    {key}: []\n"));
        return;
    }
    s.push_str(&format!("    {key}:\n"));
    for item in items {
        s.push_str("      - ");
        s.push_str(&yaml_quote(item));
        s.push('\n');
    }
}

fn render_notes(what: &str, lines: &[String]) -> String {
    let mut s = format!("# Import notes — {what}\n\n");
    s.push_str(
        "Generated by `vg import`. This is a transcription, not a judgement. \
         A line that says something was not found means the gate will not see it. \
         Do not fill the gap with a guess.\n\n",
    );
    for line in lines {
        s.push_str("- ");
        s.push_str(line);
        s.push('\n');
    }
    s
}

fn write_bundle(out: &Path, bundle: &Bundle, force: bool) -> Result<(), String> {
    write_new(&out.join("ledger.yaml"), &bundle.ledger, force)?;
    write_new(&out.join("scope.txt"), &bundle.scope, force)?;
    write_new(&out.join("known-issues.yaml"), &bundle.known, force)?;
    write_new(&out.join("index.yaml"), &bundle.index, force)?;
    Ok(())
}

fn write_new(path: &Path, body: &str, force: bool) -> Result<(), String> {
    if path.exists() && !force {
        return Err(format!(
            "{} already exists. Re-run with --force to overwrite it.",
            path.display()
        ));
    }
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, body).map_err(|e| format!("cannot write {}: {e}", tmp.display()))?;
    fs::rename(&tmp, path).map_err(|e| format!("cannot rename into {}: {e}", path.display()))?;
    Ok(())
}

fn read_to_string(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))
}

fn nonempty(s: &str) -> Option<&str> {
    let t = s.trim();
    (!t.is_empty()).then_some(t)
}

fn regex_literal(ident: &str) -> String {
    let t = ident.trim();
    if t.is_empty() {
        return String::new();
    }
    // A literal, not a pattern. Escape every regex metacharacter.
    let mut out = String::from(r"\b");
    for c in t.chars() {
        if matches!(
            c,
            '.' | '+' | '*' | '?' | '(' | ')' | '[' | ']' | '{' | '}' | '|' | '^' | '$' | '\\'
        ) {
            out.push('\\');
        }
        out.push(c);
    }
    out.push_str(r"\b");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clause_classes_come_from_the_vendored_taxonomy() {
        assert!(clause_classes().contains("privileged_role"));
        assert!(clause_classes().contains("best_practice"));
        assert!(!clause_classes().contains("transcribed"));
        assert!(tier_shapes().contains("medium_and_above"));
        assert!(!tier_shapes().contains("any"));
    }

    #[test]
    fn html_page_becomes_a_ledger_and_does_not_invent_a_floor() {
        let html = r#"<html><body>
            <h2>Rewards</h2>
            <ul>
              <li>Critical: $50,000</li>
              <li>High: $10,000</li>
              <li>Medium: n/a</li>
            </ul>
            <h2>Out of scope</h2>
            <ul><li>Attacks that need a privileged role</li></ul>
            <h2>In scope</h2>
            <ul><li>contracts/Vault.sol</li></ul>
            <h2>Known issues</h2>
            <ul><li>`reduce_outflow` credits the current segment only</li></ul>
            <script>ignored()</script>
            </body></html>"#;
        let text = html_to_text(html);
        assert!(!text.contains("ignored()"));
        let eng = engagement_from_page(&text, Path::new("saved-page.html"));
        assert_eq!(eng.severity_floor.as_deref(), Some("high"));
        assert_eq!(eng.exclusion_clauses.len(), 1);
        assert!(eng.exclusion_clauses[0].pattern.is_none());
        assert_eq!(eng.scope, vec!["contracts/Vault.sol".to_string()]);
        assert_eq!(eng.known_issues.len(), 1);
        assert!(eng.source_url.is_none());
    }

    #[test]
    fn an_uncompressed_pdf_literal_is_scraped() {
        let pdf = b"BT (reduce_outflow credits the current segment only) Tj ET";
        let text = scrape_pdf_literals(pdf);
        assert!(text.contains("reduce_outflow"), "{text}");
    }

    #[test]
    fn issue_headings_split_a_report() {
        let text = "# Intro\n\nPreamble.\n\n# M-1 Limiter\n\n`reduce_outflow` is wrong.\n\n# H-2 Other\n\n`settle` is wrong.\n";
        let parts = split_findings(text);
        assert_eq!(parts.len(), 2, "{parts:?}");
    }
}
