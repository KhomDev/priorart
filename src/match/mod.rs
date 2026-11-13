//! Identifier matching, and the lexical gates that read a finding.
//!
//! Word overlap does not work. The measured Jaccard between a rejection
//! description and the report it describes is about 0.024, so this matcher
//! scores code identifiers (contract, function and error names, file paths)
//! and requires at least two shared strong identifiers before a prior item is
//! reported at all. Prose that shares only ordinary words scores nothing.
//!
//! The threshold is a stated heuristic, calibrated on the public corpus. The
//! measured false-alarm and recall numbers, the three matcher revisions and the
//! resulting 0.1.0 ship decision are in docs/benchmark.md and are deliberately
//! not repeated here, so there is one place to update and one place to be wrong.

use crate::model::Item;
use regex::Regex;
use std::collections::BTreeSet;
use std::sync::LazyLock;

/// Minimum shared strong identifiers before a prior item is reported.
pub const MIN_SHARED_IDENTIFIERS: usize = 2;

/// How many ranked prior items the text output shows by default.
pub const DEFAULT_TOP_K: usize = 5;

/// The largest --top-k accepted; larger values are a usage error.
pub const MAX_TOP_K: usize = 50;

/// A symbol that appears in more than this share of dossier items is treated
/// as generic and never counts toward the threshold. An identifier that
/// appears once is always specific, however small the dossier.
pub const GENERIC_DF_PERCENT: usize = 20;

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
    "fun",
    "mut",
    "let",
    "const",
    "type",
    "impl",
    "trait",
    "enum",
    "module",
    "move",
    "script",
    "test",
    "tests",
    "src",
    "lib",
];

/// Root-cause stems. They never match on their own; they only appear in the
/// comparison beside identifiers that already overlap.
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

// The inline-code delimiter is written as \x60 so this source file contains no
// raw backtick that a shell or an editor wrapper could mangle.
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
static TICK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\x60([^\x60]{1,80})\x60").expect("tick"));
static CALL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b([A-Za-z_][A-Za-z0-9_]{4,})\s*\(").expect("call"));
static URL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b[a-z][a-z0-9+.-]*://\S+").expect("url"));
static FILE_PATH_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:[A-Za-z0-9_.\-]+/)*[A-Za-z0-9_\-][A-Za-z0-9_.\-]*\.[A-Za-z]{2,5}\b")
        .expect("file path")
});
static SEVERITY_LINE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?im)^\s*(?:#{1,6}\s*)?(?:[-*]\s+)?(?:\*\*|__)?\s*severity\s*(?:\*\*|__)?\s*[:\-\u{2013}|]\s*(?:\*\*|__)?\s*(critical|high|medium|low|informational|info)\b",
    )
    .expect("severity line")
});
static SEVERITY_ROW: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\|\s*severity\s*\|\s*(?:\*\*)?\s*(critical|high|medium|low|informational|info)\b",
    )
    .expect("severity row")
});

/// A severity claimed by a finding, ordered so the floor comparison is total.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Informational,
    Low,
    Medium,
    High,
    Critical,
}

impl Severity {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "critical" | "crit" => Some(Severity::Critical),
            "high" => Some(Severity::High),
            "medium" | "med" => Some(Severity::Medium),
            "low" => Some(Severity::Low),
            "informational" | "info" | "none" => Some(Severity::Informational),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Severity::Critical => "critical",
            Severity::High => "high",
            Severity::Medium => "medium",
            Severity::Low => "low",
            Severity::Informational => "informational",
        }
    }

    /// Title case, for prose. The JSON always uses label().
    pub fn display(self) -> &'static str {
        match self {
            Severity::Critical => "Critical",
            Severity::High => "High",
            Severity::Medium => "Medium",
            Severity::Low => "Low",
            Severity::Informational => "Informational",
        }
    }
}

/// The severity the finding claims, from a "Severity: High" line or a
/// "| Severity | High |" table row. Never inferred from prose.
pub fn claimed_severity(text: &str) -> Option<Severity> {
    SEVERITY_LINE
        .captures(text)
        .or_else(|| SEVERITY_ROW.captures(text))
        .and_then(|c| Severity::parse(&c[1]))
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Profile {
    /// Symbol identifiers, lowercased for comparison: functions, errors,
    /// contracts, qualified names, backticked spans. These are what the >=2
    /// threshold counts.
    pub symbols: BTreeSet<String>,
    /// File paths, lowercased. Shown as shared evidence and used to break ties,
    /// but never enough on their own: in the public benchmark, path-only matches
    /// grouped unrelated findings that merely lived in the same repository.
    pub paths: BTreeSet<String>,
    pub roots: BTreeSet<String>,
    /// The identifier as it was written, keyed by its lowercased form, so a
    /// dossier stores FeeLedger rather than feeledger and a rebuilt
    /// profile still recognises it as a symbol.
    display: std::collections::BTreeMap<String, String>,
}

/// One extracted identifier: key for comparison, original for display.
struct Ident {
    key: String,
    original: String,
    path: bool,
}

impl Profile {
    pub fn from_text(text: &str) -> Self {
        let mut profile = Self::default();
        for raw in candidates(text) {
            if let Some(ident) = classify_ident(&raw) {
                profile.insert(ident);
            }
        }
        profile.roots = root_causes(text);
        profile
    }

    pub fn from_parts(identifiers: &[String], roots: &[String]) -> Self {
        let mut profile = Self::default();
        for id in identifiers {
            if let Some(ident) = classify_ident(id) {
                profile.insert(ident);
            }
        }
        for r in roots {
            let n = r.to_lowercase();
            if ROOT_CAUSES.contains(&n.as_str()) {
                profile.roots.insert(n);
            }
        }
        profile
    }

    fn insert(&mut self, ident: Ident) {
        self.display
            .entry(ident.key.clone())
            .or_insert(ident.original);
        if ident.path {
            self.paths.insert(ident.key);
        } else {
            self.symbols.insert(ident.key);
        }
    }

    /// Every identifier, symbols first, in the form it was written.
    pub fn all_identifiers(&self) -> Vec<String> {
        let render = |set: &BTreeSet<String>| -> Vec<String> {
            set.iter()
                .map(|key| {
                    self.display
                        .get(key)
                        .cloned()
                        .unwrap_or_else(|| key.clone())
                })
                .collect()
        };
        let mut out = render(&self.symbols);
        out.extend(render(&self.paths));
        out
    }
}

/// Is this identifier a file path rather than a symbol?
fn is_path_ident(id: &str) -> bool {
    if id.contains('/') {
        return true;
    }
    const SOURCE_EXTS: [&str; 7] = ["sol", "move", "rs", "cairo", "vy", "go", "ts"];
    id.rsplit('.')
        .next()
        .is_some_and(|ext| id.contains('.') && SOURCE_EXTS.contains(&ext))
}

/// Decide whether a raw candidate is a strong identifier, and which class.
///
/// Strength is decided on the text as written, before lowercasing: a
/// 12-character camelCase contract name is a symbol, and lowercasing it first
/// would make it look like an ordinary word.
fn classify_ident(raw: &str) -> Option<Ident> {
    let t = raw
        .trim()
        .trim_matches(|c: char| c == '\u{60}' || c == '(' || c == ')' || c == ',');
    if t.is_empty() || t.chars().count() < 5 {
        return None;
    }
    let key = t.replace('\\', "/").to_lowercase();
    if STOP_IDENTS.contains(&key.as_str()) {
        return None;
    }
    let path = is_path_ident(&key);
    let strong = path
        || key.contains('_')
        || (t.chars().any(|c| c.is_uppercase()) && t.chars().any(|c| c.is_lowercase()))
        || key.chars().count() >= 14;
    strong.then(|| Ident {
        key,
        original: t.to_string(),
        path,
    })
}

fn candidates(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for m in TICK_RE.find_iter(text) {
        let inner = m.as_str().trim_matches('\u{60}');
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
        out.push(m.as_str().to_string());
    }
    for m in SNAKE_RE.find_iter(text) {
        out.push(m.as_str().to_string());
    }
    for m in CAMEL_RE.find_iter(text) {
        out.push(m.as_str().to_string());
    }
    for m in CALL_RE.captures_iter(text) {
        if let Some(g) = m.get(1) {
            out.push(g.as_str().to_string());
        }
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

/// Collapse whitespace and cap at a length, so a quote stays quotable.
pub fn collapse(s: &str) -> String {
    let one = s.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut out: String = one.chars().take(220).collect();
    if one.chars().count() > 220 {
        out.push('\u{2026}');
    }
    out
}

/// A quote may run to this many characters before it is windowed instead.
pub const MAX_QUOTE_CHARS: usize = 320;

/// Words that flip the meaning of a match when they precede it in its clause.
const NEGATION_CUES: [&str; 16] = [
    "no", "not", "never", "without", "cannot", "nor", "neither", "none", "zero", "non", "isn",
    "aren", "wasn", "weren", "doesn", "didn",
];

/// Negation that follows a match: "X is not used", "X isn't reachable". Only
/// explicit not-forms count here, so "stale but there is no pause" does not
/// negate "stale".
const POST_NEGATION_CUES: [&str; 10] = [
    "not", "never", "without", "cannot", "isn", "aren", "wasn", "weren", "doesn", "didn",
];

/// The clause fragment before a match, at most 24 characters, stopping at
/// clause punctuation.
fn before_window(low: &str, start: usize) -> &str {
    let mut left = start;
    for (i, c) in low[..start].char_indices().rev() {
        if start - i > 24 || is_clause_break(c) {
            break;
        }
        left = i;
    }
    &low[left..start]
}

/// The clause fragment after a match, at most 16 characters, stopping at
/// clause punctuation.
fn after_window(low: &str, end: usize) -> &str {
    let mut right = end;
    for (i, c) in low[end..].char_indices() {
        if i > 16 || is_clause_break(c) {
            break;
        }
        right = end + i;
    }
    &low[end..right]
}

fn tokens(window: &str) -> impl Iterator<Item = String> + '_ {
    window
        .split(|c: char| !c.is_alphanumeric() && c != '\'')
        .map(|token| token.to_lowercase())
}

fn is_clause_break(c: char) -> bool {
    matches!(c, '.' | ';' | ',' | ':' | '!' | '?')
}

/// Is the match at [start, end) negated by a cue in its own clause?
///
/// Cues before the match count, and so do explicit not-forms after it. The
/// window is deliberately narrow: "no pause" in the same sentence must not
/// negate an oracle claim three words later.
pub fn negated_near(low: &str, start: usize, end: usize) -> bool {
    if start > low.len()
        || end > low.len()
        || !low.is_char_boundary(start)
        || !low.is_char_boundary(end)
    {
        return false;
    }
    let before = before_window(low, start);
    if tokens(before).any(|token| NEGATION_CUES.contains(&token.as_str())) {
        return true;
    }
    let after = after_window(low, end);
    tokens(after).any(|token| POST_NEGATION_CUES.contains(&token.as_str()))
}

/// Does the needle appear at least once without a negation cue beside it?
pub fn contains_unnegated(haystack_low: &str, needle_low: &str) -> bool {
    if needle_low.is_empty() {
        return false;
    }
    let mut from = 0usize;
    while let Some(rel) = haystack_low[from..].find(needle_low) {
        let start = from + rel;
        let end = start + needle_low.len();
        if !negated_near(haystack_low, start, end) {
            return true;
        }
        from = start + needle_low.len();
        if from >= haystack_low.len() {
            break;
        }
    }
    false
}

/// The first unnegated match of a regex, lowercased.
pub fn find_unnegated(re: &Regex, text: &str) -> Option<String> {
    let low = text.to_lowercase();
    // Match against the lowercased text so the offsets used for the negation
    // window are always on its char boundaries.
    for m in re.find_iter(&low) {
        if !negated_near(&low, m.start(), m.end()) {
            return Some(m.as_str().to_string());
        }
    }
    None
}

/// The excerpt a dossier item quotes.
///
/// The quote is the sentence that names a strong identifier, returned whole. A
/// sentence longer than MAX_QUOTE_CHARS is windowed around the identifier at a
/// word boundary with ellipses, never cut at a fixed length mid-clause. A
/// period inside an identifier (FeeLedger.distributeFees) is not a
/// sentence boundary.
pub fn evidence_quote(text: &str) -> String {
    let profile = Profile::from_text(text);
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let sentences = sentences(&flat);
    let (candidate, anchor) = sentences
        .iter()
        .find_map(|s| {
            let low = s.to_lowercase();
            profile
                .symbols
                .iter()
                .find(|id| low.contains(id.as_str()))
                .map(|id| (*s, low.find(id.as_str())))
        })
        .or_else(|| {
            sentences
                .iter()
                .find(|s| s.chars().count() >= 20)
                .map(|s| (*s, None))
        })
        .unwrap_or((flat.as_str(), None));
    if candidate.chars().count() <= MAX_QUOTE_CHARS {
        return candidate.trim().to_string();
    }
    let chars: Vec<char> = candidate.chars().collect();
    let anchor = anchor.unwrap_or(0).min(chars.len());
    let mut from = anchor.saturating_sub(120);
    while from > 0 && !chars[from - 1].is_whitespace() {
        from -= 1;
    }
    let mut to = (anchor + 200).min(chars.len());
    while to < chars.len() && !chars[to].is_whitespace() {
        to += 1;
    }
    let mut out = String::new();
    if from > 0 {
        out.push('\u{2026}');
    }
    out.extend(&chars[from..to]);
    if to < chars.len() {
        out.push('\u{2026}');
    }
    out
}

/// Split text into sentences. A terminator only ends a sentence when it is
/// followed by whitespace or the end of the text, so identifiers containing
/// dots stay intact.
fn sentences(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let bytes = text.as_bytes();
    let mut start = 0usize;
    let mut i = 0usize;
    while i < bytes.len() {
        if matches!(bytes[i], b'.' | b'!' | b'?') {
            let next_is_break = i + 1 >= bytes.len() || bytes[i + 1].is_ascii_whitespace();
            if next_is_break {
                let piece = text[start..=i].trim();
                if !piece.is_empty() {
                    out.push(piece);
                }
                start = i + 1;
            }
        }
        i += 1;
    }
    let tail = text[start.min(text.len())..].trim();
    if !tail.is_empty() {
        out.push(tail);
    }
    out
}

/// Path-like tokens in a text, normalised to forward slashes, URLs excluded.
pub fn extract_paths(text: &str) -> Vec<String> {
    let no_urls = URL_RE.replace_all(text, " ");
    let mut out: Vec<String> = Vec::new();
    for m in FILE_PATH_RE.find_iter(&no_urls) {
        let p = m.as_str().trim_start_matches("./").replace('\\', "/");
        if !p.contains('/') {
            let ext = p.rsplit('.').next().unwrap_or("");
            if !matches!(ext, "sol" | "move" | "rs" | "cairo" | "vy" | "go") {
                continue;
            }
        }
        if !out.contains(&p) {
            out.push(p);
        }
    }
    out
}

fn glob_match(pattern: &str, path: &str) -> bool {
    let mut re = String::from("^");
    let mut chars = pattern.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '*' if chars.peek() == Some(&'*') => {
                chars.next();
                re.push_str(".*");
            }
            '*' => re.push_str("[^/]*"),
            '?' => re.push_str("[^/]"),
            other => re.push_str(&regex::escape(&other.to_string())),
        }
    }
    re.push('$');
    Regex::new(&re).is_ok_and(|r| r.is_match(path))
}

/// Is the cited path inside the scope entry?
pub fn in_scope(path: &str, entry: &str) -> bool {
    let p = path.trim_start_matches("./");
    let s = entry.trim().trim_start_matches("./").replace('\\', "/");
    if s.is_empty() {
        return false;
    }
    if s.contains('*') || s.contains('?') {
        return glob_match(&s, p) || glob_match(&format!("**/{s}"), p);
    }
    if s.ends_with('/') {
        return p.starts_with(&s) || p.contains(&format!("/{s}"));
    }
    p == s || p.ends_with(&format!("/{s}")) || s.ends_with(&format!("/{p}"))
}

/// Does the text contain this word, case-insensitive, as a whole word?
pub fn mentions_word(text: &str, word: &str) -> bool {
    if word.chars().count() < 3 {
        return false;
    }
    Regex::new(&format!(r"(?i)\b{}\b", regex::escape(word))).is_ok_and(|r| r.is_match(text))
}

/// The file stem of a scope or coverage entry (LoanBook for
/// src/LoanBook.sol), or None when it is too short to identify anything.
pub fn file_stem(entry: &str) -> Option<&str> {
    let name = entry.trim().rsplit(['/', '\\']).next()?;
    let stem = name.split('.').next()?;
    (stem.chars().count() >= 3 && !stem.contains('*')).then_some(stem)
}

/// A ranked prior item.
#[derive(Debug, Clone)]
pub struct Ranked {
    pub index: usize,
    pub shared_identifiers: Vec<String>,
    pub shared_symbols: usize,
    pub specific_symbols: usize,
    pub shared_paths: usize,
    pub shared_roots: Vec<String>,
    /// Sum of (items - df + 1) over the shared specific symbols: higher means
    /// the overlap rests on rarer names.
    pub rarity: u64,
}

/// Rank dossier items against the finding. Only items with at least
/// MIN_SHARED_IDENTIFIERS shared strong identifiers are returned; the sort is
/// shared-identifier count descending, then item order ascending (which is the
/// dossier's stable document order). Word overlap is never scored.
pub fn rank(items: &[Item], finding_text: &str, top_k: usize) -> Vec<Ranked> {
    let mine = Profile::from_text(finding_text);
    if mine.symbols.len() < MIN_SHARED_IDENTIFIERS {
        return Vec::new();
    }
    // Document frequency of every symbol across the dossier, so an identifier
    // that appears in most items (a shared file's only contract, a common
    // helper) cannot carry a match on its own.
    let total = items.len();
    let mut df: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for item in items {
        let theirs = Profile::from_parts(&item.identifiers, &[]);
        for symbol in &theirs.symbols {
            *df.entry(symbol.clone()).or_insert(0) += 1;
        }
    }
    let is_specific = |id: &str| -> bool {
        let count = df.get(id).copied().unwrap_or(0);
        // A symbol named by at most two items is rare in absolute terms, which
        // keeps the rule usable on small dossiers (an audit plus its index).
        // This is the specificity floor the 0.1.0 benchmark was measured on;
        // it is not tuned after seeing the deciding corpus.
        count <= 2 || count * 100 <= total * GENERIC_DF_PERCENT
    };
    let mut rows: Vec<Ranked> = Vec::new();
    for (index, item) in items.iter().enumerate() {
        let theirs = Profile::from_parts(&item.identifiers, &[]);
        let shared_symbols: Vec<String> = mine
            .symbols
            .intersection(&theirs.symbols)
            .cloned()
            .collect();
        if shared_symbols.len() < MIN_SHARED_IDENTIFIERS {
            continue;
        }
        let specific: Vec<&String> = shared_symbols.iter().filter(|id| is_specific(id)).collect();
        if specific.len() < MIN_SHARED_IDENTIFIERS {
            continue;
        }
        let rarity: u64 = specific
            .iter()
            .map(|id| (total + 1 - df.get(id.as_str()).copied().unwrap_or(0)) as u64)
            .sum();
        let shared_paths: Vec<String> = mine.paths.intersection(&theirs.paths).cloned().collect();
        let mut shared: Vec<String> = specific.iter().map(|s| (*s).clone()).collect();
        shared.extend(
            shared_symbols
                .iter()
                .filter(|id| !specific.contains(id))
                .cloned(),
        );
        shared.extend(shared_paths.iter().cloned());
        let shared_roots: Vec<String> = mine.roots.intersection(&theirs.roots).cloned().collect();
        rows.push(Ranked {
            index,
            shared_symbols: shared_symbols.len(),
            specific_symbols: specific.len(),
            shared_paths: shared_paths.len(),
            shared_identifiers: shared,
            shared_roots,
            rarity,
        });
    }
    rows.sort_by(|a, b| {
        b.specific_symbols
            .cmp(&a.specific_symbols)
            .then_with(|| b.rarity.cmp(&a.rarity))
            .then_with(|| b.shared_symbols.cmp(&a.shared_symbols))
            .then_with(|| b.shared_paths.cmp(&a.shared_paths))
            // Fully tied rows are ordered by their own identifiers, not by the
            // dossier's document order, so two dossiers holding the same items
            // in a different order rank them the same way.
            .then_with(|| a.shared_identifiers.cmp(&b.shared_identifiers))
    });
    rows.truncate(top_k.max(1));
    rows
}

// ------------------------------------------------------- mechanism lexicon

/// Lexical mechanism hints, keyed to the mechanism_class vocabulary published
/// with the rejection dataset. This is a HINT shown beside a prior item so the
/// reader can compare mechanisms; it is never used for ranking or for a
/// verdict, and an unmatched text stays None.
pub fn mechanism_of(text: &str) -> Option<&'static str> {
    let low = text.to_lowercase();
    const TABLE: [(&str, &[&str]); 30] = [
        ("callback_reentrancy_window", &["reentran"]),
        (
            "oracle_bounds_or_staleness",
            &["oracle", "price feed", "stale price"],
        ),
        (
            "slippage_protection_missing",
            &["slippage", "min amount out", "minimum amount out"],
        ),
        ("rounding_direction", &["rounding", "round down", "truncat"]),
        (
            "liveness_block",
            &[
                "freeze",
                "frozen",
                "locked forever",
                "permanently locked",
                "liveness",
                "denial of service",
                "grief",
                "halt",
            ],
        ),
        (
            "panic_without_persisted_impact",
            &["panic", "aborts", "assertion failure"],
        ),
        ("replay_scope", &["replay", "nonce reuse"]),
        (
            "signature_context_binding",
            &["signature", "domain separator", "eip-712", "eip712"],
        ),
        (
            "privileged_path_bypass",
            &["privileged", "onlyowner", "capability", "admin role"],
        ),
        (
            "token_classification_assumption",
            &[
                "fee-on-transfer",
                "fee on transfer",
                "rebasing",
                "non-standard token",
            ],
        ),
        (
            "reward_dilution",
            &["dilution", "share price", "inflation attack", "reward per"],
        ),
        ("stale_cached_value", &["cached", "stale value"]),
        (
            "missing_zero_check",
            &["zero address", "zero amount", "zero-value"],
        ),
        (
            "unused_state_flag",
            &["unused flag", "flag is never", "state flag"],
        ),
        (
            "parameter_bound_missing",
            &["parameter", "missing bound", "no upper bound", "unbounded"],
        ),
        (
            "approval_or_allowance_overwrite",
            &["approval", "allowance", "permit"],
        ),
        (
            "missing_consent_or_authorisation",
            &[
                "authorization",
                "authorisation",
                "access control",
                "missing check",
            ],
        ),
        (
            "secret_material_exposure",
            &["secret", "private key", "mnemonic"],
        ),
        (
            "config_dependent",
            &["configuration", "config", "governance parameter"],
        ),
        (
            "accounting_order_of_operations",
            &["accounting", "ledger", "balance update", "bookkeeping"],
        ),
        (
            "incorrect_formula_or_constant",
            &["formula", "incorrect constant", "off-by-one", "off by one"],
        ),
        (
            "missing_input_validation",
            &["input validation", "unvalidated"],
        ),
        (
            "boundary_condition_off_by_one",
            &["boundary", "off-by-one", "off by one", "exactly equal"],
        ),
        ("key_binding_missing", &["key binding", "wrong key"]),
        (
            "cross_boundary_message_ordering",
            &["message ordering", "cross-chain message"],
        ),
        (
            "no_op_state_transition",
            &["no-op", "no op", "noop", "idempotent"],
        ),
        (
            "role_asymmetry",
            &["role asymmetry", "asymmetric role", "guardian"],
        ),
        (
            "accounting_bias_direction",
            &["accounting bias", "in one direction"],
        ),
        (
            "fee_base_or_ordering",
            &["fee base", "fee taken", "fee applied"],
        ),
        (
            "self_harm_precondition",
            &["self-harm", "self harm", "self-inflicted"],
        ),
    ];
    for (class, needles) in TABLE {
        if needles.iter().any(|n| contains_unnegated(&low, n)) {
            return Some(class);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Item;

    fn item(id: &str, text: &str) -> Item {
        let profile = Profile::from_text(text);
        Item {
            id: id.to_string(),
            source_id: "src-0001".into(),
            kind: "audit-finding".into(),
            title: None,
            section: None,
            page: 1,
            quote: evidence_quote(text),
            identifiers: profile.all_identifiers(),
            mechanism_class: mechanism_of(text).map(str::to_string),
            clause_class: None,
            text: text.to_string(),
        }
    }

    #[test]
    fn shared_function_names_rank_and_plain_words_do_not() {
        let prior = item(
            "item-0001",
            "The reduce_outflow helper credits only the current segment, so a refund that crosses the hour boundary is destroyed. add_outflow then aborts.",
        );
        let draft = "The same defect: reduce_outflow in limiter destroys the refund, and add_outflow still sums every segment.";
        let hits = rank(&[prior], draft, 5);
        assert_eq!(hits.len(), 1, "expected an identifier hit");
        assert!(hits[0]
            .shared_identifiers
            .iter()
            .any(|s| s.contains("reduce_outflow")));

        let prose_a = "The attacker can withdraw funds from the vault and the user loses money because the protocol does not check the cached total before it updates the share price for the next depositor in the same block.";
        let prose_b = item("item-0002", "The user loses money because an attacker can withdraw funds from the vault when the protocol does not check the cached total before the share price updates for the next depositor.");
        assert!(
            rank(&[prose_b], prose_a, 5).is_empty(),
            "plain word overlap must not rank"
        );
    }

    #[test]
    fn identifiers_shared_by_most_items_are_generic() {
        // Ten items in one repository all name the same two contracts; a
        // finding that shares only those must not match any of them.
        let mut items: Vec<Item> = Vec::new();
        for i in 0..10 {
            items.push(item(
                &format!("item-{:04}", i + 1),
                "The LoanBook contract calls SwapDesk on every route.",
            ));
        }
        items.push(item(
            "item-0011",
            "A rare path: FeeAccountingMismatch fires inside FeeLedger.",
        ));
        let hits = rank(&items, "LoanBook and SwapDesk appear here", 5);
        assert!(
            hits.is_empty(),
            "symbols shared by most items are generic: {hits:?}"
        );
        let hits = rank(
            &items,
            "FeeAccountingMismatch and FeeLedger both appear here",
            5,
        );
        assert_eq!(hits.len(), 1, "{hits:?}");
        assert_eq!(hits[0].index, 10);
    }

    #[test]
    fn tied_rows_are_ordered_by_identifiers_not_document_order() {
        let text = "The alpha_beta and gamma_delta calls.";
        let mut items = vec![item("item-0001", text), item("item-0002", text)];
        for i in 0..8 {
            items.push(item(
                &format!("noise-{i}"),
                &format!("Unrelated text {i} with noise_symbol_{i}."),
            ));
        }
        let a = rank(&items, text, 5);
        let mut reversed = vec![items[1].clone(), items[0].clone()];
        reversed.extend(items[2..].iter().cloned());
        let b = rank(&reversed, text, 5);
        assert_eq!(a.len(), 2);
        assert_eq!(b.len(), 2);
        assert_eq!(
            a[0].shared_identifiers.join(","),
            b[0].shared_identifiers.join(","),
            "identical rows must rank by identity, not by position"
        );
    }

    #[test]
    fn a_path_alone_does_not_meet_the_threshold() {
        let prior = item(
            "item-0001",
            "The bug is in src/Vault.sol: distributeFees loses the refund.",
        );
        let draft = "src/Vault.sol is where the settle path lives.";
        assert!(
            rank(&[prior], draft, 5).is_empty(),
            "a shared path alone must not rank: paths are shared by every finding in a repo"
        );
    }

    #[test]
    fn one_shared_identifier_is_below_the_threshold() {
        let prior = item("item-0001", "The distributeFees call reverts.");
        let draft = "Only distributeFees is mentioned once, with settleFees.";
        assert!(rank(&[prior], draft, 5).is_empty());
    }

    #[test]
    fn the_top_k_caps_the_result() {
        let p = "The alpha_beta and gamma_delta calls.";
        let mut items: Vec<Item> = Vec::new();
        for i in 0..8 {
            items.push(item(
                &format!("item-{:04}", i + 1),
                &format!("Noise item {i} names noise_symbol_{i} only."),
            ));
        }
        items.push(item("item-0009", p));
        items.push(item("item-0010", p));
        let hits = rank(&items, p, 5);
        assert_eq!(hits.len(), 2, "{hits:?}");
        let capped = rank(&items, p, 1);
        assert_eq!(capped.len(), 1);
    }

    #[test]
    fn severity_comes_from_a_labelled_line_only() {
        assert_eq!(claimed_severity("Severity: High"), Some(Severity::High));
        assert_eq!(
            claimed_severity("| Severity | Medium |"),
            Some(Severity::Medium)
        );
        assert_eq!(claimed_severity("This is a high severity bug"), None);
        assert_eq!(Severity::High.label(), "high");
    }

    #[test]
    fn scope_matching_handles_globs_directories_and_bare_names() {
        assert!(in_scope("src/Vault.sol", "src/*.sol"));
        assert!(in_scope("a/b/src/Vault.sol", "src/"));
        assert!(in_scope("src/Vault.sol", "**/Vault.sol"));
        assert!(in_scope("Vault.sol", "src/Vault.sol"));
        assert!(!in_scope("test/Vault.t.sol", "src/"));
    }

    #[test]
    fn path_extraction_skips_urls() {
        let paths = extract_paths("see https://example.com/src/Vault.sol and src/Vault.sol:44");
        assert!(paths.iter().any(|p| p == "src/Vault.sol"));
        assert!(!paths.iter().any(|p| p.contains("example.com")));
    }

    #[test]
    fn mechanism_is_a_hint_and_unknown_is_none() {
        assert_eq!(
            mechanism_of("the oracle price is stale"),
            Some("oracle_bounds_or_staleness")
        );
        assert_eq!(mechanism_of("no oracle or price movement is needed"), None);
        assert_eq!(mechanism_of("nothing in the table"), None);
    }

    #[test]
    fn quotes_are_whole_sentences_and_never_cut_inside_an_identifier() {
        let text = "The consistency check in FeeLedger.distributeFees reverts with FeeAccountingMismatch when the fee index rounds up past the collected balance.";
        let quote = evidence_quote(text);
        assert!(quote.ends_with("collected balance."), "{quote}");
        assert!(quote.contains("FeeLedger.distributeFees"), "{quote}");
    }

    #[test]
    fn a_very_long_sentence_is_windowed_at_a_word_boundary() {
        let long = format!(
            "{} FeeAccountingMismatch {}",
            "word ".repeat(90),
            "tail ".repeat(90)
        );
        let quote = evidence_quote(&long);
        assert!(
            quote.chars().count() <= MAX_QUOTE_CHARS + 40,
            "window drifted too far: {}",
            quote.chars().count()
        );
        assert!(quote.contains("FeeAccountingMismatch"), "{quote}");
        assert!(
            quote.starts_with('\u{2026}') && quote.ends_with('\u{2026}'),
            "{quote}"
        );
    }

    #[test]
    fn multi_byte_text_never_panics_in_the_negation_window() {
        // A curly apostrophe immediately after the match used to land inside a
        // multi-byte character and panic on a byte slice (bench-eval, 2026-11-13).
        let text = "The reserve isn\u{2019}t funded; the oracle price is stale but there\u{2019}s no pause, and the admin can\u{2019}t fix it. \u{4e2d}\u{6587} text after the match.";
        assert!(contains_unnegated(&text.to_lowercase(), "oracle"));
        assert!(contains_unnegated(&text.to_lowercase(), "stale"));
        // "reserve isn't funded" is a negated statement about the reserve.
        assert!(!contains_unnegated(&text.to_lowercase(), "reserve"));
        let oracle = Regex::new(r"(?i)\b(oracle|price feed)\b").unwrap();
        let hits = find_unnegated(&oracle, text);
        assert!(hits.is_some(), "oracle is unnegated here");
        // And the whole ranking path on such a text.
        let prior = item(
            "item-0001",
            "The oracle price is stale and reserveisn\u{2019}t funded.",
        );
        let _ = rank(&[prior], text, 5);
    }

    #[test]
    fn negation_awareness_is_local_to_the_clause() {
        let text = "Attacker is any user; no role, no privileged call. The vault has no pause function; the oracle price is stale.";
        assert!(!contains_unnegated(&text.to_lowercase(), "role"));
        assert!(contains_unnegated(&text.to_lowercase(), "oracle"));
        assert!(contains_unnegated(&text.to_lowercase(), "any user"));
        assert!(!contains_unnegated(&text.to_lowercase(), "privileged"));
    }
}
