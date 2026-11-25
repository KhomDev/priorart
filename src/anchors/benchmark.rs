//! The DEV-corpus resolution measurement.
//!
//! For each corpus under `eval/benchmark/` this reads the pinned finding texts
//! from the gitignored fetch cache, counts the findings that cite code under a
//! definition fixed in this module, and, when the audited repository checkout
//! is present at the pinned commit, resolves them and reports the fraction with
//! at least one span.
//!
//! A corpus whose audited source is absent is **blocked**: it reports its
//! finding and citation counts but no resolution rate. It is never counted as
//! zero resolved, because that would turn a missing input into a false number.

use crate::anchors::cite::cites_code;
use crate::anchors::index::{parse_repo, IndexOptions, IndexSet};
use crate::anchors::{resolve_text, ANCHORS_BENCH_SCHEMA};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The six DEV corpora, in report order. `tapioca` is the root benchmark
/// directory; the others are subdirectories.
pub const DEV_CORPORA: [&str; 6] = [
    "tapioca",
    "pooltogether",
    "maia-c4",
    "panoptic-c4",
    "party-c4",
    "wildcat-c4",
];

/// One benchmark run's inputs.
#[derive(Debug, Clone)]
pub struct BenchOptions {
    pub benchmark_root: PathBuf,
    pub cache_root: PathBuf,
    pub sources_root: PathBuf,
    pub allow_unverified_commit: bool,
    pub corpora: Vec<String>,
}

impl BenchOptions {
    pub fn new(
        benchmark_root: impl Into<PathBuf>,
        cache_root: impl Into<PathBuf>,
        sources_root: impl Into<PathBuf>,
    ) -> Self {
        Self {
            benchmark_root: benchmark_root.into(),
            cache_root: cache_root.into(),
            sources_root: sources_root.into(),
            allow_unverified_commit: false,
            corpora: DEV_CORPORA.iter().map(|s| s.to_string()).collect(),
        }
    }
}

/// Whether a corpus was measured or blocked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CorpusStatus {
    Measured,
    Blocked,
}

/// A k/n rate with its Wilson 95% interval (absent for n < 5).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Rate {
    pub resolved: usize,
    pub citing: usize,
    pub point: f64,
    pub wilson: Option<[f64; 2]>,
}

/// One corpus's row.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorpusReport {
    pub corpus: String,
    pub status: CorpusStatus,
    pub blocked_reason: Option<String>,
    pub audited_repo: Option<String>,
    pub audited_commit: Option<String>,
    pub source_checkout: Option<String>,
    pub commit_verified: bool,
    /// Solidity files the index parsed, when a source was available.
    pub files_indexed: Option<usize>,
    /// Solidity files that failed to parse, when a source was available.
    pub files_unparsed: Option<usize>,
    pub findings_total: usize,
    pub findings_citing_code: usize,
    pub findings_resolved: Option<usize>,
    pub resolution_rate: Option<Rate>,
    /// Every abstention reason seen, counted once per finding that hit it.
    pub abstention_citations_by_reason: BTreeMap<String, usize>,
    /// Unresolved findings counted once, under the most specific reason.
    pub unresolved_findings_by_reason: BTreeMap<String, usize>,
}

/// The pooled row over measured corpora only.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PooledReport {
    pub measured_corpora: usize,
    pub blocked_corpora: usize,
    pub findings_citing_code: usize,
    pub findings_resolved: usize,
    pub resolution_rate: Rate,
}

/// The measurement document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchReport {
    pub schema: String,
    pub tool_version: String,
    pub benchmark_root: String,
    pub cache_root: String,
    pub sources_root: String,
    /// The definition of "cites code", written before the number.
    pub cites_code_definition: Vec<String>,
    pub corpora: Vec<CorpusReport>,
    pub pooled: Option<PooledReport>,
}

/// Wilson's 95% interval for `k` of `n`. `None` when `n < 5`: below that the
/// interval is wider than it is useful and the count is reported instead.
pub fn wilson(k: usize, n: usize) -> Option<[f64; 2]> {
    if n == 0 || n < 5 {
        return None;
    }
    let p = k as f64 / n as f64;
    let z = 1.959_963_984_540_054_f64;
    let denom = 1.0 + z * z / n as f64;
    let centre = (p + z * z / (2.0 * n as f64)) / denom;
    let half = z * ((p * (1.0 - p) / n as f64) + z * z / (4.0 * (n as f64).powi(2))).sqrt() / denom;
    Some([
        round4((centre - half).max(0.0)),
        round4((centre + half).min(1.0)),
    ])
}

fn round4(v: f64) -> f64 {
    (v * 10_000.0).round() / 10_000.0
}

fn rate(k: usize, n: usize) -> Rate {
    Rate {
        resolved: k,
        citing: n,
        point: if n == 0 {
            0.0
        } else {
            round4(k as f64 / n as f64)
        },
        wilson: wilson(k, n),
    }
}

/// Run the measurement.
pub fn bench(options: &BenchOptions) -> Result<BenchReport, String> {
    let mut corpora = Vec::new();
    for name in &options.corpora {
        corpora.push(measure_corpus(name, options));
    }
    let measured: Vec<&CorpusReport> = corpora
        .iter()
        .filter(|c| c.status == CorpusStatus::Measured)
        .collect();
    let blocked_count = corpora.len() - measured.len();
    let pooled = if measured.is_empty() {
        None
    } else {
        let citing: usize = measured.iter().map(|c| c.findings_citing_code).sum();
        let resolved: usize = measured
            .iter()
            .map(|c| c.findings_resolved.unwrap_or(0))
            .sum();
        Some(PooledReport {
            measured_corpora: measured.len(),
            blocked_corpora: blocked_count,
            findings_citing_code: citing,
            findings_resolved: resolved,
            resolution_rate: rate(resolved, citing),
        })
    };
    Ok(BenchReport {
        schema: ANCHORS_BENCH_SCHEMA.to_string(),
        tool_version: env!("CARGO_PKG_VERSION").to_string(),
        benchmark_root: normalise_root(&options.benchmark_root),
        cache_root: normalise_root(&options.cache_root),
        sources_root: normalise_root(&options.sources_root),
        cites_code_definition: vec![
            "a token ending in `.sol`, with an optional `:N`, `:N-M`, `#LN` or `#LN-LM` line fragment".to_string(),
            "a GitHub `blob`/`tree` URL to a `.sol` file, with an optional `#L` line fragment".to_string(),
            "a backticked identifier after stripping a call's parentheses and keeping the last dotted member, excluding Solidity keywords, type names and builtins".to_string(),
            "the word `function` followed by an identifier".to_string(),
            "a plain `Contract.member` pair: an uppercase type qualifier, a dot, and a member name".to_string(),
        ],
        corpora,
        pooled,
    })
}

fn normalise_root(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn measure_corpus(name: &str, options: &BenchOptions) -> CorpusReport {
    let benchmark_dir = if name == "tapioca" || !options.benchmark_root.join(name).is_dir() {
        options.benchmark_root.clone()
    } else {
        options.benchmark_root.join(name)
    };
    let is_root_corpus = benchmark_dir == options.benchmark_root;
    let cache_dir = if is_root_corpus {
        options.cache_root.clone()
    } else {
        options.cache_root.join(name)
    };

    let mut report = CorpusReport {
        corpus: name.to_string(),
        status: CorpusStatus::Blocked,
        blocked_reason: None,
        audited_repo: None,
        audited_commit: None,
        source_checkout: None,
        commit_verified: false,
        files_indexed: None,
        files_unparsed: None,
        findings_total: 0,
        findings_citing_code: 0,
        findings_resolved: None,
        resolution_rate: None,
        abstention_citations_by_reason: BTreeMap::new(),
        unresolved_findings_by_reason: BTreeMap::new(),
    };

    // The pinned audited repository and commit are in the committed
    // cache.lock.json, as the `known-issues` source. For every DEV corpus that
    // repository is the contest snapshot the findings cite.
    let lock_path = benchmark_dir.join("cache.lock.json");
    let lock = match std::fs::read_to_string(&lock_path)
        .ok()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
    {
        Some(lock) => lock,
        None => {
            report.blocked_reason = Some(format!("lock-missing: {}", normalise_root(&lock_path)));
            return report;
        }
    };
    let roles = lock.get("sources").and_then(|s| s.as_object());
    let known = roles.and_then(|r| {
        r.iter()
            .find(|(_, v)| v.get("role").and_then(|x| x.as_str()) == Some("known-issues"))
            .map(|(id, v)| (id.clone(), v.clone()))
    });
    let judging = roles.and_then(|r| {
        r.iter()
            .find(|(_, v)| v.get("role").and_then(|x| x.as_str()) == Some("judging"))
            .map(|(id, v)| (id.clone(), v.clone()))
    });
    let (_known_id, known) = match known {
        Some(pair) => pair,
        None => {
            report.blocked_reason = Some("lock has no known-issues source".to_string());
            return report;
        }
    };
    let repo = known
        .get("repo")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let commit = known
        .get("commit")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    report.audited_repo = Some(repo.clone());
    report.audited_commit = Some(commit.clone());

    // Findings live in the judging source's cache directory.
    let finding_paths = match judging {
        Some((judging_id, _)) => {
            let root = cache_dir.join(&judging_id);
            if !root.is_dir() {
                report.blocked_reason = Some(format!(
                    "judging-cache-missing: the local cache has no {judging_id}"
                ));
                return report;
            }
            enumerate_findings(&root)
        }
        None => {
            report.blocked_reason = Some("lock has no judging source".to_string());
            return report;
        }
    };
    report.findings_total = finding_paths.len();

    // Read every finding once. Text-level counts do not need the audited
    // source, so a blocked corpus still reports its denominator.
    let mut texts: Vec<(String, Result<String, String>)> = Vec::with_capacity(finding_paths.len());
    for (id, path) in &finding_paths {
        let text = std::fs::read_to_string(path).map_err(|e| e.to_string());
        if text.as_ref().is_ok_and(|t| cites_code(t)) {
            report.findings_citing_code += 1;
        }
        texts.push((id.clone(), text));
    }

    let source = match find_checkout(
        &options.sources_root.join(name),
        &repo,
        &commit,
        options.allow_unverified_commit,
    ) {
        Ok(source) => source,
        Err(reason) => {
            report.blocked_reason = Some(reason);
            return report;
        }
    };
    report.source_checkout = Some(normalise_root(&source.path));
    report.commit_verified = source.verified;

    let index = match parse_repo(&source.path, &repo, &commit, &IndexOptions::default()) {
        Ok(index) => index,
        Err(reason) => {
            report.blocked_reason = Some(format!("index-failed: {reason}"));
            return report;
        }
    };
    report.files_indexed = Some(index.files.len());
    report.files_unparsed = Some(index.parse_failures.len());
    let index = IndexSet::single(index);

    let mut resolved = 0usize;
    let mut unresolved_by_reason: BTreeMap<String, usize> = BTreeMap::new();
    for (_, text) in &texts {
        let text = match text {
            Ok(text) => text,
            Err(_) => {
                *report
                    .abstention_citations_by_reason
                    .entry("io-error".to_string())
                    .or_default() += 1;
                continue;
            }
        };
        let set = resolve_text(text, &index);
        if !set.cites_code {
            continue;
        }
        if set.resolved() {
            resolved += 1;
            continue;
        }
        let mut reasons: Vec<&'static str> =
            set.abstentions.iter().map(|a| a.reason.as_str()).collect();
        reasons.sort();
        reasons.dedup();
        for reason in &reasons {
            *report
                .abstention_citations_by_reason
                .entry(reason.to_string())
                .or_default() += 1;
        }
        let dominant = set
            .abstentions
            .iter()
            .map(|a| a.reason)
            .max_by_key(|r| dominant_priority(*r))
            .map(|r| r.as_str())
            .unwrap_or("not-found");
        *unresolved_by_reason
            .entry(dominant.to_string())
            .or_default() += 1;
    }
    report.status = CorpusStatus::Measured;
    report.findings_resolved = Some(resolved);
    report.resolution_rate = Some(rate(resolved, report.findings_citing_code));
    report.unresolved_findings_by_reason = unresolved_by_reason;
    report
}

/// A finding-level reason ranking: a missing file explains more than a symbol
/// that did not resolve, so it is reported as the dominant reason.
fn dominant_priority(reason: crate::anchors::AbstainReason) -> u8 {
    use crate::anchors::AbstainReason::*;
    match reason {
        NoCitation => 0,
        IoError => 10,
        RepoUnavailable => 15,
        CommitMismatch => 20,
        NotFound => 30,
        Ambiguous => 40,
        LineOutOfRange => 50,
        ParseFailure => 60,
        FileMissing => 70,
    }
}

/// A verified (or explicitly allowed unverified) checkout.
struct SourceCheckout {
    path: PathBuf,
    verified: bool,
}

fn find_checkout(
    corpus_sources: &Path,
    repo: &str,
    commit: &str,
    allow_unverified: bool,
) -> Result<SourceCheckout, String> {
    let expected_name = repo.rsplit('/').next().unwrap_or(repo);
    let mut candidates: Vec<PathBuf> = Vec::new();
    if corpus_sources.is_dir() {
        if is_git_checkout(corpus_sources) {
            candidates.push(corpus_sources.to_path_buf());
        } else if allow_unverified && looks_like_snapshot(corpus_sources) {
            return Ok(SourceCheckout {
                path: corpus_sources.to_path_buf(),
                verified: false,
            });
        } else {
            let mut entries: Vec<PathBuf> = std::fs::read_dir(corpus_sources)
                .map_err(|e| format!("source-missing: cannot read sources root: {e}"))?
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.is_dir() && p.join(".git").exists())
                .collect();
            entries.sort();
            if entries.is_empty() {
                // A snapshot with no `.git` is still usable when the caller
                // explicitly accepts an unverified commit.
                if allow_unverified {
                    let mut snapshots: Vec<PathBuf> = std::fs::read_dir(corpus_sources)
                        .map_err(|e| format!("source-missing: cannot read sources root: {e}"))?
                        .filter_map(|e| e.ok())
                        .map(|e| e.path())
                        .filter(|p| p.is_dir())
                        .collect();
                    snapshots.sort();
                    if snapshots.len() == 1 {
                        return Ok(SourceCheckout {
                            path: snapshots.remove(0),
                            verified: false,
                        });
                    }
                }
            }
            candidates.extend(entries);
        }
    }
    if candidates.is_empty() {
        return Err(format!(
            "source-missing: no checkout of {repo}@{commit} under the corpus sources directory"
        ));
    }

    let mut unverified: Option<PathBuf> = None;
    let mut mismatch: Option<String> = None;
    let mut unverified_error: Option<String> = None;
    for candidate in &candidates {
        let head = git_head(candidate);
        let remote = git_remote(candidate).and_then(|url| owner_repo(&url));
        let name_matches = candidate
            .file_name()
            .map(|n| n.to_string_lossy().replace('-', "_") == expected_name.replace('-', "_"))
            .unwrap_or(false);
        let remote_matches = remote
            .as_deref()
            .is_some_and(|r| r == repo || r.ends_with(&format!("/{expected_name}")));
        let matches = if remote.is_some() {
            remote_matches
        } else {
            name_matches || candidates.len() == 1
        };
        if !matches {
            continue;
        }
        match head {
            Some(head) if head.eq_ignore_ascii_case(commit) => {
                return Ok(SourceCheckout {
                    path: candidate.clone(),
                    verified: true,
                });
            }
            Some(head) => {
                if allow_unverified {
                    unverified.get_or_insert_with(|| candidate.clone());
                } else {
                    mismatch.get_or_insert_with(|| {
                        format!("commit-mismatch: {repo} checkout is at {head}, expected {commit}")
                    });
                }
            }
            None => {
                if allow_unverified {
                    unverified.get_or_insert_with(|| candidate.clone());
                } else {
                    unverified_error.get_or_insert_with(|| {
                        format!(
                            "commit-unverified: {} is not a git checkout; pass --allow-unverified-commit only for a snapshot pinned by other means",
                            normalise_root(candidate)
                        )
                    });
                }
            }
        }
    }
    if let Some(path) = unverified {
        return Ok(SourceCheckout {
            path,
            verified: false,
        });
    }
    if let Some(reason) = mismatch {
        return Err(reason);
    }
    if let Some(reason) = unverified_error {
        return Err(reason);
    }
    Err(format!(
        "source-missing: no checkout of {repo}@{commit} under the corpus sources directory"
    ))
}

fn is_git_checkout(dir: &Path) -> bool {
    // `.git` is a directory in a normal clone and a file in a worktree; either
    // way its presence makes the directory itself the checkout. Without this
    // guard, `git -C` walks up to an enclosing repository and reports its HEAD.
    dir.join(".git").exists()
}

/// A source snapshot with no `.git` that still looks like a Solidity project.
fn looks_like_snapshot(dir: &Path) -> bool {
    dir.join("contracts").is_dir()
        || dir.join("src").is_dir()
        || dir.join("foundry.toml").is_file()
        || dir.join("hardhat.config.js").is_file()
        || dir.join("hardhat.config.ts").is_file()
}

fn git_head(dir: &Path) -> Option<String> {
    if !is_git_checkout(dir) {
        return None;
    }
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

fn git_remote(dir: &Path) -> Option<String> {
    if !is_git_checkout(dir) {
        return None;
    }
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["remote", "get-url", "origin"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

/// `https://github.com/o/r.git`, `git@github.com:o/r.git` -> `o/r`.
fn owner_repo(url: &str) -> Option<String> {
    let url = url.trim().trim_end_matches(".git");
    for prefix in [
        "https://github.com/",
        "http://github.com/",
        "git@github.com:",
        "ssh://git@github.com/",
    ] {
        if let Some(rest) = url.strip_prefix(prefix) {
            return Some(rest.trim_end_matches('/').replace("//", "/"));
        }
    }
    None
}

/// One finding per accepted directory and per invalid file or group, exactly
/// as `eval/benchmark/fetch.py` enumerates the judged units.
fn enumerate_findings(root: &Path) -> Vec<(String, PathBuf)> {
    let mut out = Vec::new();
    let mut dirs: Vec<PathBuf> = match std::fs::read_dir(root) {
        Ok(entries) => entries
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect(),
        Err(_) => return out,
    };
    dirs.sort();
    for dir in dirs {
        let name = dir
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let is_accepted = name.len() == 5
            && name.as_bytes()[3] == b'-'
            && (name.ends_with("-H") || name.ends_with("-M"))
            && name.as_bytes()[..3].iter().all(u8::is_ascii_digit);
        if is_accepted {
            if let Some(rep) = representative(&dir) {
                out.push((format!("acc:{name}"), rep));
            }
        } else if name == "invalid" {
            let mut files = markdown_files(&dir);
            files.sort();
            for file in files {
                let stem = file
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default();
                out.push((format!("inv:{stem}"), file));
            }
        } else if name.starts_with("invalid-") {
            if let Some(rep) = representative(&dir) {
                out.push((format!("inv:{name}"), rep));
            }
        }
    }
    out
}

fn representative(dir: &Path) -> Option<PathBuf> {
    let mut files = markdown_files(dir);
    files.sort();
    files
        .iter()
        .find(|p| {
            p.file_name()
                .map(|n| n.to_string_lossy().ends_with("-best.md"))
                .unwrap_or(false)
        })
        .cloned()
        .or_else(|| files.into_iter().next())
}

fn markdown_files(dir: &Path) -> Vec<PathBuf> {
    match std::fs::read_dir(dir) {
        Ok(entries) => entries
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.is_file() && p.extension().and_then(|e| e.to_str()) == Some("md"))
            .collect(),
        Err(_) => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn bench_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/anchors-bench")
    }

    fn fixture() -> BenchReport {
        let root = bench_root();
        let options = BenchOptions {
            benchmark_root: root.join("benchmark"),
            cache_root: root.join("cache"),
            sources_root: root.join("sources"),
            allow_unverified_commit: true,
            corpora: vec!["tiny".to_string()],
        };
        bench(&options).expect("tiny corpus measures")
    }

    #[test]
    fn a_blocked_corpus_reports_its_denominator_but_no_rate() {
        let root = bench_root();
        let options = BenchOptions {
            benchmark_root: root.join("benchmark"),
            cache_root: root.join("cache"),
            sources_root: root.join("missing-sources"),
            allow_unverified_commit: true,
            corpora: vec!["tiny".to_string()],
        };
        let report = bench(&options).expect("blocked corpus still reports");
        let row = &report.corpora[0];
        assert_eq!(row.status, CorpusStatus::Blocked);
        assert_eq!(row.findings_total, 2);
        assert_eq!(row.findings_citing_code, 1);
        assert_eq!(row.findings_resolved, None);
        assert!(row.resolution_rate.is_none());
        assert!(report.pooled.is_none());
    }

    #[test]
    fn a_measured_corpus_reports_k_over_n() {
        let report = fixture();
        let row = &report.corpora[0];
        assert_eq!(row.status, CorpusStatus::Measured, "{row:#?}");
        assert_eq!(row.findings_total, 2);
        assert_eq!(row.findings_citing_code, 1);
        assert_eq!(row.findings_resolved, Some(1));
        let rate = row.resolution_rate.as_ref().expect("rate");
        assert_eq!((rate.resolved, rate.citing), (1, 1));
        assert_eq!(rate.point, 1.0);
        assert!(rate.wilson.is_none(), "n < 5 has no interval");
    }

    #[test]
    fn wilson_matches_the_known_value() {
        // 90 of 100 under the standard Wilson score interval: 82.56% - 94.48%.
        let [lo, hi] = wilson(90, 100).expect("n >= 5");
        assert!((lo - 0.8256).abs() < 0.001, "{lo}");
        assert!((hi - 0.9448).abs() < 0.001, "{hi}");
        assert!(wilson(1, 4).is_none());
    }

    #[test]
    fn owner_repo_parses_the_common_remote_forms() {
        for url in [
            "https://github.com/a/b.git",
            "git@github.com:a/b.git",
            "ssh://git@github.com/a/b",
        ] {
            assert_eq!(owner_repo(url).as_deref(), Some("a/b"), "{url}");
        }
    }

    #[test]
    fn the_report_is_deterministic() {
        let a = fixture();
        let b = fixture();
        assert_eq!(
            serde_json::to_vec(&a).expect("a"),
            serde_json::to_vec(&b).expect("b")
        );
    }
}
