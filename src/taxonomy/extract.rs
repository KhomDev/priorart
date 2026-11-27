//! Deterministic extraction of mechanism, impact and asset classes.
//!
//! The extractor is a lexicon with two guards. Negation: a cue negated in its
//! own clause ("no reentrancy risk", "prevents reentrancy", "reentrancy is
//! prevented") is skipped, but a second clause still asserts the class.
//! Anti-cues: a cue occurrence inside the fix-side spelling of the same words
//! ("reentrancy guard") is skipped unless the anti-cue is itself described as
//! missing or absent ("the reentrancy guard is missing").
//!
//! There is no model, no network, no clock and no file access. Given the same
//! text the output is byte-identical.

use crate::taxonomy::{Class, Match, MechanismSet, Span, Taxonomy, ABSTAIN_NO_MECHANISM};
use regex::Regex;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

/// The largest text `priorart taxonomy classify` accepts, in bytes. It matches
/// the finding cap `priorart check` uses. The extractor itself is pure and
/// takes any `&str`, but spans and the CLI are bounded by this.
pub const MAX_TEXT_BYTES: usize = 16 * 1024 * 1024;

/// A finding with more than this many cue (or anti-cue) occurrences is
/// pathological: a 16 MiB file of the same phrase would otherwise make the
/// anti-cue containment check quadratic. The cap is deterministic (first
/// occurrences in left-to-right order win) and far above any real finding.
const MAX_OCCURRENCES: usize = 5_000;

/// Words that flip the meaning of a cue when they precede it in its clause.
const NEGATION_WORDS: &[&str] = &[
    "no",
    "not",
    "never",
    "without",
    "cannot",
    "can't",
    "nor",
    "neither",
    "none",
    "non",
    "zero",
    "isn",
    "aren",
    "wasn",
    "weren",
    "doesn",
    "didn",
    "isn't",
    "aren't",
    "wasn't",
    "weren't",
    "doesn't",
    "didn't",
    "won't",
    "don't",
    "couldn't",
    "shouldn't",
    "wouldn't",
];

/// Words that flip the meaning of a cue when they follow it in its clause.
const POST_NEGATION_WORDS: &[&str] = &[
    "not",
    "never",
    "cannot",
    "can't",
    "nor",
    "neither",
    "isn",
    "aren",
    "wasn",
    "weren",
    "doesn",
    "didn",
    "isn't",
    "aren't",
    "wasn't",
    "weren't",
    "doesn't",
    "didn't",
    "won't",
    "don't",
    "couldn't",
    "shouldn't",
    "wouldn't",
];

/// Verb stems that describe a fix rather than the defect.
const MITIGATION_STEMS: &[&str] = &[
    "prevent",
    "mitigat",
    "protect",
    "guard",
    "avoid",
    "eliminat",
    "remediat",
    "fix",
    "resolv",
    "immune",
    "unaffected",
    "safe",
];

/// Nouns that name a protection. A negated cue that is itself followed by one
/// of these is the ABSENCE of a protection, which is the defect: "no slippage
/// protection" asserts the price-impact class.
const PROTECTIVE_TOKENS: &[&str] = &[
    "protection",
    "protections",
    "guard",
    "guards",
    "check",
    "checks",
    "checked",
    "checking",
    "validation",
    "validate",
    "validates",
    "validating",
    "validated",
    "validator",
    "verify",
    "verifies",
    "verifying",
    "enforce",
    "enforces",
    "enforcing",
    "ensure",
    "ensures",
    "ensuring",
    "nonce",
    "nonces",
    "commitment",
    "heartbeat",
    "heartbeats",
    "freshness",
    "limit",
    "limits",
    "cap",
    "caps",
    "modifier",
    "modifiers",
    "authorization",
    "authorisation",
    "allowlist",
    "timelock",
];

/// Multi-word protection names, matched as substrings of the lowercased text.
const PROTECTIVE_PHRASES: &[&str] = &["access control", "access-control", "role check"];

/// Words that mark an anti-cue as missing rather than present.
const DEFICIENCY_BEFORE: &[&str] = &[
    "no",
    "not",
    "missing",
    "without",
    "lacks",
    "lacking",
    "absent",
    "unprotected",
    "fails",
    "failed",
    "removed",
    "disabled",
    "omits",
    "omitted",
    "forgot",
    "forgotten",
];

/// Words that mark an anti-cue as missing rather than present, after it.
const DEFICIENCY_AFTER: &[&str] = &[
    "missing",
    "absent",
    "unprotected",
    "disabled",
    "removed",
    "required",
    "needed",
    "lacking",
    "omitted",
];

/// Words that turn a following mitigation verb into a description of the fix.
const COPULAS: &[&str] = &[
    "is", "are", "was", "were", "be", "been", "being", "gets", "get", "can", "could", "would",
    "will", "must", "may", "might", "has", "have", "had",
];

#[derive(Debug, Clone)]
struct Token {
    text: String,
    start: usize,
    end: usize,
}

/// One cue or anti-cue occurrence, with the classes whose term it is.
#[derive(Debug, Clone)]
struct Occurrence {
    start: usize,
    end: usize,
    classes: Vec<String>,
}

/// A compiled class list: one alternation for its cues, one for its anti-cues.
#[derive(Debug)]
struct Category {
    cues: Option<Regex>,
    cue_classes: BTreeMap<String, Vec<String>>,
    anti_cues: Option<Regex>,
    anti_classes: BTreeMap<String, Vec<String>>,
    /// Assets keep one match per class and term; root causes and impacts keep
    /// one per class.
    per_term: bool,
}

/// The compiled taxonomy extractor.
#[derive(Debug)]
pub struct Extractor {
    taxonomy: &'static Taxonomy,
    root_causes: Category,
    impacts: Category,
    assets: Category,
    symbols: BTreeSet<String>,
}

/// Classify one text with the compiled taxonomy.
pub fn classify(text: &str) -> MechanismSet {
    extractor().classify(text)
}

fn extractor() -> &'static Extractor {
    static EXTRACTOR: OnceLock<Extractor> = OnceLock::new();
    EXTRACTOR.get_or_init(|| Extractor::new(crate::taxonomy::taxonomy()))
}

impl Extractor {
    /// Compile the three class lists. Compilation is deterministic and only
    /// fails on an invalid vendored taxonomy, which the unit tests rule out.
    pub fn new(taxonomy: &'static Taxonomy) -> Self {
        let symbol_class = taxonomy.assets.iter().find(|c| c.id == "specific-token");
        let symbols = symbol_class
            .map(|c| c.symbols.iter().map(|s| s.to_lowercase()).collect())
            .unwrap_or_default();
        Self {
            taxonomy,
            root_causes: Category::new(&taxonomy.root_causes, false),
            impacts: Category::new(&taxonomy.impacts, false),
            assets: Category::new(&taxonomy.assets, true),
            symbols,
        }
    }

    /// Classify one text. Pure: same input, same output, in the same build.
    pub fn classify(&self, text: &str) -> MechanismSet {
        let root_causes = self.root_causes.extract(text);
        let impacts = self.impacts.extract(text);
        let mut assets = self.assets.extract(text);
        assets.extend(self.specific_tokens(text));
        assets.sort_by(|a, b| {
            a.span
                .cmp(&b.span)
                .then_with(|| a.class.cmp(&b.class))
                .then_with(|| a.term.cmp(&b.term))
        });
        let mut seen: BTreeSet<(String, String)> = BTreeSet::new();
        assets.retain(|m| seen.insert((m.class.clone(), m.term.to_lowercase())));
        let abstain_reason = if root_causes.is_empty() && impacts.is_empty() && assets.is_empty() {
            Some(ABSTAIN_NO_MECHANISM.to_string())
        } else {
            None
        };
        MechanismSet {
            schema: crate::taxonomy::TAXONOMY_SCHEMA.to_string(),
            root_causes,
            impacts,
            assets,
            abstain_reason,
        }
    }

    /// Named token symbols, only when the symbol is written inside quotes.
    fn specific_tokens(&self, text: &str) -> Vec<Match> {
        if self.symbols.is_empty() {
            return Vec::new();
        }
        let class = self
            .taxonomy
            .assets
            .iter()
            .find(|c| c.id == "specific-token")
            .map(|c| c.id.clone())
            .unwrap_or_else(|| "specific-token".to_string());
        let mut out = Vec::new();
        for (open, close) in [('`', '`'), ('"', '"'), ('\'', '\'')] {
            let mut from = 0usize;
            while let Some(rel) = text[from..].find(open) {
                let start = from + rel;
                let content_start = start + open.len_utf8();
                if open == '\'' && is_apostrophe(text, start) {
                    from = content_start;
                    continue;
                }
                let Some(rel_end) = text[content_start..].find(close) else {
                    break;
                };
                let content_end = content_start + rel_end;
                let content = &text[content_start..content_end];
                if !content.is_empty()
                    && content.len() <= 15
                    && !content.contains(char::is_whitespace)
                    && self.symbols.contains(&content.to_lowercase())
                {
                    out.push(Match {
                        class: class.clone(),
                        span: Span {
                            start_byte: content_start,
                            end_byte: content_end,
                        },
                        term: content.to_string(),
                    });
                }
                from = content_end + close.len_utf8();
            }
        }
        out
    }
}

/// A single quote is an apostrophe when it follows a letter or digit
/// ("don't", "users' funds"), not an opening quotation mark.
fn is_apostrophe(text: &str, pos: usize) -> bool {
    text[..pos]
        .chars()
        .next_back()
        .is_some_and(|c| c.is_alphanumeric())
}

impl Category {
    fn new(classes: &[Class], per_term: bool) -> Self {
        Self {
            cues: alternation(classes.iter().flat_map(|c| c.cues.iter().cloned())),
            cue_classes: class_map(classes, false),
            anti_cues: alternation(classes.iter().flat_map(|c| c.anti_cues.iter().cloned())),
            anti_classes: class_map(classes, true),
            per_term,
        }
    }

    fn extract(&self, text: &str) -> Vec<Match> {
        let Some(cues) = &self.cues else {
            return Vec::new();
        };
        let anti = self.anti_occurrences(text);
        let mut candidates: Vec<Match> = Vec::new();
        for m in cues.find_iter(text) {
            if candidates.len() >= MAX_OCCURRENCES {
                break;
            }
            let key = m.as_str().to_lowercase();
            let Some(classes) = self.cue_classes.get(&key) else {
                continue;
            };
            for class in classes {
                if anti.iter().any(|a| {
                    a.classes.contains(class)
                        && a.start <= m.start()
                        && m.end() <= a.end
                        && anti_cue_is_fix_mention(text, a.start, a.end)
                }) {
                    continue;
                }
                if is_negated(text, m.start(), m.end()) {
                    continue;
                }
                candidates.push(Match {
                    class: class.clone(),
                    span: Span {
                        start_byte: m.start(),
                        end_byte: m.end(),
                    },
                    term: m.as_str().to_string(),
                });
            }
        }
        candidates.sort_by(|a, b| {
            a.span
                .cmp(&b.span)
                .then_with(|| a.class.cmp(&b.class))
                .then_with(|| a.term.cmp(&b.term))
        });
        let mut seen: BTreeSet<(String, Option<String>)> = BTreeSet::new();
        candidates.retain(|m| {
            let term = if self.per_term {
                Some(m.term.to_lowercase())
            } else {
                None
            };
            seen.insert((m.class.clone(), term))
        });
        candidates
    }

    fn anti_occurrences(&self, text: &str) -> Vec<Occurrence> {
        let Some(anti) = &self.anti_cues else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for m in anti.find_iter(text) {
            if out.len() >= MAX_OCCURRENCES {
                break;
            }
            let key = m.as_str().to_lowercase();
            if let Some(classes) = self.anti_classes.get(&key) {
                out.push(Occurrence {
                    start: m.start(),
                    end: m.end(),
                    classes: classes.clone(),
                });
            }
        }
        out
    }
}

/// Build one case-insensitive alternation over `terms`, wrapped in `\b` at
/// every edge that is a word character so a cue never matches inside a longer
/// word. Longer terms come first so the most specific spelling wins.
fn alternation(terms: impl Iterator<Item = String>) -> Option<Regex> {
    let mut terms: Vec<String> = terms.collect();
    if terms.is_empty() {
        return None;
    }
    terms.sort_by(|a, b| {
        b.chars()
            .count()
            .cmp(&a.chars().count())
            .then_with(|| a.cmp(b))
    });
    terms.dedup();
    let mut pattern = String::from("(?i)");
    for (i, term) in terms.iter().enumerate() {
        if i > 0 {
            pattern.push('|');
        }
        pattern.push_str(&wrapped(term));
    }
    Some(Regex::new(&pattern).expect("vendored taxonomy cues compile as a regex"))
}

fn wrapped(term: &str) -> String {
    let mut out = String::new();
    if term.chars().next().is_some_and(is_word_char) {
        out.push_str("\\b");
    }
    out.push_str(&regex::escape(term));
    if term.chars().next_back().is_some_and(is_word_char) {
        out.push_str("\\b");
    }
    out
}

fn class_map(classes: &[Class], anti: bool) -> BTreeMap<String, Vec<String>> {
    let mut map: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for class in classes {
        let terms = if anti { &class.anti_cues } else { &class.cues };
        for term in terms {
            map.entry(term.to_lowercase())
                .or_default()
                .push(class.id.clone());
        }
    }
    for ids in map.values_mut() {
        ids.sort();
        ids.dedup();
    }
    map
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn is_token_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '\''
}

fn is_clause_break(c: char) -> bool {
    matches!(c, '.' | ';' | ',' | ':' | '!' | '?')
}

/// Move `pos` down to the nearest character boundary, so a byte-window
/// subtraction never slices a multi-byte character.
fn floor_boundary(text: &str, mut pos: usize) -> usize {
    pos = pos.min(text.len());
    while pos > 0 && !text.is_char_boundary(pos) {
        pos -= 1;
    }
    pos
}

/// The byte offset just after the last clause break before `pos`.
fn clause_start(text: &str, pos: usize) -> usize {
    text[..pos]
        .char_indices()
        .rev()
        .find(|(_, c)| is_clause_break(*c))
        .map(|(i, c)| i + c.len_utf8())
        .unwrap_or(0)
}

/// The end of the clause fragment that starts at `from`, capped at `max` bytes
/// and stopping at clause punctuation.
fn window_end(text: &str, from: usize, max: usize) -> usize {
    let mut end = from;
    for (i, c) in text[from..].char_indices() {
        if i > max || is_clause_break(c) {
            break;
        }
        end = from + i + c.len_utf8();
    }
    end
}

/// Lowercased word tokens in `text[from..to]`, with their byte offsets.
fn tokens_in(text: &str, from: usize, to: usize) -> Vec<Token> {
    let mut out = Vec::new();
    let mut start: Option<usize> = None;
    for (i, c) in text[from..to].char_indices() {
        let at = from + i;
        if is_token_char(c) {
            start.get_or_insert(at);
        } else if let Some(s) = start.take() {
            out.push(Token {
                text: text[s..at].to_lowercase(),
                start: s,
                end: at,
            });
        }
    }
    if let Some(s) = start {
        out.push(Token {
            text: text[s..to].to_lowercase(),
            start: s,
            end: to,
        });
    }
    out
}

fn word_tokens(text: &str) -> impl Iterator<Item = String> + '_ {
    text.split(|c: char| !is_token_char(c))
        .filter(|t| !t.is_empty())
        .map(|t| t.to_lowercase())
}

fn is_negation(token: &str) -> bool {
    NEGATION_WORDS.contains(&token)
}

fn is_post_negation(token: &str) -> bool {
    POST_NEGATION_WORDS.contains(&token)
}

fn is_mitigation(token: &str) -> bool {
    MITIGATION_STEMS.iter().any(|stem| token.starts_with(*stem))
}

fn is_copula(token: &str) -> bool {
    COPULAS.contains(&token)
}

fn is_protective_token(token: &str) -> bool {
    PROTECTIVE_TOKENS.contains(&token)
}

/// Is the cue itself, or the text immediately after it, a protection noun?
/// Such a cue is a missing-protection claim, not an absence claim.
fn follows_protective(text: &str, start: usize, end: usize) -> bool {
    let cue_low = text[start..end].to_lowercase();
    if protective(&cue_low) {
        return true;
    }
    let stop = window_end(text, end, 32);
    protective(&text[end..stop].to_lowercase())
}

fn protective(low: &str) -> bool {
    if PROTECTIVE_PHRASES.iter().any(|p| low.contains(*p)) {
        return true;
    }
    word_tokens(low).any(|t| is_protective_token(&t))
}

/// Is the cue negated by its own clause?
///
/// A negation word ("no", "without") in front of a cue that is itself followed
/// by a protection noun is the ABSENCE of that protection, so it counts as
/// evidence: "no reentrancy protection" asserts the class. A mitigation verb
/// ("prevents") negates regardless of what follows.
fn is_negated(text: &str, start: usize, end: usize) -> bool {
    let protective = follows_protective(text, start, end);
    let from = clause_start(text, start).max(floor_boundary(text, start.saturating_sub(96)));
    let toks = tokens_in(text, from, start);
    // "no validation of reentrancy": the protection noun sits just before the
    // cue, and the negation describes its absence.
    let protective_before = toks
        .iter()
        .rev()
        .take(3)
        .any(|tok| is_protective_token(&tok.text));
    for (i, tok) in toks.iter().enumerate() {
        let distance = start.saturating_sub(tok.end);
        if is_negation(&tok.text) && distance <= 32 {
            // "does not check reentrancy" is an assertion of the defect.
            if toks.get(i + 1).is_some_and(|next| {
                (is_mitigation(&next.text) || is_protective_token(&next.text))
                    && next.start.saturating_sub(tok.end) <= 12
            }) {
                continue;
            }
            if protective || protective_before {
                continue;
            }
            return true;
        }
        if is_mitigation(&tok.text) && distance <= 64 {
            if i > 0
                && is_negation(&toks[i - 1].text)
                && tok.start.saturating_sub(toks[i - 1].end) <= 12
            {
                continue;
            }
            return true;
        }
    }
    post_negated(text, end)
}

/// Is the cue followed by a negation or by "is/are/... <mitigation>"?
fn post_negated(text: &str, end: usize) -> bool {
    let stop = window_end(text, end, 32);
    let toks = tokens_in(text, end, stop);
    let mut any_negation = false;
    let mut every_negation_is_double = true;
    for (i, tok) in toks.iter().enumerate() {
        if !is_post_negation(&tok.text) {
            continue;
        }
        any_negation = true;
        let paired = toks[i + 1..]
            .iter()
            .take(2)
            .any(|next| is_mitigation(&next.text) || is_protective_token(&next.text));
        if !paired {
            every_negation_is_double = false;
        }
    }
    if any_negation {
        return !every_negation_is_double;
    }
    has_copula_mitigation(&toks)
}

/// "is prevented", "has been mitigated", "gets guarded", "is checked": a
/// copula followed by a mitigation or protection word within two tokens.
fn has_copula_mitigation(toks: &[Token]) -> bool {
    for i in 0..toks.len() {
        if !is_copula(&toks[i].text) {
            continue;
        }
        for next in toks.iter().skip(i + 1).take(2) {
            if is_mitigation(&next.text) || is_protective_token(&next.text) {
                return true;
            }
        }
    }
    false
}

/// An anti-cue is a fix mention unless the text describes it as missing.
fn anti_cue_is_fix_mention(text: &str, start: usize, end: usize) -> bool {
    !(deficiency_before(text, start) || deficiency_after(text, end))
}

fn deficiency_before(text: &str, start: usize) -> bool {
    let from = clause_start(text, start).max(floor_boundary(text, start.saturating_sub(48)));
    tokens_in(text, from, start).iter().any(|tok| {
        DEFICIENCY_BEFORE.contains(&tok.text.as_str()) && start.saturating_sub(tok.end) <= 32
    })
}

fn deficiency_after(text: &str, end: usize) -> bool {
    let stop = window_end(text, end, 48);
    let toks = tokens_in(text, end, stop);
    if toks
        .iter()
        .any(|tok| DEFICIENCY_AFTER.contains(&tok.text.as_str()))
    {
        return true;
    }
    for (i, tok) in toks.iter().enumerate() {
        if tok.text != "not" {
            continue;
        }
        if toks.iter().skip(i + 1).take(3).any(|next| {
            matches!(
                next.text.as_str(),
                "implemented"
                    | "present"
                    | "enforced"
                    | "called"
                    | "checked"
                    | "applied"
                    | "used"
                    | "set"
                    | "configured"
            )
        }) {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenization_keeps_contractions_and_offsets() {
        let text = "no, isn't reentrancy";
        let toks = tokens_in(text, 0, text.len());
        assert_eq!(toks.len(), 3);
        assert_eq!(toks[0].text, "no");
        assert_eq!(toks[1].text, "isn't");
        assert_eq!(toks[2].text, "reentrancy");
        assert_eq!(&text[toks[2].start..toks[2].end], "reentrancy");
    }

    #[test]
    fn word_boundary_wrapping_escapes_regex_metacharacters() {
        assert_eq!(wrapped("eip-712"), "\\beip\\-712\\b");
        assert_eq!(wrapped("msg.value"), "\\bmsg\\.value\\b");
        assert_eq!(wrapped("(x)"), "\\(x\\)");
    }

    #[test]
    fn an_empty_term_list_compiles_to_no_regex() {
        assert!(alternation(std::iter::empty()).is_none());
        assert!(alternation(["reentrancy".to_string()].into_iter()).is_some());
    }
}
