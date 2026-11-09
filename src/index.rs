//! Duplicate / prior-finding matching for gate D1.
//!
//! Word overlap does not work. The author measured Jaccard between a rejection
//! description and the report it describes at about 0.024. This matcher scores
//! code identifiers (contract, function, and variable names, and file paths)
//! plus a small root-cause lexicon. Two shared strong identifiers are required
//! before a row is even reported. Prose that shares only ordinary words scores
//! nothing.
//!
//! The fail threshold is a stated heuristic. It has not been calibrated against
//! the author's duplicate log, and the docs say so. A hit is a prompt to read
//! the quoted lines, not a determination that the findings are the same bug.

use crate::model::IndexedFinding;
use regex::Regex;
use std::collections::BTreeSet;
use std::sync::LazyLock;

/// Score at or above this, with at least [`MIN_SHARED_IDENTIFIERS`] shared
/// strong identifiers, fails D1. Uncalibrated. See docs/duplicate-matching.md.
pub const FAIL_SCORE: f64 = 0.50;

/// Minimum shared strong identifiers before a prior finding is reported at all.
pub const MIN_SHARED_IDENTIFIERS: usize = 2;

/// How many ranked rows `vg match` and the gate detail show.
pub const TOP_N: usize = 5;

/// Language keywords and names that appear in almost every report. They are
/// not evidence that two findings share a root cause.
const STOP_IDENTS: &[&str] = &[
    "function",
    "mapping",
    "address",
    "uint256",
    "uint128",
    "uint64",
    "bytes32",
    "string",
    "memory",
    "calldata",
    "external",
    "internal",
    "public",
    "private",
    "view",
    "pure",
    "returns",
    "return",
    "require",
    "assert",
    "revert",
    "import",
    "pragma",
    "contract",
    "struct",
    "event",
    "error",
    "modifier",
    "constructor",
    "receive",
    "fallback",
    "override",
    "virtual",
    "constant",
    "immutable",
    "storage",
    "call",
    "staticcall",
    "delegatecall",
    "msg",
    "sender",
    "value",
    "this",
    "super",
    "true",
    "false",
    "none",
    "self",
    "uint",
    "int",
    "bytes",
    "bool",
    "from",
    "into",
    "data",
    "result",
    "amount",
    "owner",
    "user",
    "admin",
    "token",
    "vault",
    "pool",
    "state",
    "total",
    "current",
    "index",
    "value",
    "public",
    "fun",
    "mut",
    "let",
    "const",
    "type",
    "impl",
    "trait",
    "struct",
    "enum",
    "module",
    "move",
    "script",
    "test",
    "tests",
    "src",
    "lib",
];

/// Root-cause stems. They never match on their own. They only break ties once
/// two strong identifiers already overlap.
const ROOT_CAUSES: &[&str] = &[
    "reentrancy",
    "overflow",
    "underflow",
    "rounding",
    "precision",
    "truncation",
    "slippage",
    "deadline",
    "oracle",
    "stale",
    "staleness",
    "signature",
    "replay",
    "authorization",
    "unchecked",
    "inflation",
    "dilution",
    "liquidation",
    "accounting",
    "refund",
    "invariant",
    "callback",
    "nonce",
    "grief",
    "griefing",
    "freeze",
    "collateral",
    "allowance",
    "permit",
    "sandwich",
    "frontrun",
    "twap",
    "netting",
    "outflow",
    "segment",
    "share",
    "fee",
    "reward",
];

static PATH_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(?:[A-Za-z0-9_.\-]+/)+[A-Za-z0-9_\-][A-Za-z0-9_.\-]*\.(?:sol|move|rs|cairo|vy|go|ts|js|md|txt)\b",
    )
    .expect("path")
});
static QUAL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b[A-Za-z_][A-Za-z0-9_]*::[A-Za-z_][A-Za-z0-9_]*\b").expect("qual")
});
static SNAKE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b[A-Za-z][A-Za-z0-9]*(?:_[A-Za-z0-9]+)+\b").expect("snake"));
static CAMEL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b[A-Z][a-z0-9]+(?:[A-Z][a-z0-9]+)+\b").expect("camel"));
static TICK_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"`([^`]{1,80})`").expect("tick"));
static CALL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b([A-Za-z_][A-Za-z0-9_]{4,})\s*\(").expect("call"));

#[derive(Debug, Clone, PartialEq)]
pub struct Profile {
    pub strong: BTreeSet<String>,
    pub roots: BTreeSet<String>,
}

#[derive(Debug, Clone)]
pub struct Ranked<'a> {
    pub finding: &'a IndexedFinding,
    pub score: f64,
    pub shared_identifiers: Vec<String>,
    pub shared_roots: Vec<String>,
    pub quotes: Vec<String>,
    /// At or above [`FAIL_SCORE`]. Still a heuristic, not a verdict.
    pub fails: bool,
}

impl Profile {
    pub fn from_text(text: &str) -> Self {
        let mut strong = BTreeSet::new();
        for raw in candidates(text) {
            if let Some(id) = normalize_ident(&raw) {
                strong.insert(id);
            }
        }
        let roots = root_causes(text);
        Self { strong, roots }
    }

    pub fn from_parts(identifiers: &[String], roots: &[String]) -> Self {
        let mut strong = BTreeSet::new();
        for id in identifiers {
            if let Some(n) = normalize_ident(id) {
                strong.insert(n);
            }
        }
        let mut root_set = BTreeSet::new();
        for r in roots {
            let n = r.to_lowercase();
            if ROOT_CAUSES.contains(&n.as_str()) {
                root_set.insert(n);
            }
        }
        Self {
            strong,
            roots: root_set,
        }
    }
}

fn normalize_ident(raw: &str) -> Option<String> {
    let t = raw
        .trim()
        .trim_matches(|c: char| c == '`' || c == '(' || c == ')' || c == ',');
    if t.is_empty() || t.chars().count() < 5 {
        return None;
    }
    // Paths: keep the file stem and the path, both lowercased.
    if t.contains('/')
        || t.contains('.')
            && t.split('.')
                .next_back()
                .is_some_and(|e| e.len() <= 5 && t.contains('.'))
    {
        let low = t.replace('\\', "/").to_lowercase();
        if STOP_IDENTS.contains(&low.as_str()) {
            return None;
        }
        return Some(low);
    }
    let low = t.to_lowercase();
    if STOP_IDENTS.contains(&low.as_str()) {
        return None;
    }
    let strong = low.contains('_')
        || low.contains("::")
        || t.chars().any(|c| c.is_uppercase()) && t.chars().any(|c| c.is_lowercase())
        || low.chars().count() >= 14;
    strong.then_some(low)
}

fn candidates(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for m in TICK_RE.find_iter(text) {
        let inner = m.as_str().trim_matches('`');
        for part in inner.split(|c: char| c.is_whitespace() || c == ',' || c == '(' || c == ')') {
            if !part.is_empty() {
                out.push(part.to_string());
            }
        }
    }
    for m in PATH_RE.find_iter(text) {
        out.push(m.as_str().to_string());
        if let Some(stem) = m.as_str().rsplit(['/', '\\']).next() {
            out.push(stem.to_string());
        }
    }
    for m in QUAL_RE.find_iter(text) {
        let s = m.as_str();
        out.push(s.to_string());
        for part in s.split("::") {
            out.push(part.to_string());
        }
    }
    for m in SNAKE_RE.find_iter(text) {
        out.push(m.as_str().to_string());
    }
    for m in CAMEL_RE.find_iter(text) {
        out.push(m.as_str().to_string());
    }
    for m in CALL_RE.captures_iter(text) {
        out.push(m.get(1).unwrap().as_str().to_string());
    }
    out
}

fn root_causes(text: &str) -> BTreeSet<String> {
    let low = text.to_lowercase();
    ROOT_CAUSES
        .iter()
        .filter(|w| low.split(|c: char| !c.is_alphanumeric()).any(|t| t == **w))
        .map(|w| (*w).to_string())
        .collect()
}

/// Sentences worth quoting: those that name a strong identifier.
pub fn evidence_quotes(text: &str, profile: &Profile) -> Vec<String> {
    let mut quotes = Vec::new();
    for sentence in text.split(['\n', '.', ';']) {
        let s = collapse(sentence);
        if s.chars().count() < 20 {
            continue;
        }
        let low = s.to_lowercase();
        if profile.strong.iter().any(|id| low.contains(id)) {
            quotes.push(s);
        }
        if quotes.len() == 5 {
            break;
        }
    }
    quotes
}

pub fn collapse(s: &str) -> String {
    let one = s.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut out: String = one.chars().take(220).collect();
    if one.chars().count() > 220 {
        out.push('…');
    }
    out
}

/// Build one index row from a whole document. `id` is the caller's stable name.
pub fn index_document(id: &str, source: &str, text: &str) -> IndexedFinding {
    let profile = Profile::from_text(text);
    let title = text
        .lines()
        .find(|l| !l.trim().is_empty())
        .map(|l| collapse(l.trim_start_matches('#')))
        .unwrap_or_else(|| id.to_string());
    let quotes = evidence_quotes(text, &profile);
    IndexedFinding {
        id: id.to_string(),
        source: source.to_string(),
        title,
        identifiers: profile.strong.iter().cloned().collect(),
        root_causes: profile.roots.iter().cloned().collect(),
        quotes,
    }
}

fn profile_of(f: &IndexedFinding) -> Profile {
    Profile::from_parts(&f.identifiers, &f.root_causes)
}

/// Rank `text` against the index. Only rows with at least two shared strong
/// identifiers are returned, best score first, then id, at most [`TOP_N`].
pub fn rank<'a>(text: &str, index: &'a [IndexedFinding]) -> Vec<Ranked<'a>> {
    let mine = Profile::from_text(text);
    if mine.strong.len() < MIN_SHARED_IDENTIFIERS {
        return Vec::new();
    }
    let mut rows: Vec<Ranked<'a>> = Vec::new();
    for finding in index {
        let theirs = profile_of(finding);
        let shared_identifiers: Vec<String> =
            mine.strong.intersection(&theirs.strong).cloned().collect();
        if shared_identifiers.len() < MIN_SHARED_IDENTIFIERS {
            continue;
        }
        let shared_roots: Vec<String> = mine.roots.intersection(&theirs.roots).cloned().collect();
        let denom = mine.strong.len().min(theirs.strong.len()).max(1) as f64;
        let id_c = shared_identifiers.len() as f64 / denom;
        let root_c = if mine.roots.is_empty() || theirs.roots.is_empty() {
            0.0
        } else {
            shared_roots.len() as f64 / (mine.roots.len().min(theirs.roots.len()) as f64)
        };
        let score = (0.75 * id_c + 0.25 * root_c).min(1.0);
        let mut quotes: Vec<String> = finding
            .quotes
            .iter()
            .filter(|q| {
                let low = q.to_lowercase();
                shared_identifiers.iter().any(|id| low.contains(id))
            })
            .cloned()
            .collect();
        if quotes.is_empty() {
            quotes.extend(finding.quotes.iter().take(1).cloned());
        }
        quotes.truncate(2);
        rows.push(Ranked {
            finding,
            score,
            shared_identifiers,
            shared_roots,
            quotes,
            fails: score >= FAIL_SCORE,
        });
    }
    rows.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.finding.id.cmp(&b.finding.id))
    });
    rows.truncate(TOP_N);
    rows
}

/// One line a gate detail or `vg match` can print.
pub fn summary(row: &Ranked<'_>) -> String {
    let mark = if row.fails { "FAIL" } else { "near" };
    let ids = row.shared_identifiers.join(", ");
    let quote = row.quotes.first().map(|s| s.as_str()).unwrap_or("");
    format!(
        "{mark} {score:.2} {id} shared [{ids}] \"{quote}\"",
        score = row.score,
        id = row.finding.id,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(id: &str, text: &str) -> IndexedFinding {
        index_document(id, id, text)
    }

    #[test]
    fn shared_function_names_rank_and_plain_words_do_not() {
        let prior = doc(
            "m-1",
            "# Limiter refund\n\n`limiter::reduce_outflow` credits only the current segment, so a refund that crosses the hour boundary is destroyed. `add_outflow` then aborts.",
        );
        let draft = "The same defect: `reduce_outflow` in limiter.move destroys the refund, and `add_outflow` still sums every segment. The attacker cycles the outflow.";
        let index = [prior];
        let hits = rank(draft, &index);
        assert_eq!(hits.len(), 1, "expected an identifier hit");
        assert!(hits[0]
            .shared_identifiers
            .iter()
            .any(|s| s.contains("reduce_outflow")));
        assert!(hits[0].fails, "score {}", hits[0].score);
        assert!(!hits[0].quotes.is_empty());

        let prose_a = "The attacker can withdraw funds from the vault and the user loses money because the protocol does not check the cached total before it updates the share price for the next depositor in the same block.";
        let prose_b = doc(
            "other",
            "The user loses money because an attacker can withdraw funds from the vault when the protocol does not check the cached total before the share price updates for the next depositor.",
        );
        let only_words = [prose_b];
        assert!(
            rank(prose_a, &only_words).is_empty(),
            "plain word overlap must not rank"
        );
    }

    #[test]
    fn two_long_reports_that_only_share_english_score_nothing() {
        let a = "In this report the attacker describes a detailed sequence of ordinary deposits and withdrawals over many paragraphs, and the victim is any depositor who happens to transact during the window, and the impact is a loss of funds that the author believes is serious, and the recommendation is to add a check. ".repeat(8);
        let b = doc("b", &a.replace("attacker", "caller"));
        let index = [b];
        assert!(rank(&a, &index).is_empty());
    }
}
