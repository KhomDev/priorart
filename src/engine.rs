//! Matching engine: run the catalog against a finding, evaluate the gates, and
//! compute an outcome.
//!
//! The engine is deterministic and offline. It never contacts the network, and it
//! holds no target data - every target-specific input arrives at runtime.
//!
//! MATCHING SEMANTICS
//! ------------------
//! 1. A document is split into SECTIONS at markdown headings (`#` to `######`
//!    followed by a space, outside fenced code blocks), and each section into
//!    SENTENCES. A '.', '!' or '?' ends a sentence only when it is followed by
//!    whitespace, a closing mark or the end of the text, so `Vault.sol`, `1.5`,
//!    `assert!(` and `e.g.` do not split.
//! 2. any_of / all_of are evaluated sentence by sentence.
//! 3. A match is discarded when a negation cue appears within the N tokens
//!    immediately before it (default 6). Typographic apostrophes are folded to
//!    ASCII first, so "doesn’t" negates like "doesn't". A contrastive phrase
//!    in a slightly wider window ("rather than", "instead of", "as opposed to")
//!    negates what follows it, which a single-token cue cannot see. An
//!    immediately preceding "non" negates a hyphenated form ("non-best-practice").
//!    An exclusive phrase in that same window ("no one but", "nobody except",
//!    "no one other than") is NOT a negation: it says only the following thing
//!    holds. Rules that already handle negation through none_of set
//!    negation_aware: false.
//! 4. Every fire reports the matched snippet AND its sentence, so the user can
//!    see exactly what the tool reacted to and argue with it.
//!
//! Everything here is a heuristic over prose. It cannot verify a claim in the
//! report, and it can be gamed by wording. See docs/rules/ for the per-rule
//! false-positive modes.

use crate::model::*;
use regex::Regex;
use std::collections::BTreeSet;
use std::sync::LazyLock;

/// Tokens that negate a following match, within the negation window.
const NEGATION_CUES: [&str; 19] = [
    "no", "not", "never", "without", "neither", "nor", "cannot", "cant", "none", "nobody",
    "nothing", "lack", "lacks", "lacking", "absent", "fails", "fail", "unable", "missing",
];

/// How many tokens before a match the single-token negation window looks at.
pub const NEGATION_WINDOW: usize = 6;

/// How many tokens before a match a multi-word contrastive phrase is sought in.
/// Wider than the single-token window so "in contrast to a best-practice"
/// still reaches the phrase, without letting a lone "not" travel that far.
pub const NEGATION_PHRASE_WINDOW: usize = 8;

/// Contrastive phrases that negate the words which follow them.
/// "rather than a best-practice observation" is a denial of that class, and
/// "rather" / "than" are not single-token cues.
const NEGATION_PHRASES: [&str; 11] = [
    "rather than",
    "instead of",
    "as opposed to",
    "other than",
    "in contrast to",
    "more than a",
    "more than just",
    "beyond a",
    "anything but",
    "far from a",
    "far from the",
];

/// A rule with its regexes pre-compiled, so matching is a single pass.
pub struct CompiledRule {
    pub rule: Rule,
    any_of: Vec<Regex>,
    all_of: Vec<Regex>,
    none_of: Vec<Regex>,
    escalate_on: Vec<Regex>,
    downgrade_on: Vec<Regex>,
    suppress_document: Vec<Regex>,
}

#[derive(Debug)]
pub enum RuleError {
    BadRegex {
        rule: String,
        pattern: String,
        error: String,
    },
}

impl std::fmt::Display for RuleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RuleError::BadRegex {
                rule,
                pattern,
                error,
            } => {
                write!(f, "rule {rule}: invalid regex {pattern:?}: {error}")
            }
        }
    }
}

/// Compile a user or catalog pattern the way every matcher in this tool does:
/// case-insensitive unless the pattern opts out with `(?-i:...)`.
pub fn compile_pattern(p: &str) -> Result<Regex, regex::Error> {
    Regex::new(&format!("(?i){p}"))
}

fn compile(rule_id: &str, patterns: &[String]) -> Result<Vec<Regex>, RuleError> {
    patterns
        .iter()
        .map(|p| {
            compile_pattern(p).map_err(|e| RuleError::BadRegex {
                rule: rule_id.to_string(),
                pattern: p.clone(),
                error: e.to_string(),
            })
        })
        .collect()
}

impl CompiledRule {
    pub fn new(rule: Rule) -> Result<Self, RuleError> {
        let any_of = compile(&rule.id, &rule.any_of)?;
        let all_of = compile(&rule.id, &rule.all_of)?;
        let none_of = compile(&rule.id, &rule.none_of)?;
        let escalate_on = compile(&rule.id, &rule.escalate_on)?;
        let downgrade_on = compile(&rule.id, &rule.downgrade_on)?;
        let suppress_document = compile(&rule.id, &rule.suppress_document)?;
        Ok(Self {
            rule,
            any_of,
            all_of,
            none_of,
            escalate_on,
            downgrade_on,
            suppress_document,
        })
    }

    fn skips(&self, kind: SectionKind) -> bool {
        self.rule.ignore_sections.contains(&kind)
    }

    /// Does this rule fire against the text?
    pub fn matches(&self, text: &str) -> Option<Match> {
        // A structural rule ignores the generic matcher entirely.
        if let Some(kind) = self.rule.structural {
            return structural_match(kind, text);
        }

        let sections = split_sections(text);

        // Document-level suppression. Unlike sentence-scoped none_of, one
        // sentence that demonstrates an unprivileged profit path drops the
        // rule even when the privileged word is in a different sentence.
        if self.suppress_document.iter().any(|re| {
            sections.iter().any(|section| {
                section.sentences.iter().any(|sentence| {
                    find_unnegated(re, sentence, self.rule.negation_aware).is_some()
                })
            })
        }) {
            return None;
        }

        // Document-scoped escape hatch. A none_of hit anywhere suppresses the
        // rule, unless the rule scopes suppression to the sentence.
        let sentence_scoped_none = self.rule.none_of_scope == NoneOfScope::Sentence;
        if !sentence_scoped_none && self.none_of.iter().any(|re| re.is_match(text)) {
            return None;
        }

        let mut hits: Vec<SentenceHit> = Vec::new();
        let mut strong: Vec<SentenceHit> = Vec::new();

        for section in &sections {
            if self.skips(section.kind) {
                continue;
            }
            for sentence in &section.sentences {
                if sentence_scoped_none && self.none_of.iter().any(|re| re.is_match(sentence.text))
                {
                    continue;
                }
                for re in &self.any_of {
                    if let Some(hit) = find_unnegated(re, sentence, self.rule.negation_aware) {
                        if self.downgrade_on.iter().any(|d| d.is_match(&hit.snippet)) {
                            continue;
                        }
                        hits.push(hit);
                    }
                }
                for re in &self.escalate_on {
                    if let Some(hit) = find_unnegated(re, sentence, self.rule.negation_aware) {
                        strong.push(hit);
                    }
                }
            }
        }

        if !self.any_of.is_empty() && hits.is_empty() && strong.is_empty() {
            return None;
        }

        // all_of must also hold, each pattern somewhere, non-negated.
        for re in &self.all_of {
            let found = sections
                .iter()
                .filter(|s| !self.skips(s.kind))
                .flat_map(|s| s.sentences.iter())
                .any(|sentence| find_unnegated(re, sentence, self.rule.negation_aware).is_some());
            if !found {
                return None;
            }
        }

        // An all_of-only rule draws its evidence from the all_of patterns.
        if self.any_of.is_empty() && hits.is_empty() && strong.is_empty() {
            for re in &self.all_of {
                for section in sections.iter().filter(|s| !self.skips(s.kind)) {
                    for sentence in &section.sentences {
                        if let Some(hit) = find_unnegated(re, sentence, self.rule.negation_aware) {
                            hits.push(hit);
                        }
                    }
                }
            }
        }

        // Strong hits first: they decide the verdict, so they must be shown.
        let verdict = if strong.is_empty() {
            self.rule.verdict
        } else {
            self.rule.escalate_to.unwrap_or(Verdict::Hard)
        };
        strong.extend(hits);
        let mut hits = strong;
        let mut seen = BTreeSet::new();
        hits.retain(|h| seen.insert((h.snippet.clone(), h.sentence.clone())));
        if hits.is_empty() {
            return None;
        }
        hits.truncate(3);

        Some(Match {
            evidence: hits.iter().map(|h| h.snippet.clone()).collect(),
            sentences: hits.iter().map(|h| h.sentence.clone()).collect(),
            verdict,
        })
    }
}

#[derive(Debug, Clone)]
pub struct Match {
    pub evidence: Vec<String>,
    pub sentences: Vec<String>,
    pub verdict: Verdict,
}

#[derive(Debug, Clone)]
struct SentenceHit {
    snippet: String,
    sentence: String,
}

#[derive(Debug)]
pub(crate) struct Sentence<'a> {
    pub(crate) text: &'a str,
}

#[derive(Debug)]
pub(crate) struct Section<'a> {
    pub(crate) kind: SectionKind,
    pub(crate) body: &'a str,
    pub(crate) sentences: Vec<Sentence<'a>>,
}

// ------------------------------------------------------------- parsing

/// Classify a section heading by whole words, so "epoch" is not "poc" and
/// "latest" is not "test".
pub(crate) fn section_kind(heading: &str) -> SectionKind {
    let low = heading.to_lowercase();
    let words: Vec<&str> = low
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();
    let has_any = |list: &[&str]| list.iter().any(|w| words.contains(w));
    if low.contains("proof of concept")
        || has_any(&[
            "poc",
            "pocs",
            "exploit",
            "reproduction",
            "reproduce",
            "reproducing",
            "test",
            "tests",
            "testing",
            "steps",
        ])
    {
        SectionKind::Poc
    } else if has_any(&[
        "recommendation",
        "recommendations",
        "recommended",
        "mitigation",
        "mitigations",
        "remediation",
        "fix",
        "fixes",
        "patch",
    ]) {
        SectionKind::Recommendation
    } else if has_any(&["impact", "severity", "headline", "summary", "description"]) {
        SectionKind::Claim
    } else {
        SectionKind::Other
    }
}

/// If `line` is an ATX heading, return its text.
fn heading_text(line: &str) -> Option<&str> {
    let t = line.trim_start_matches([' ', '\t']);
    if line.len() - t.len() > 3 {
        return None;
    }
    let hashes = t.bytes().take_while(|b| *b == b'#').count();
    if hashes == 0 || hashes > 6 {
        return None;
    }
    let rest = &t[hashes..];
    if !(rest.is_empty() || rest.starts_with([' ', '\t', '\r', '\n'])) {
        return None;
    }
    Some(rest.trim().trim_end_matches('#').trim())
}

/// The fence marker a line opens or closes, e.g. ("`", 3).
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

/// Split a document into sections at markdown headings, then into sentences.
/// Lines inside fenced code blocks never start a section, so a `# comment`
/// in a shell block or a `#[test]` attribute stays part of the PoC.
pub(crate) fn split_sections(text: &str) -> Vec<Section<'_>> {
    let mut sections: Vec<Section<'_>> = Vec::new();
    let mut current_kind = SectionKind::Claim;
    let mut current_start = 0usize;
    let mut fence: Option<(char, usize)> = None;

    let mut offset = 0usize;
    for line in text.split_inclusive('\n') {
        if let Some((c, n)) = fence_marker(line) {
            match fence {
                None => fence = Some((c, n)),
                Some((oc, on)) if oc == c && n >= on => {
                    let rest = line.trim().trim_start_matches(c);
                    if rest.is_empty() {
                        fence = None;
                    }
                }
                Some(_) => {}
            }
        } else if fence.is_none() {
            if let Some(heading) = heading_text(line) {
                if offset > current_start {
                    let body = &text[current_start..offset];
                    sections.push(Section {
                        kind: current_kind,
                        body,
                        sentences: split_sentences(body),
                    });
                }
                current_kind = section_kind(heading);
                // Start the new section AT the heading, so the headline text is
                // part of the section it introduces.
                current_start = offset;
            }
        }
        offset += line.len();
    }
    if current_start < text.len() {
        let body = &text[current_start..];
        sections.push(Section {
            kind: current_kind,
            body,
            sentences: split_sentences(body),
        });
    }
    if sections.is_empty() {
        sections.push(Section {
            kind: SectionKind::Claim,
            body: text,
            sentences: split_sentences(text),
        });
    }
    sections
}

const ABBREVIATIONS: [&str; 10] = [
    "e.g", "i.e", "eg", "ie", "etc", "vs", "cf", "approx", "incl", "fig",
];

fn ends_with_abbreviation(prefix: &str) -> bool {
    let word = prefix
        .rsplit(|c: char| c.is_whitespace() || c == '(' || c == '[')
        .next()
        .unwrap_or("")
        .to_lowercase();
    ABBREVIATIONS.contains(&word.as_str())
}

/// Split a block into sentences.
///
/// A newline always ends a sentence. '.', '!' and '?' end one only when the
/// next character is whitespace, a closing mark, or the end of the text, and a
/// '.' after a known abbreviation does not.
fn split_sentences(text: &str) -> Vec<Sentence<'_>> {
    let mut out = Vec::new();
    let mut start = 0usize;
    let bytes = text.as_bytes();
    for i in 0..bytes.len() {
        let c = bytes[i];
        let boundary = match c {
            b'\n' => true,
            b'.' | b'!' | b'?' => {
                let closes = match bytes.get(i + 1) {
                    None => true,
                    Some(n) => {
                        n.is_ascii_whitespace()
                            || matches!(n, b')' | b'"' | b'\'' | b'*' | b'_' | b']')
                    }
                };
                closes && !(c == b'.' && ends_with_abbreviation(&text[start..i]))
            }
            _ => false,
        };
        if boundary {
            let slice = &text[start..i];
            if !slice.trim().is_empty() {
                out.push(Sentence { text: slice });
            }
            start = i + 1;
        }
    }
    if start < text.len() {
        let slice = &text[start..];
        if !slice.trim().is_empty() {
            out.push(Sentence { text: slice });
        }
    }
    out
}

/// Find the pattern in the sentence, rejecting a match whose negation window
/// holds a cue.
fn find_unnegated(
    re: &Regex,
    sentence: &Sentence<'_>,
    negation_aware: bool,
) -> Option<SentenceHit> {
    for m in re.find_iter(sentence.text) {
        if negation_aware && is_negated(sentence.text, m.start()) {
            continue;
        }
        return Some(SentenceHit {
            snippet: collapse(m.as_str()),
            sentence: collapse(sentence.text),
        });
    }
    None
}

/// Tokens before `at`, apostrophes folded, lowercased.
fn prefix_tokens(sentence: &str, at: usize) -> Vec<String> {
    let prefix: String = sentence[..at]
        .chars()
        .map(|c| match c {
            '\u{2018}' | '\u{2019}' | '\u{02BC}' | '\u{FF07}' => '\'',
            other => other,
        })
        .collect();
    prefix
        .split(|c: char| !c.is_alphanumeric() && c != '\'')
        .filter(|t| !t.is_empty())
        .map(|t| t.to_lowercase())
        .collect()
}

/// Is the match negated by a cue or a contrastive phrase in the preceding window?
fn is_negated(sentence: &str, at: usize) -> bool {
    let tokens = prefix_tokens(sentence, at);
    // "non-best-practice" splits on the hyphen, so "non" is the token
    // immediately before the match. "non" further back is a different word
    // ("a non trivial note about best practice" is still that class).
    if tokens.last().is_some_and(|t| t == "non") {
        return true;
    }
    // "No one but the admin can" is an exclusive claim, not a denial. The
    // cue "no" and the phrase "other than" both sit in front of "admin can",
    // and treating them as negation hid the privilege.
    let phrase_at = tokens.len().saturating_sub(NEGATION_PHRASE_WINDOW);
    if exclusive_affirmation(&tokens[phrase_at..]) {
        return false;
    }
    let cue_at = tokens.len().saturating_sub(NEGATION_WINDOW);
    if tokens[cue_at..]
        .iter()
        .any(|t| NEGATION_CUES.contains(&t.as_str()) || t.ends_with("n't"))
    {
        return true;
    }
    let joined = tokens[phrase_at..].join(" ");
    NEGATION_PHRASES.iter().any(|p| joined.contains(p))
}

/// "no one but", "nobody except", "nothing other than", "no user but".
/// The words select the match; they do not deny it.
fn exclusive_affirmation(tokens: &[String]) -> bool {
    let n = tokens.len();
    for i in 0..n {
        // "no one but", "no user but", "no one other than". The word after
        // "no" is the thing being excluded, not a negation of the match.
        if tokens[i] == "no"
            && i + 1 < n
            && !NEGATION_CUES.contains(&tokens[i + 1].as_str())
            && exclusive_tail(&tokens[i + 2..])
        {
            return true;
        }
        if matches!(tokens[i].as_str(), "nobody" | "nothing" | "none" | "noone")
            && exclusive_tail(&tokens[i + 1..])
        {
            return true;
        }
    }
    false
}

fn exclusive_tail(tokens: &[String]) -> bool {
    match tokens {
        [t, ..] if t == "but" || t == "except" => true,
        [a, b, ..] if a == "other" && b == "than" => true,
        _ => false,
    }
}

/// A catalog id (`P2`, `P26`). Known-issue ids (`KI:`) and the citation anchor
/// (`CITE`) are structured facts, not claim heuristics.
pub fn claim_heuristic(rule_id: &str) -> bool {
    let Some(rest) = rule_id.strip_prefix('P') else {
        return false;
    };
    !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit())
}

/// Collapse whitespace, drop control characters, and cap the length, so a
/// snippet is safe and short enough to print.
pub(crate) fn collapse(s: &str) -> String {
    let clean: String = s
        .chars()
        .filter(|c| !c.is_control() || c.is_whitespace())
        .collect();
    let one = clean.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut out: String = one.chars().take(200).collect();
    if one.chars().count() > 200 {
        out.push('…');
    }
    out
}

// ------------------------------------------------------- structural rules

static LOSS_CLAIM: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(permanent(ly)?|irreversibl[ey]|unrecoverabl[ey]|total|complete(ly)?|100%)\s+(\w+\s+){0,3}(lock|loss|freez|brick|strand|drain|theft|dead)\w*",
    )
    .expect("LOSS_CLAIM compiles")
});
static FAILURE_ONLY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(reverts?|reverted|panics?|aborts?|expected_failure|assertion fail\w*|div(ision)? by zero)\b",
    )
    .expect("FAILURE_ONLY compiles")
});
/// The proof demonstrates a positive state change, which is what would make a
/// loss headline true. When one is present there is no contradiction.
static POSITIVE_EVIDENCE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(balances? (\w+ ){0,2}(increases?|increased|grows?|grew|rises?|rose|goes up|went up|drops?|dropped|decreases?|decreased)|profits?|profited|drains?|drained|steals?|stole|stolen|extracts?|extracted)\b",
    )
    .expect("POSITIVE_EVIDENCE compiles")
});
/// An assertion that something persisted AFTER the failure.
static PERSISTED_STATE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(persist\w*|storage|state remains|after the (call|transaction|tx) returns|remains? after|committed|unchanged across|survives the)\b",
    )
    .expect("PERSISTED_STATE compiles")
});

/// P19: the headline claims a loss that the proof contradicts.
///
///   * a sentence BEFORE the first proof section claims permanent or total
///     loss ("total value locked" is not a loss claim), AND
///   * a proof section (PoC / reproduction / test heading) shows a revert,
///     panic or expected failure, AND
///   * the proof shows no positive state change (no drain, profit or balance
///     movement).
///
/// Then the proof shows only a failure. Without an assertion of persisted state
/// in the claim or proof sections, the proof disproves the title: the SPAM
/// boundary, hard. With one, it fires as a NOTE for a human to read.
fn structural_match(kind: Structural, text: &str) -> Option<Match> {
    match kind {
        Structural::P19HeadlineVsProof => p19(text),
        Structural::P23Attacker => crate::claims::attacker(text),
        Structural::P24Preconditions => crate::claims::preconditions(text),
        Structural::P25Impact => crate::claims::impact(text),
        Structural::P26SeverityRule => crate::claims::severity_rule(text),
    }
}

fn p19(text: &str) -> Option<Match> {
    let sections = split_sections(text);

    let mut headline_hit: Option<String> = None;
    let mut poc_text = String::new();
    let mut claim_text = String::new();
    let mut seen_poc = false;
    for section in &sections {
        match section.kind {
            SectionKind::Poc => {
                seen_poc = true;
                for s in &section.sentences {
                    poc_text.push_str(s.text);
                    poc_text.push(' ');
                }
            }
            SectionKind::Recommendation => {}
            _ => {
                if !seen_poc {
                    for s in &section.sentences {
                        claim_text.push_str(s.text);
                        claim_text.push(' ');
                        if headline_hit.is_none()
                            && LOSS_CLAIM
                                .find_iter(s.text)
                                .any(|m| !m.as_str().to_lowercase().contains("value lock"))
                        {
                            headline_hit = Some(collapse(s.text));
                        }
                    }
                }
            }
        }
    }

    let headline = headline_hit?;
    if !FAILURE_ONLY.is_match(&poc_text) || POSITIVE_EVIDENCE.is_match(&poc_text) {
        return None;
    }
    let persisted = PERSISTED_STATE.is_match(&poc_text) || PERSISTED_STATE.is_match(&claim_text);
    let verdict = if persisted {
        Verdict::Warn
    } else {
        Verdict::Hard
    };
    Some(Match {
        evidence: vec![headline.clone()],
        sentences: vec![headline],
        verdict,
    })
}

// ---------------------------------------------------------------- evaluate

/// Run every default rule against the finding text.
///
/// Opt-in rules (P26) are skipped. Pass `evaluate_with(..., true)` to run them.
pub fn evaluate(rules: &[CompiledRule], text: &str) -> (Vec<Finding>, Vec<String>) {
    evaluate_with(rules, text, false)
}

/// Run the catalog. `include_opt_in` turns on rules with `default_emit: false`.
pub fn evaluate_with(
    rules: &[CompiledRule],
    text: &str,
    include_opt_in: bool,
) -> (Vec<Finding>, Vec<String>) {
    let mut findings = Vec::new();
    let mut passed = Vec::new();

    for cr in rules {
        if !cr.rule.default_emit && !include_opt_in {
            continue;
        }
        match cr.matches(text) {
            Some(m) => findings.push(Finding {
                rule: cr.rule.id.clone(),
                title: cr.rule.title.clone(),
                group: cr.rule.group.clone(),
                verdict: m.verdict,
                weight: cr.rule.weight,
                evidence: m.evidence,
                matched_sentences: m.sentences,
                fix: cr.rule.fix.clone(),
                doc: cr.rule.doc.clone(),
                evidence_kind: cr.rule.evidence,
                origin_rows: cr.rule.origin_rows,
            }),
            None => passed.push(cr.rule.id.clone()),
        }
    }
    (findings, passed)
}

/// Compute the report outcome and the viability score.
///
/// ONE decision rule, in this order:
///   1. a STRUCTURED hard finding (known issue, failed citation) -> KILL
///   2. a claim-heuristic hard rule, default                     -> ROUTE-TO-VERIFY
///      2b. the same rule with `strict` (--strict)               -> KILL
///   3. any soft rule, any FAILED gate, or score < threshold     -> HOLD
///   4. any warn rule with a non-zero weight                     -> NOTE
///   5. any gate Unknown                                         -> INCOMPLETE
///   6. only zero-weight notes, and every gate ran               -> NOTE
///   7. otherwise                                                -> CLEAR
///
/// A zero-weight note (the claim checks, P23-P26) does not hide an unknown
/// gate: absence of a heading is a weaker fact than a gate that could not run.
/// A warn rule that carries weight still outranks an unknown gate, as before.
///
/// The score is 100 minus the hand-set weights of the fired rules, clamped to
/// 0..=100. It is an UNCALIBRATED HEURISTIC and not a probability. Gates do NOT
/// deduct from it; a failed gate holds the finding and an unknown one makes the
/// outcome INCOMPLETE instead. The citation anchor is the exception that still
/// deducts: a missing file or line is emitted as a weighted hard finding, not
/// only as a gate, so a fabricated citation cannot score 100.
///
/// Since 0.8.0 a claim-heuristic hard rule (the P-ids) is ROUTE-TO-VERIFY, not a
/// hard stop. That is the default because the public benchmark does not show a
/// separator strong enough to justify stopping a submission: on 0.7.0 the hard
/// kills were 48% precise and hit 5.5% of paid findings. A known-issue match
/// (`KI:`) and a failed citation (`CITE`) are structured hard failures and
/// stay KILL in every mode. The pre-0.8.0 behaviour is `outcome_strict` /
/// `decide(..., false)`, reachable from the CLI with `--strict`.
pub fn outcome(findings: &[Finding], gates: &[GateOutcome], threshold: u32) -> (Outcome, u32) {
    decide(findings, gates, threshold, true)
}

/// The pre-0.8.0 decision rule: claim-heuristic hard rules are KILL, not ROUTE.
/// Kept for tests and for callers that explicitly want the strict mode.
pub fn outcome_strict(
    findings: &[Finding],
    gates: &[GateOutcome],
    threshold: u32,
) -> (Outcome, u32) {
    decide(findings, gates, threshold, false)
}

/// Same decision as [`outcome`]. `route_claim_rules` is true by default: a
/// claim-heuristic hard rule becomes ROUTE-TO-VERIFY. Pass false for the strict
/// mode (`--strict` on the CLI).
pub fn decide(
    findings: &[Finding],
    gates: &[GateOutcome],
    threshold: u32,
    route_claim_rules: bool,
) -> (Outcome, u32) {
    let deducted: u64 = findings.iter().map(|f| u64::from(f.weight)).sum();
    let score = 100u64.saturating_sub(deducted) as u32;

    let structured_hard = findings
        .iter()
        .any(|f| f.verdict == Verdict::Hard && !claim_heuristic(&f.rule));
    let heuristic_hard = findings
        .iter()
        .any(|f| f.verdict == Verdict::Hard && claim_heuristic(&f.rule));
    let has_soft = findings.iter().any(|f| f.verdict == Verdict::Soft);
    let has_weighted_warn = findings
        .iter()
        .any(|f| f.verdict == Verdict::Warn && f.weight > 0);
    let has_note = findings
        .iter()
        .any(|f| f.verdict == Verdict::Warn && f.weight == 0);
    let gate_failed = gates.iter().any(|g| g.status == GateStatus::Fail);
    let gate_unknown = gates.iter().any(|g| g.status == GateStatus::Unknown);

    let outcome = if structured_hard {
        Outcome::Kill
    } else if route_claim_rules && heuristic_hard {
        Outcome::Route
    } else if heuristic_hard {
        Outcome::Kill
    } else if has_soft || gate_failed || score < threshold {
        Outcome::Hold
    } else if has_weighted_warn {
        Outcome::Note
    } else if gate_unknown {
        Outcome::Incomplete
    } else if has_note {
        Outcome::Note
    } else {
        Outcome::Clear
    };
    (outcome, score)
}

// ---------------------------------------------------------------- severity

/// A severity, ordered from lowest to highest.
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

    /// The floor a tier shape implies, or None for an unknown shape.
    pub fn floor_of_shape(shape: &str) -> Option<Self> {
        match shape {
            "critical_only" => Some(Severity::Critical),
            "high_and_above" => Some(Severity::High),
            "medium_and_above" => Some(Severity::Medium),
            "low_and_above" => Some(Severity::Low),
            _ => None,
        }
    }
}

static SEVERITY_LINE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?im)^\s*(?:#{1,6}\s*)?(?:[-*]\s+)?(?:\*\*|__)?\s*severity\s*(?:\*\*|__)?\s*[:\-–|]\s*(?:\*\*|__)?\s*(critical|high|medium|low|informational|info)\b",
    )
    .expect("SEVERITY_LINE compiles")
});
static SEVERITY_ROW: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\|\s*severity\s*\|\s*(?:\*\*)?\s*(critical|high|medium|low|informational|info)\b",
    )
    .expect("SEVERITY_ROW compiles")
});

/// The severity the finding claims, from a `Severity: High` line or a
/// `| Severity | High |` table row.
pub fn claimed_severity(text: &str) -> Option<Severity> {
    SEVERITY_LINE
        .captures(text)
        .or_else(|| SEVERITY_ROW.captures(text))
        .and_then(|c| Severity::parse(&c[1]))
}

// ---------------------------------------------------------------- dates

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Days since the Unix epoch for a strict ISO `YYYY-MM-DD` date, or None when
/// the string is not a real calendar date.
pub fn parse_iso_date(iso: &str) -> Option<i64> {
    let b = iso.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return None;
    }
    let y: i64 = iso[0..4].parse().ok()?;
    let m: i64 = iso[5..7].parse().ok()?;
    let d: i64 = iso[8..10].parse().ok()?;
    if !(1..=12).contains(&m) {
        return None;
    }
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let max = match m {
        2 if leap => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    if !(1..=max).contains(&d) {
        return None;
    }
    Some(days_from_civil(y, m, d))
}

/// Today, in days since the Unix epoch (UTC).
pub fn today_days() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
        .div_euclid(86_400)
}

// ---------------------------------------------------------------- gates

/// Everything the gates may read. Every field is optional: a gate whose input
/// is missing reports Unknown, never Pass.
#[derive(Default, Clone, Copy)]
pub struct GateInputs<'a> {
    pub ledger: Option<&'a [LedgerEntry]>,
    /// Which ledger entry to use when the ledger holds more than one program.
    pub program: Option<&'a str>,
    /// The severity of the thesis. Falls back to a `Severity:` line in the
    /// finding.
    pub severity: Option<Severity>,
    pub scope: Option<&'a [String]>,
    pub prior_audits: Option<&'a [String]>,
    /// The target's test suite, one (path, text) pair per file.
    pub tests: Option<&'a [(String, String)]>,
    pub fork_of: Option<&'a str>,
    pub own_log: Option<&'a [LogEntry]>,
    pub known_issues: Option<&'a [KnownIssue]>,
    /// Prior findings ranked by code identifiers. Drives D1 together with,
    /// or instead of, the regex corpus.
    pub index: Option<&'a [IndexedFinding]>,
    pub finding: Option<&'a str>,
    /// Citation anchor, already computed against `--target` / `--commit`.
    /// None means no target was supplied.
    pub anchor: Option<AnchorGate<'a>>,
}

/// The citation-anchor result the gate printer shows. The weighted finding,
/// when there is one, is attached by the caller so the score moves.
#[derive(Clone, Copy)]
pub struct AnchorGate<'a> {
    pub status: GateStatus,
    pub detail: &'a str,
}

/// The ledger entry a gate should read.
enum Program<'a> {
    NoLedger,
    Empty,
    Ambiguous(usize),
    One(&'a LedgerEntry),
}

impl<'a> GateInputs<'a> {
    fn program(&self) -> Program<'a> {
        let Some(entries) = self.ledger else {
            return Program::NoLedger;
        };
        if let Some(id) = self.program {
            return match entries.iter().find(|e| e.id == id) {
                Some(e) => Program::One(e),
                None => Program::Empty,
            };
        }
        match entries {
            [] => Program::Empty,
            [one] => Program::One(one),
            many => Program::Ambiguous(many.len()),
        }
    }

    fn severity(&self) -> Option<Severity> {
        self.severity
            .or_else(|| self.finding.and_then(claimed_severity))
    }
}

fn worst(a: GateStatus, b: GateStatus) -> GateStatus {
    let rank = |s: GateStatus| match s {
        GateStatus::Pass => 0,
        GateStatus::Unknown => 1,
        GateStatus::Fail => 2,
    };
    if rank(b) > rank(a) {
        b
    } else {
        a
    }
}

fn program_unavailable(p: &Program<'_>, what: &str) -> Option<(GateStatus, String)> {
    match p {
        Program::NoLedger => Some((
            GateStatus::Unknown,
            format!("no ledger supplied - cannot check {what}"),
        )),
        Program::Empty => Some((GateStatus::Unknown, "ledger is empty".into())),
        Program::Ambiguous(n) => Some((
            GateStatus::Unknown,
            format!("the ledger holds {n} programs - choose one with --program <id>"),
        )),
        Program::One(_) => None,
    }
}

fn gate_anchor(anchor: Option<AnchorGate<'_>>) -> (GateStatus, String) {
    match anchor {
        None => (
            GateStatus::Unknown,
            "no target tree supplied - cited files were not opened. Unknown is not a pass".into(),
        ),
        Some(a) => (a.status, a.detail.to_string()),
    }
}

/// Evaluate the pre-hunt gates from whatever inputs were supplied.
///
/// A gate whose input is missing reports Unknown, never Pass. Unknown is not
/// pass: a gate you could not run is a gate you have not run. When a finding is
/// supplied, G1 to G5 and D1 compare their input against it; without one they
/// check only that the input is complete and well-formed, and say so.
pub fn evaluate_gates(gates: &[Gate], inp: &GateInputs<'_>) -> Vec<GateOutcome> {
    gates
        .iter()
        .map(|g| {
            let (status, detail) = match g.id.as_str() {
                "G1" => gate_floor(inp),
                "G2" => gate_exclusions(inp),
                "G3" => gate_scope(inp),
                "G4" => gate_prior_audits(inp),
                "G5" => gate_tests(inp),
                "G6" => gate_fork(inp.fork_of),
                "G7" => gate_own_history(inp.own_log, inp.finding),
                "D1" => gate_known_issues(inp.known_issues, inp.index, inp.finding),
                "A1" => gate_anchor(inp.anchor),
                _ => (GateStatus::Unknown, "unrecognised gate".to_string()),
            };
            GateOutcome {
                gate: g.id.clone(),
                title: g.title.clone(),
                question: g.question.clone(),
                needs: g.needs.clone(),
                blocks: g.blocks.clone(),
                weight: g.weight,
                status,
                detail,
                checks: g
                    .checks
                    .iter()
                    .map(|c| format!("{} — {} (fail: {})", c.name, c.test, c.fail))
                    .collect(),
                evidence: g.evidence,
                origin_rows: g.origin_rows,
            }
        })
        .collect()
}

fn gate_floor(inp: &GateInputs<'_>) -> (GateStatus, String) {
    let program = inp.program();
    if let Some(r) = program_unavailable(&program, "the payout floor") {
        return r;
    }
    let Program::One(e) = program else {
        unreachable!()
    };
    let mut notes = Vec::new();
    let mut status = GateStatus::Pass;
    fn fail(status: &mut GateStatus, notes: &mut Vec<String>, msg: String) {
        *status = worst(*status, GateStatus::Fail);
        notes.push(msg);
    }

    // An uncited floor is a guess, and a guessed floor is worse than none
    // because you will act on it.
    match e.source_url.as_deref().map(str::trim) {
        None | Some("") => fail(
            &mut status,
            &mut notes,
            format!("{}: no source_url - uncited floor", e.id),
        ),
        Some(_) => {}
    }
    match e.retrieved.as_deref().map(str::trim) {
        None | Some("") => fail(
            &mut status,
            &mut notes,
            format!("{}: no retrieved date - uncited floor", e.id),
        ),
        Some(date) => match parse_iso_date(date) {
            None => fail(
                &mut status,
                &mut notes,
                format!("{}: retrieved {date:?} is not a YYYY-MM-DD date", e.id),
            ),
            Some(then) => {
                let days = today_days() - then;
                if days < 0 {
                    fail(
                        &mut status,
                        &mut notes,
                        format!("{}: retrieved {date} is in the future", e.id),
                    );
                } else if days > 180 {
                    notes.push(format!(
                        "{}: retrieved {days} days ago - re-read the page, terms change",
                        e.id
                    ));
                }
            }
        },
    }

    // An UNSTATED floor is not a Low floor. It is a page that has not been
    // read, and it must not be read as permissive.
    match e.floor_status.as_deref() {
        None => fail(
            &mut status,
            &mut notes,
            format!(
                "{}: floor_status missing - record whether the floor was documented, \
                 unstated or conflicting",
                e.id
            ),
        ),
        Some("unstated") => fail(
            &mut status,
            &mut notes,
            format!(
                "{}: floor_status 'unstated' - no floor is documented. That is not evidence \
                 that Low pays; it is evidence the page has not been read",
                e.id
            ),
        ),
        Some("conflicting") => fail(
            &mut status,
            &mut notes,
            format!(
                "{}: floor_status 'conflicting' - the program's own materials disagree. \
                 The reward table governs; resolve it before hunting",
                e.id
            ),
        ),
        Some("documented") => {}
        Some(other) => fail(
            &mut status,
            &mut notes,
            format!("{}: unknown floor_status {other:?}", e.id),
        ),
    }

    let declared = e.floor_severity.as_deref().and_then(Severity::parse);
    match e.tier_shape.as_deref() {
        None => fail(
            &mut status,
            &mut notes,
            format!("{}: tier_shape missing", e.id),
        ),
        Some("unknown") => fail(
            &mut status,
            &mut notes,
            format!(
                "{}: tier_shape 'unknown' - NOT permissive, the page has not been read",
                e.id
            ),
        ),
        Some(shape) => match Severity::floor_of_shape(shape) {
            None => fail(
                &mut status,
                &mut notes,
                format!("{}: unknown tier_shape {shape:?}", e.id),
            ),
            Some(implied) => {
                if let Some(d) = declared {
                    if d != implied {
                        notes.push(format!(
                            "{}: floor_severity '{}' disagrees with tier_shape '{shape}' \
                             (implies '{}') - using the higher floor",
                            e.id,
                            d.label(),
                            implied.label()
                        ));
                    }
                }
                let floor = declared.map_or(implied, |d| d.max(implied));
                let narrow = floor >= Severity::High;
                match inp.severity() {
                    Some(sev) if sev < floor => fail(
                        &mut status,
                        &mut notes,
                        format!(
                            "{}: a {} finding is below this program's floor ({}) - it pays nothing",
                            e.id,
                            sev.label(),
                            floor.label()
                        ),
                    ),
                    Some(sev) => notes.push(format!(
                        "{}: tier_shape '{shape}'; a {} finding reaches the floor ({})",
                        e.id,
                        sev.label(),
                        floor.label()
                    )),
                    None if narrow => {
                        status = worst(status, GateStatus::Unknown);
                        notes.push(format!(
                            "{}: tier_shape '{shape}' pays nothing below {} - state the thesis \
                             severity with --severity or a 'Severity:' line",
                            e.id,
                            floor.label()
                        ));
                    }
                    None => notes.push(format!(
                        "{}: tier_shape '{shape}' (floor: {})",
                        e.id,
                        floor.label()
                    )),
                }
            }
        },
    }
    for c in &e.conflicts {
        notes.push(format!("{}: CONFLICT - {c}", e.id));
    }
    (status, notes.join("; "))
}

/// The prose of a finding, without its proof and recommendation sections.
fn claim_prose(text: &str) -> String {
    split_sections(text)
        .into_iter()
        .filter(|s| !matches!(s.kind, SectionKind::Poc | SectionKind::Recommendation))
        .map(|s| s.body)
        .collect::<Vec<_>>()
        .join("\n")
}

fn gate_exclusions(inp: &GateInputs<'_>) -> (GateStatus, String) {
    let program = inp.program();
    if let Some(r) = program_unavailable(&program, "exclusions") {
        return r;
    }
    let Program::One(e) = program else {
        unreachable!()
    };
    if e.exclusions.is_empty() {
        return (
            GateStatus::Fail,
            format!("{}: no exclusion clauses transcribed", e.id),
        );
    }
    let mut problems = Vec::new();
    let mut untested = Vec::new();
    let mut compiled: Vec<(usize, &Exclusion, Regex)> = Vec::new();
    for (i, x) in e.exclusions.iter().enumerate() {
        if x.clause.as_deref().unwrap_or("").trim().is_empty() {
            problems.push(format!("{}: exclusion[{i}] missing verbatim clause", e.id));
        }
        match x.pattern.as_deref().map(str::trim) {
            None | Some("") => untested.push(format!("exclusion[{i}] ({})", x.class)),
            Some(p) => match compile_pattern(p) {
                Ok(re) => compiled.push((i, x, re)),
                Err(_) => problems.push(format!(
                    "{}: exclusion[{i}] pattern is not a valid regex (Rust regex syntax: no \
                     lookaround, no backreferences)",
                    e.id
                )),
            },
        }
    }
    if !problems.is_empty() {
        return (GateStatus::Fail, problems.join("; "));
    }
    let mut classes: Vec<&str> = e.exclusions.iter().map(|x| x.class.as_str()).collect();
    classes.sort_unstable();
    classes.dedup();
    let mut tail = String::new();
    if !untested.is_empty() {
        tail.push_str(&format!(
            "; no pattern, so not tested: {}",
            untested.join(", ")
        ));
    }
    if let Some(d) = e
        .post_audit_delta
        .as_deref()
        .filter(|d| !d.trim().is_empty())
    {
        tail.push_str(&format!("; post-audit delta recorded: {d}"));
    }

    let Some(text) = inp.finding else {
        return (
            GateStatus::Pass,
            format!(
                "{} clause(s) transcribed and well-formed across {} class(es); not tested \
                 against a finding - supply one to test them{tail}",
                e.exclusions.len(),
                classes.len()
            ),
        );
    };

    let prose = claim_prose(text);
    let sections = split_sections(&prose);
    let mut matched = Vec::new();
    for (i, x, re) in &compiled {
        let hit = sections
            .iter()
            .flat_map(|s| s.sentences.iter())
            .find_map(|s| find_unnegated(re, s, true));
        if let Some(h) = hit {
            matched.push(format!(
                "exclusion[{i}] '{}' matches \"{}\" - clause: \"{}\"",
                x.class,
                h.snippet,
                collapse(x.clause.as_deref().unwrap_or(""))
            ));
        }
    }
    if matched.is_empty() {
        (
            GateStatus::Pass,
            format!(
                "{} pattern(s) tested against the finding; none matched{tail}",
                compiled.len()
            ),
        )
    } else {
        (
            GateStatus::Fail,
            format!(
                "{}: {}. A match is a prompt to read the clause, not an automatic reject{tail}",
                e.id,
                matched.join("; ")
            ),
        )
    }
}

static URL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b[a-z][a-z0-9+.-]*://\S+").expect("URL compiles"));
static PATH: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:[A-Za-z0-9_.\-]+/)*[A-Za-z0-9_\-][A-Za-z0-9_.\-]*\.[A-Za-z]{2,5}\b")
        .expect("PATH compiles")
});

/// Path-like tokens in a text, normalised to forward slashes, URLs excluded.
pub(crate) fn extract_paths(text: &str) -> Vec<String> {
    let no_urls = URL.replace_all(text, " ");
    let mut out: Vec<String> = Vec::new();
    for m in PATH.find_iter(&no_urls) {
        let p = m.as_str().trim_start_matches("./").replace('\\', "/");
        let stem = p.rsplit('/').next().unwrap_or(&p);
        let base = stem.split('.').next().unwrap_or("");
        if !p.contains('/') && base.chars().count() < 2 {
            continue;
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

/// Is `path` (as cited in a finding) inside the scope entry `entry`?
fn in_scope(path: &str, entry: &str) -> bool {
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

fn file_stem(entry: &str) -> Option<&str> {
    let name = entry.trim().rsplit(['/', '\\']).next()?;
    let stem = name.split('.').next()?;
    (stem.chars().count() >= 3 && !stem.contains('*')).then_some(stem)
}

fn mentions_word(text: &str, word: &str) -> bool {
    Regex::new(&format!(r"\b{}\b", regex::escape(word))).is_ok_and(|r| r.is_match(text))
}

fn gate_scope(inp: &GateInputs<'_>) -> (GateStatus, String) {
    let Some(scope) = inp.scope else {
        return (
            GateStatus::Unknown,
            "no scope tree supplied - unknown, not pass".into(),
        );
    };
    if scope.is_empty() {
        return (GateStatus::Fail, "scope list is empty".into());
    }
    let Some(text) = inp.finding else {
        return (
            GateStatus::Pass,
            format!(
                "{} scoped path(s); not traced against a finding - supply one to trace the \
                 root cause",
                scope.len()
            ),
        );
    };
    let prose = claim_prose(text);
    let paths = extract_paths(&prose);
    let mut inside: Vec<String> = Vec::new();
    let mut outside: Vec<String> = Vec::new();
    for p in &paths {
        let bare = p.split(':').next().unwrap_or(p);
        if scope.iter().any(|s| in_scope(bare, s)) {
            inside.push(p.clone());
        } else {
            outside.push(p.clone());
        }
    }
    for s in scope {
        if let Some(stem) = file_stem(s) {
            if !inside.iter().any(|p| p.contains(stem)) && mentions_word(&prose, stem) {
                inside.push(stem.to_string());
            }
        }
    }
    if !inside.is_empty() {
        let mut detail = format!("the finding cites scoped code: {}", inside.join(", "));
        if !outside.is_empty() {
            detail.push_str(&format!(
                "; it also cites {} - confirm none of those is the root cause",
                outside.join(", ")
            ));
        }
        (GateStatus::Pass, detail)
    } else if !outside.is_empty() {
        (
            GateStatus::Fail,
            format!(
                "the finding cites only paths outside the scope list: {}. Trace the root \
                 cause, not the impact site",
                outside.join(", ")
            ),
        )
    } else {
        (
            GateStatus::Unknown,
            "the finding names no scoped file or contract, so the root cause could not be \
             traced - name the file"
                .into(),
        )
    }
}

fn gate_prior_audits(inp: &GateInputs<'_>) -> (GateStatus, String) {
    let mut notes = Vec::new();
    let mut covered_paths = 0usize;
    let mut known: Vec<&str> = Vec::new();
    let mut covers: Vec<(&str, &str)> = Vec::new();
    let mut missing_covers = false;
    let program = inp.program();
    if let Program::Ambiguous(n) = program {
        notes.push(format!(
            "the ledger holds {n} programs - choose one with --program <id> to read its audits"
        ));
    }
    if let Program::One(e) = program {
        for (i, a) in e.prior_audits.iter().enumerate() {
            let firm = a.firm.as_deref().unwrap_or("unnamed firm");
            if a.firm.is_none() {
                notes.push(format!("{}: prior_audits[{i}] has no firm name", e.id));
            }
            if a.url.is_none() {
                notes.push(format!("{}: prior_audits[{i}] has no url", e.id));
            }
            if a.covers.is_empty() {
                missing_covers = true;
                notes.push(format!(
                    "{}: prior_audits[{i}] has no 'covers' list - read the finding, not the \
                     contents page",
                    e.id
                ));
            } else {
                covered_paths += a.covers.len();
                covers.extend(a.covers.iter().map(|c| (firm, c.as_str())));
            }
            known.extend(a.known_issues.iter().map(String::as_str));
        }
    }
    let Some(prior) = inp.prior_audits else {
        let mut detail = "no prior-audit surface supplied - unknown, not pass".to_string();
        if !notes.is_empty() {
            detail.push_str("; ");
            detail.push_str(&notes.join("; "));
        }
        return (GateStatus::Unknown, detail);
    };
    if prior.is_empty() && covered_paths == 0 {
        return (GateStatus::Fail, "prior-audit list is empty".into());
    }
    if missing_covers {
        return (GateStatus::Fail, notes.join("; "));
    }
    let mut detail = format!(
        "{} prior-artifact(s) supplied, {} path(s) covered",
        prior.len(),
        covered_paths
    );
    if let Some(text) = inp.finding {
        let finding_tokens = tokens(text);
        let resembles: Vec<String> = known
            .iter()
            .filter(|k| contained(&tokens(k), &finding_tokens).is_some())
            .map(|k| collapse(k))
            .collect();
        if !resembles.is_empty() {
            return (
                GateStatus::Fail,
                format!(
                    "the finding resembles a prior-audit known issue: {}. Read it before \
                     submitting",
                    resembles.join("; ")
                ),
            );
        }
        let prose = claim_prose(text);
        let paths = extract_paths(&prose);
        let hit: Vec<String> = covers
            .iter()
            .filter(|(_, c)| {
                paths
                    .iter()
                    .any(|p| in_scope(p.split(':').next().unwrap_or(p), c))
            })
            .map(|(f, c)| format!("{c} ({f})"))
            .collect();
        if hit.is_empty() {
            detail.push_str("; no cited path is in a prior audit's coverage");
        } else {
            detail.push_str(&format!(
                "; cited path(s) covered by a prior audit: {} - read that report's findings \
                 for this function",
                hit.join(", ")
            ));
        }
    }
    if !notes.is_empty() {
        detail.push_str("; ");
        detail.push_str(&notes.join("; "));
    }
    (GateStatus::Pass, detail)
}

static INTENT_MARKER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(by design|as designed|intended|intentional(ly)?|should preserve|expected to survive|not penali[sz]ed|documented behaviou?r)\b",
    )
    .expect("INTENT_MARKER compiles")
});
static IDENT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"`([^`\n]{3,80})`|\b([A-Za-z_][A-Za-z0-9_]{2,})\(|\b([a-z]+[A-Z][A-Za-z0-9]*)\b|\b([A-Z][a-z0-9]+[A-Z][A-Za-z0-9]*)\b|\b([a-z][a-z0-9]*_[a-z0-9_]+)\b",
    )
    .expect("IDENT compiles")
});

/// Code identifiers a finding names: backticked spans, calls, camelCase,
/// PascalCase compounds and snake_case.
pub(crate) fn identifiers(text: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for c in IDENT.captures_iter(text) {
        for i in 1..=5 {
            if let Some(m) = c.get(i) {
                for part in m
                    .as_str()
                    .split(|ch: char| !(ch.is_alphanumeric() || ch == '_'))
                {
                    if part.chars().count() >= 4 && part.chars().any(|ch| ch.is_alphabetic()) {
                        out.insert(part.to_string());
                    }
                }
            }
        }
    }
    out
}

fn gate_tests(inp: &GateInputs<'_>) -> (GateStatus, String) {
    let Some(files) = inp.tests else {
        return (
            GateStatus::Unknown,
            "no test suite supplied - unknown, not pass".into(),
        );
    };
    let mut hits: Vec<(&str, usize, String)> = Vec::new();
    for (path, text) in files {
        for (n, line) in text.lines().enumerate() {
            if let Some(m) = INTENT_MARKER.find(line) {
                hits.push((path.as_str(), n + 1, m.as_str().to_lowercase()));
            }
        }
    }
    let show = |list: &[&(&str, usize, String)]| -> String {
        let mut parts: Vec<String> = list
            .iter()
            .take(5)
            .map(|(p, n, m)| format!("{p}:{n} ({m})"))
            .collect();
        if list.len() > 5 {
            parts.push(format!("and {} more", list.len() - 5));
        }
        parts.join(", ")
    };
    if hits.is_empty() {
        return (
            GateStatus::Pass,
            format!("no intent markers in {} test file(s)", files.len()),
        );
    }
    let all: Vec<&(&str, usize, String)> = hits.iter().collect();
    let Some(text) = inp.finding else {
        return (
            GateStatus::Unknown,
            format!(
                "{} intent marker(s) in the suite: {}. Supply a finding to check whether they \
                 cover it",
                hits.len(),
                show(&all)
            ),
        );
    };
    let idents = identifiers(text);
    if idents.is_empty() {
        return (
            GateStatus::Unknown,
            format!(
                "the finding names no code identifier, so the suite's {} intent marker(s) could \
                 not be tied to it: {}",
                hits.len(),
                show(&all)
            ),
        );
    }
    let relevant_files: BTreeSet<&str> = files
        .iter()
        .filter(|(_, t)| idents.iter().any(|i| mentions_word(t, i)))
        .map(|(p, _)| p.as_str())
        .collect();
    let relevant: Vec<&(&str, usize, String)> = hits
        .iter()
        .filter(|(p, _, _)| relevant_files.contains(p))
        .collect();
    if relevant.is_empty() {
        (
            GateStatus::Pass,
            format!(
                "{} intent marker(s) in the suite, none in a test that names this finding's \
                 identifiers",
                hits.len()
            ),
        )
    } else {
        (
            GateStatus::Fail,
            format!(
                "intent markers in tests that exercise this finding's code: {}. Read the test \
                 before submitting - this is Not Applicable, not Informational",
                show(&relevant)
            ),
        )
    }
}

fn gate_fork(fork_of: Option<&str>) -> (GateStatus, String) {
    match fork_of.map(|f| f.trim()) {
        None => (
            GateStatus::Unknown,
            "fork status unknown - unknown, not pass".into(),
        ),
        Some(f)
            if f.is_empty() || f.eq_ignore_ascii_case("none") || f.eq_ignore_ascii_case("no") =>
        {
            (GateStatus::Pass, "not a fork".into())
        }
        Some(name) => (
            GateStatus::Fail,
            format!(
                "fork of {} - check whether the behaviour exists identically upstream and is \
                 documented there",
                collapse(name)
            ),
        ),
    }
}

/// G7 - the author's own submission history.
///
/// Fails closed: with no log supplied, this reports Unknown, never Pass. The
/// gate exists because a regex over a finding's prose cannot see the
/// researcher's own earlier submissions; in the source log 12 of 113 rows
/// repeat a family already present.
fn gate_own_history(log: Option<&[LogEntry]>, finding: Option<&str>) -> (GateStatus, String) {
    let Some(entries) = log else {
        return (
            GateStatus::Unknown,
            "no own-history log supplied - unknown, not pass. Run vg init to scaffold one".into(),
        );
    };
    if entries.is_empty() {
        return (
            GateStatus::Pass,
            "own-history log is empty - no earlier submission to repeat".into(),
        );
    }
    let Some(text) = finding else {
        return (
            GateStatus::Unknown,
            format!(
                "{} logged submission(s); no finding text supplied to compare against",
                entries.len()
            ),
        );
    };
    let matches = probable_repeats(entries, text);
    if matches.is_empty() {
        (
            GateStatus::Pass,
            format!(
                "{} logged submission(s); no probable repeat above the similarity threshold",
                entries.len()
            ),
        )
    } else {
        let names: Vec<String> = matches
            .iter()
            .map(|(e, score)| {
                format!(
                    "{} ({:.2}, outcome: {})",
                    collapse(&e.family),
                    score,
                    collapse(&e.outcome)
                )
            })
            .collect();
        (
            GateStatus::Fail,
            format!(
                "probable repeat of {} earlier submission(s): {}. Read the earlier outcome \
                 before submitting",
                matches.len(),
                names.join(", ")
            ),
        )
    }
}

/// D1 - the known-issues corpus and, when supplied, the identifier index.
fn gate_known_issues(
    known: Option<&[KnownIssue]>,
    index: Option<&[IndexedFinding]>,
    finding: Option<&str>,
) -> (GateStatus, String) {
    if known.is_none() && index.is_none() {
        return (
            GateStatus::Unknown,
            "no known-issues corpus or prior-finding index supplied - supply \
             --known-issues or --index"
                .into(),
        );
    }
    let Some(issues) = known else {
        return gate_index(index, finding);
    };
    if issues.is_empty() {
        if index.is_some() {
            let (status, detail) = gate_index(index, finding);
            return (
                status,
                format!("the regex corpus is empty: 0 known issues transcribed. {detail}"),
            );
        }
        return (
            GateStatus::Pass,
            "the corpus is empty: 0 known issues transcribed. That is a pass only if the \
             program really lists none"
                .into(),
        );
    }
    let Some(text) = finding else {
        return (
            GateStatus::Unknown,
            format!(
                "{} known issue(s) loaded; no finding text supplied to compare against",
                issues.len()
            ),
        );
    };
    let (found, warnings) = match_known_issues(issues, text);
    let mut parts = Vec::new();
    if !found.is_empty() {
        let ids: Vec<&str> = found.iter().map(|f| f.rule.as_str()).collect();
        parts.push(format!("matches {}", ids.join(", ")));
    }
    if !warnings.is_empty() {
        parts.push(format!("unusable corpus entries: {}", warnings.join("; ")));
    }
    let (index_status, index_detail) = if index.is_some() {
        gate_index(index, finding)
    } else {
        (GateStatus::Pass, String::new())
    };
    if !index_detail.is_empty() {
        parts.push(index_detail);
    }
    let status = if parts.is_empty() && index_status == GateStatus::Pass {
        GateStatus::Pass
    } else if !found.is_empty() || !warnings.is_empty() || index_status == GateStatus::Fail {
        GateStatus::Fail
    } else {
        worst(GateStatus::Pass, index_status)
    };
    if status == GateStatus::Pass && parts.is_empty() {
        (
            GateStatus::Pass,
            format!(
                "{} known issue(s) checked; none matched. A known-issue matcher cannot catch a \
                 concurrent duplicate",
                issues.len()
            ),
        )
    } else if status == GateStatus::Pass {
        (
            GateStatus::Pass,
            format!(
                "{} known issue(s) checked; none matched. {}",
                issues.len(),
                parts.join("; ")
            ),
        )
    } else {
        (status, parts.join("; "))
    }
}

/// Identifier ranking. A near miss is reported and does not fail the gate.
/// The fail line is the uncalibrated threshold in `index::FAIL_SCORE`.
fn gate_index(index: Option<&[IndexedFinding]>, finding: Option<&str>) -> (GateStatus, String) {
    let Some(index) = index else {
        return (
            GateStatus::Unknown,
            "no prior-finding index supplied".into(),
        );
    };
    if index.is_empty() {
        return (GateStatus::Pass, "the prior-finding index is empty".into());
    }
    let Some(text) = finding else {
        return (
            GateStatus::Unknown,
            format!(
                "{} prior finding(s) indexed; no finding text supplied to compare",
                index.len()
            ),
        );
    };
    let ranked = crate::index::rank(text, index);
    if ranked.is_empty() {
        return (
            GateStatus::Pass,
            format!(
                "{} prior finding(s) indexed; no shared code identifiers. Word overlap is not \
                 scored",
                index.len()
            ),
        );
    }
    let shown: Vec<String> = ranked.iter().map(crate::index::summary).collect();
    if ranked.iter().any(|r| r.fails) {
        (
            GateStatus::Fail,
            format!(
                "identifier overlap with a prior finding (threshold {:.2}, uncalibrated): {}",
                crate::index::FAIL_SCORE,
                shown.join(" | ")
            ),
        )
    } else {
        (
            GateStatus::Pass,
            format!(
                "closest prior findings are below the fail threshold {:.2}: {}",
                crate::index::FAIL_SCORE,
                shown.join(" | ")
            ),
        )
    }
}

/// Deterministic similarity between a finding and a logged family: the share of
/// the family label's tokens that appear in the finding (containment).
///
/// Jaccard over the whole document cannot work here: a 300-token finding and
/// a 5-token label share at most 5/300. Containment asks the question the gate
/// actually has - does the draft talk about the family I already filed? A
/// probable repeat needs at least REPEAT_THRESHOLD of the label's tokens and at
/// least two shared tokens (one, for a one-token label). No model, no network,
/// no randomness.
pub const REPEAT_THRESHOLD: f64 = 0.6;

fn contained(label: &BTreeSet<String>, finding: &BTreeSet<String>) -> Option<f64> {
    if label.is_empty() {
        return None;
    }
    let shared = label.intersection(finding).count();
    let score = shared as f64 / label.len() as f64;
    (shared >= label.len().min(2) && score >= REPEAT_THRESHOLD).then_some(score)
}

pub fn probable_repeats<'a>(entries: &'a [LogEntry], text: &str) -> Vec<(&'a LogEntry, f64)> {
    let finding = tokens(text);
    if finding.is_empty() {
        return Vec::new();
    }
    let mut out: Vec<(&LogEntry, f64)> = entries
        .iter()
        .filter_map(|e| contained(&tokens(&e.family), &finding).map(|s| (e, s)))
        .collect();
    out.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    out
}

const STOP: [&str; 40] = [
    "the", "and", "for", "are", "but", "not", "can", "has", "was", "its", "any", "all", "one",
    "two", "via", "per", "our", "out", "may", "now", "own", "who", "why", "how", "off", "yet",
    "you", "that", "this", "with", "from", "when", "then", "than", "into", "over", "have", "been",
    "which", "will",
];

/// Lowercased alphanumeric tokens of 3+ characters, minus stop words, with a
/// trailing plural 's' dropped.
fn tokens(text: &str) -> BTreeSet<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|t| t.chars().count() >= 3)
        .map(|t| t.to_lowercase())
        .filter(|t| !STOP.contains(&t.as_str()))
        .map(|t| {
            if t.chars().count() > 4 && t.ends_with('s') && !t.ends_with("ss") {
                t[..t.len() - 1].to_string()
            } else {
                t
            }
        })
        .collect()
}

// ---------------------------------------------------------------- known issues

/// Match a finding against a user-supplied known-issue corpus.
///
/// This is a KNOWN-ISSUE MATCHER, not a duplicate catcher. It can only see what
/// the user already transcribed, so it CANNOT catch a concurrent duplicate: a
/// concurrent duplicate is, by definition, not in any corpus yet.
///
/// A match is reported as a KI finding with a hard verdict, because a known
/// issue is Not Applicable rather than Informational - a materially worse
/// outcome than an unpaid finding.
pub fn match_known_issues(issues: &[KnownIssue], text: &str) -> (Vec<Finding>, Vec<String>) {
    let mut findings = Vec::new();
    let mut warnings = Vec::new();
    let sections = split_sections(text);
    let mut seen_ids = BTreeSet::new();

    for issue in issues {
        if !seen_ids.insert(issue.id.as_str()) {
            warnings.push(format!("{}: duplicate known-issue id", collapse(&issue.id)));
        }
        if issue.patterns.is_empty() {
            warnings.push(format!(
                "{}: no patterns, so it cannot be matched - transcribe a regex",
                collapse(&issue.id)
            ));
            continue;
        }
        let mut evidence: Vec<String> = Vec::new();
        let mut sentences: Vec<String> = Vec::new();
        for pattern in &issue.patterns {
            let re = match compile_pattern(pattern) {
                Ok(re) => re,
                Err(e) => {
                    warnings.push(format!(
                        "{}: invalid regex {pattern:?}: {e}",
                        collapse(&issue.id)
                    ));
                    continue;
                }
            };
            if let Some(hit) = sections
                .iter()
                .flat_map(|s| s.sentences.iter())
                .find_map(|s| find_unnegated(&re, s, true))
            {
                evidence.push(hit.snippet);
                sentences.push(hit.sentence);
            }
            if evidence.len() >= 2 {
                break;
            }
        }
        if !evidence.is_empty() {
            findings.push(Finding {
                rule: format!("KI:{}", collapse(&issue.id)),
                title: format!("Matches known issue - {}", collapse(&issue.summary)),
                group: "K".into(),
                verdict: Verdict::Hard,
                weight: 60,
                evidence,
                matched_sentences: sentences,
                fix: "Do not submit. A known issue is Not Applicable, not Informational - a \
                      materially worse outcome than an unpaid finding. Read the referenced \
                      source and confirm the root cause is the same, not merely adjacent."
                    .into(),
                doc: issue
                    .source
                    .as_deref()
                    .map(collapse)
                    .unwrap_or_else(|| "user-supplied known-issues corpus".into()),
                evidence_kind: Evidence::None,
                origin_rows: 0,
            });
        }
    }
    (findings, warnings)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(id: &str, verdict: Verdict, any_of: &[&str], none_of: &[&str]) -> CompiledRule {
        CompiledRule::new(Rule {
            id: id.into(),
            title: "t".into(),
            group: "A".into(),
            verdict,
            weight: 30,
            any_of: any_of.iter().map(|s| s.to_string()).collect(),
            all_of: vec![],
            none_of: none_of.iter().map(|s| s.to_string()).collect(),
            escalate_on: vec![],
            downgrade_on: vec![],
            escalate_to: None,
            structural: None,
            none_of_scope: NoneOfScope::Document,
            ignore_sections: vec![],
            fix: "f".into(),
            doc: "d".into(),
            origin_rows: 0,
            evidence: Evidence::None,
            last_validated: None,
            fp_rate: None,
            negation_aware: true,
            suppress_document: vec![],
            default_emit: true,
        })
        .unwrap()
    }

    fn gate(id: &str) -> Gate {
        Gate {
            id: id.into(),
            title: "t".into(),
            question: "q".into(),
            needs: "n".into(),
            blocks: "b".into(),
            weight: 30,
            checks: vec![],
            evidence: Evidence::None,
            origin_rows: 0,
            implemented_by: None,
        }
    }

    fn one(id: &str, inp: GateInputs<'_>) -> GateOutcome {
        evaluate_gates(&[gate(id)], &inp).remove(0)
    }

    fn entry() -> LedgerEntry {
        LedgerEntry {
            id: "prog".into(),
            tier_shape: Some("medium_and_above".into()),
            floor_severity: Some("medium".into()),
            floor_status: Some("documented".into()),
            source_url: Some("https://example.org/rewards".into()),
            retrieved: Some("2026-10-01".into()),
            conflicts: vec![],
            exclusions: vec![],
            prior_audits: vec![],
            post_audit_delta: None,
        }
    }

    #[test]
    fn fires_on_match() {
        let r = rule("P1", Verdict::Hard, &["not currently"], &[]);
        assert!(r.matches("This is not currently exploitable.").is_some());
    }

    #[test]
    fn does_not_fire_without_match() {
        let r = rule("P1", Verdict::Hard, &["not currently"], &[]);
        assert!(r.matches("The attacker drains the vault.").is_none());
    }

    #[test]
    fn none_of_suppresses() {
        let r = rule("P3", Verdict::Hard, &["front[- ]?run"], &["atomic"]);
        assert!(r.matches("The attacker front-runs the operator.").is_some());
        assert!(r
            .matches("Front-run, but atomic in one transaction.")
            .is_none());
    }

    #[test]
    fn no_one_but_the_admin_is_an_affirmation_not_a_negation() {
        let r = rule(
            "P2",
            Verdict::Hard,
            &["(admin|owner|guardian) (can|may)"],
            &[],
        );
        assert!(
            r.matches("No one but the admin can call setParams().")
                .is_some(),
            "exclusive admin claim must fire"
        );
        assert!(r
            .matches("No one other than the owner can call setParams().")
            .is_some());
        assert!(r
            .matches("Nobody except the guardian can call setParams().")
            .is_some());
        assert!(
            r.matches("There is no admin can-call in this path.")
                .is_none(),
            "a real denial must stay silent"
        );
        assert!(r
            .matches("This is rather than an admin can call it.")
            .is_none());
    }

    #[test]
    fn default_routes_a_claim_heuristic_and_strict_kills_it() {
        let mut heuristic = finding(Verdict::Hard, 40);
        heuristic.rule = "P2".into();
        let (routed, score) = outcome(std::slice::from_ref(&heuristic), &[], 50);
        assert_eq!(routed, Outcome::Route);
        assert_eq!(score, 60);
        let (routed_explicit, _) = decide(std::slice::from_ref(&heuristic), &[], 50, true);
        assert_eq!(routed_explicit, Outcome::Route);
        let (strict, _) = outcome_strict(std::slice::from_ref(&heuristic), &[], 50);
        assert_eq!(strict, Outcome::Kill);
        let (strict_explicit, _) = decide(std::slice::from_ref(&heuristic), &[], 50, false);
        assert_eq!(strict_explicit, Outcome::Kill);
        let mut structured = finding(Verdict::Hard, 40);
        structured.rule = "CITE".into();
        let (killed, _) = outcome(std::slice::from_ref(&structured), &[], 50);
        assert_eq!(killed, Outcome::Kill);
        let mut known = finding(Verdict::Hard, 40);
        known.rule = "KI:KI-1".into();
        let (known_killed, _) = outcome(std::slice::from_ref(&known), &[], 50);
        assert_eq!(known_killed, Outcome::Kill);
    }

    #[test]
    fn negation_window_discards_a_negated_match() {
        let r = rule("P3", Verdict::Hard, &["front[- ]?run"], &[]);
        assert!(r
            .matches("There is no front-run exposure in this path.")
            .is_none());
        let far = "There is no realistic scenario at all in which the attacker can \
                   practically arrange the state such that they front-run the operator.";
        assert!(r.matches(far).is_some(), "distant cue must not suppress");
    }

    #[test]
    fn typographic_apostrophes_negate_like_ascii_ones() {
        let r = rule("P3", Verdict::Hard, &["front[- ]?run"], &[]);
        assert!(r.matches("This path doesn't front-run anything.").is_none());
        assert!(r
            .matches("This path doesn\u{2019}t front-run anything.")
            .is_none());
    }

    #[test]
    fn missing_is_a_negation_cue() {
        let r = rule("P2", Verdict::Hard, &["only(owner|admin)"], &[]);
        assert!(r
            .matches("The setFee function is missing the onlyOwner modifier.")
            .is_none());
    }

    #[test]
    fn negation_aware_false_ignores_the_window() {
        let mut r = rule("P3", Verdict::Hard, &["front[- ]?run"], &[]);
        r.rule.negation_aware = false;
        assert!(r
            .matches("There is no front-run exposure in this path.")
            .is_some());
    }

    #[test]
    fn sentence_scoped_none_of_only_suppresses_that_sentence() {
        let mut r = rule("P2", Verdict::Hard, &["admin can"], &["permissionless"]);
        r.rule.none_of_scope = NoneOfScope::Sentence;
        let text = "An admin can set the fee to zero. The path is permissionless for anyone else.";
        assert!(r.matches(text).is_some());
        let text = "An admin can set the fee to zero and the path is permissionless.";
        assert!(r.matches(text).is_none());
    }

    #[test]
    fn evidence_reports_the_snippet_and_its_sentence() {
        let r = rule("P7", Verdict::Soft, &["best[- ]practice"], &[]);
        let m = r
            .matches("Some preamble. This is a best-practice critique of the code. Trailing.")
            .expect("must fire");
        assert_eq!(m.evidence.len(), 1);
        assert!(m.evidence[0].contains("best-practice"));
        assert!(m.sentences[0].contains("critique of the code"));
    }

    #[test]
    fn all_of_requires_every_pattern() {
        let mut r = rule("P19", Verdict::Hard, &["permanent fund lock"], &[]);
        r.all_of = vec![Regex::new("(?i)\\breverts?\\b").unwrap()];
        assert!(r
            .matches("Permanent fund lock because it reverts.")
            .is_some());
        assert!(r
            .matches("Permanent fund lock with no failure mode.")
            .is_none());
    }

    #[test]
    fn downgrade_on_drops_a_weak_match() {
        let mut r = rule("P4", Verdict::Hard, &["Mock[A-Z]\\w*"], &[]);
        r.downgrade_on = vec![Regex::new("(?i)MockERC20").unwrap()];
        assert!(r.matches("The PoC uses MockERC20 as the token.").is_none());
        assert!(r
            .matches("The PoC uses MockVault instead of the real contract.")
            .is_some());
    }

    #[test]
    fn escalate_on_raises_the_verdict() {
        let mut r = rule("P4", Verdict::Warn, &["Mock[A-Z]\\w*"], &[]);
        r.escalate_on = vec![Regex::new("(?i)instead of the real").unwrap()];
        r.rule.escalate_to = Some(Verdict::Hard);
        let m = r
            .matches("The PoC uses MockVault instead of the real contract.")
            .expect("must fire");
        assert_eq!(m.verdict, Verdict::Hard);
        assert!(
            m.evidence[0].contains("instead of the real"),
            "the strong hit decides the verdict, so it is shown first"
        );
        let m = r
            .matches("The PoC uses MockVault somewhere.")
            .expect("must still fire as a note");
        assert_eq!(m.verdict, Verdict::Warn);
    }

    #[test]
    fn ignore_sections_skips_the_poc() {
        let mut r = rule("P5", Verdict::Hard, &["outside the scope"], &[]);
        r.rule.ignore_sections = vec![SectionKind::Poc];
        let text = "# Finding\n\nThe root cause is in scope.\n\n## Proof of concept\n\nThis is outside the scope.\n";
        assert!(r.matches(text).is_none(), "the PoC section must be ignored");
    }

    // ------------------------------------------------------ parsing

    #[test]
    fn headings_are_classified_by_whole_word() {
        assert_eq!(section_kind("Epoch accounting"), SectionKind::Other);
        assert_eq!(section_kind("Latest attestation"), SectionKind::Other);
        assert_eq!(section_kind("Proof of Concept"), SectionKind::Poc);
        assert_eq!(section_kind("Steps to reproduce"), SectionKind::Poc);
        assert_eq!(section_kind("Recommended fix"), SectionKind::Recommendation);
        assert_eq!(section_kind("Impact"), SectionKind::Claim);
    }

    #[test]
    fn hash_lines_inside_a_code_fence_are_not_headings() {
        let text = "# Title\n\n## Proof of concept\n\n```bash\n# run the impact script\nforge test\n```\n\nThe call reverts.\n";
        let s = split_sections(text);
        assert_eq!(
            s.len(),
            2,
            "{:?}",
            s.iter().map(|x| x.kind).collect::<Vec<_>>()
        );
        assert_eq!(s[1].kind, SectionKind::Poc);
        assert!(s[1].body.contains("The call reverts"));
    }

    #[test]
    fn a_hashtag_is_not_a_heading() {
        let s = split_sections("Intro text.\n#hashtag line\nMore text.\n");
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn sentences_do_not_split_inside_paths_numbers_or_macros() {
        let s = split_sentences(
            "See src/Vault.sol for 1.5 ETH, e.g. the assert!(x >= 0) call. Next one.",
        );
        let texts: Vec<&str> = s.iter().map(|x| x.text.trim()).collect();
        assert_eq!(texts.len(), 2, "{texts:?}");
        assert!(texts[0].contains("src/Vault.sol"));
        assert!(texts[0].contains("assert!(x >= 0)"));
    }

    #[test]
    fn collapse_strips_control_characters() {
        assert_eq!(collapse("a\u{1b}[31mred\u{7} b"), "a[31mred b");
    }

    // ------------------------------------------------------ structural P19

    fn structural_rule() -> CompiledRule {
        let mut r = rule("P19", Verdict::Hard, &[], &[]);
        r.rule.structural = Some(Structural::P19HeadlineVsProof);
        r
    }

    #[test]
    fn p19_fires_hard_when_the_proof_disproves_the_title() {
        let text = "# Permanent fund lock in the validator\n\n## Proof of concept\n\nThe transaction reverts and all state is rolled back.\n";
        let m = structural_rule().matches(text).expect("must fire");
        assert_eq!(m.verdict, Verdict::Hard);
    }

    #[test]
    fn p19_downgrades_when_a_persisted_consequence_is_asserted() {
        let text = "# Permanent fund lock in the validator\n\n## Proof of concept\n\nThe call reverts for the attacker, but the committed storage slot keeps the funds stranded after the call returns.\n";
        let m = structural_rule().matches(text).expect("must still fire");
        assert_eq!(m.verdict, Verdict::Warn);
    }

    #[test]
    fn p19_does_not_fire_on_a_loss_claim_with_no_failure_in_the_proof() {
        let text = "# Permanent fund lock in the validator\n\n## Proof of concept\n\nThe attacker withdraws the entire balance to their own address.\n";
        assert!(structural_rule().matches(text).is_none());
    }

    #[test]
    fn p19_ignores_total_value_locked_and_the_recommendation() {
        let text = "# Rounding bug lets attacker steal part of the total value locked\n\n## Proof of concept\n\nThe attacker balance increases by 1,200 USDC.\n\n## Recommendation\n\nThe function should revert when shares round to zero.\n";
        assert!(structural_rule().matches(text).is_none());
    }

    #[test]
    fn p19_does_not_fire_when_the_proof_shows_a_drain_and_a_revert() {
        let text = "# Total loss of vault funds\n\n## Proof of concept\n\nThe attacker drains the vault; the victim's later withdraw reverts.\n";
        assert!(structural_rule().matches(text).is_none());
    }

    #[test]
    fn p19_ignores_storage_mentioned_only_in_the_recommendation() {
        let text = "# Permanent fund lock\n\n## Proof of concept\n\nThe transaction reverts.\n\n## Recommendation\n\nUse a storage pointer.\n";
        let m = structural_rule().matches(text).expect("must fire");
        assert_eq!(m.verdict, Verdict::Hard);
    }

    // ------------------------------------------------------ outcome

    fn finding(verdict: Verdict, weight: u32) -> Finding {
        Finding {
            rule: "X".into(),
            title: "t".into(),
            group: "A".into(),
            verdict,
            weight,
            evidence: vec![],
            matched_sentences: vec![],
            fix: String::new(),
            doc: String::new(),
            evidence_kind: Evidence::None,
            origin_rows: 0,
        }
    }

    fn gate_with(status: GateStatus) -> GateOutcome {
        let mut g = one("G6", GateInputs::default());
        g.status = status;
        g
    }

    #[test]
    fn hard_fail_kills_even_at_a_high_score() {
        let (o, score) = outcome(&[finding(Verdict::Hard, 50)], &[], 50);
        assert_eq!(o, Outcome::Kill);
        assert_eq!(score, 50);
    }

    #[test]
    fn a_clean_finding_with_every_gate_passing_is_clear() {
        let (o, score) = outcome(&[], &[gate_with(GateStatus::Pass)], 50);
        assert_eq!(o, Outcome::Clear);
        assert_eq!(score, 100);
    }

    #[test]
    fn unknown_gate_with_no_findings_is_incomplete_not_clear() {
        let (o, _) = outcome(&[], &[gate_with(GateStatus::Unknown)], 50);
        assert_eq!(o, Outcome::Incomplete);
    }

    #[test]
    fn a_failed_gate_holds_the_finding() {
        let gates = [gate_with(GateStatus::Fail), gate_with(GateStatus::Unknown)];
        let (o, _) = outcome(&[], &gates, 50);
        assert_eq!(o, Outcome::Hold, "a failure must outrank an unknown");
    }

    #[test]
    fn warn_only_is_note_not_clear() {
        let (o, _) = outcome(&[finding(Verdict::Warn, 10)], &[], 50);
        assert_eq!(o, Outcome::Note);
    }

    #[test]
    fn a_zero_weight_note_does_not_hide_an_unknown_gate() {
        let (o, score) = outcome(
            &[finding(Verdict::Warn, 0)],
            &[gate_with(GateStatus::Unknown)],
            50,
        );
        assert_eq!(o, Outcome::Incomplete);
        assert_eq!(score, 100);
        let (noted, _) = outcome(
            &[finding(Verdict::Warn, 0)],
            &[gate_with(GateStatus::Pass)],
            50,
        );
        assert_eq!(noted, Outcome::Note);
    }

    #[test]
    fn rather_than_negates_what_follows_it() {
        let r = rule("P7", Verdict::Soft, &["best[- ]practice"], &[]);
        assert!(r.matches("This is a best-practice observation.").is_some());
        assert!(r
            .matches(
                "The behaviour differs from the description rather than a best-practice observation."
            )
            .is_none());
        assert!(r
            .matches("The report is instead of a best-practice note.")
            .is_none());
        assert!(r
            .matches("This is a non-best-practice defect with measured loss.")
            .is_none());
    }

    #[test]
    fn score_below_threshold_holds() {
        let (o, _) = outcome(&[finding(Verdict::Warn, 60)], &[], 50);
        assert_eq!(o, Outcome::Hold);
    }

    #[test]
    fn huge_weights_do_not_overflow() {
        let (_, score) = outcome(&[finding(Verdict::Warn, u32::MAX)], &[], 50);
        assert_eq!(score, 0);
    }

    #[test]
    fn no_outcome_word_is_submit() {
        for o in [
            Outcome::Kill,
            Outcome::Hold,
            Outcome::Note,
            Outcome::Incomplete,
            Outcome::Clear,
            Outcome::Route,
        ] {
            assert_ne!(o.word(), "SUBMIT");
        }
    }

    // ------------------------------------------------------ gates

    #[test]
    fn every_gate_reports_unknown_not_pass_without_input() {
        for id in ["G1", "G2", "G3", "G4", "G5", "G6", "G7", "D1", "A1"] {
            assert_eq!(
                one(id, GateInputs::default()).status,
                GateStatus::Unknown,
                "{id}"
            );
        }
    }

    #[test]
    fn g1_fails_a_finding_below_the_floor_and_passes_one_above_it() {
        let mut e = entry();
        e.tier_shape = Some("high_and_above".into());
        e.floor_severity = Some("high".into());
        let ledger = [e];
        let base = GateInputs {
            ledger: Some(&ledger),
            ..Default::default()
        };
        let low = GateInputs {
            finding: Some("**Severity:** Medium\n\nA bug."),
            ..base
        };
        assert_eq!(one("G1", low).status, GateStatus::Fail);
        let high = GateInputs {
            severity: Some(Severity::Critical),
            ..base
        };
        assert_eq!(one("G1", high).status, GateStatus::Pass);
        assert_eq!(
            one("G1", base).status,
            GateStatus::Unknown,
            "a narrow floor and no stated severity"
        );
    }

    #[test]
    fn g1_fails_a_missing_floor_status_and_a_future_date() {
        let mut e = entry();
        e.floor_status = None;
        let ledger = [e];
        let g = one(
            "G1",
            GateInputs {
                ledger: Some(&ledger),
                ..Default::default()
            },
        );
        assert_eq!(g.status, GateStatus::Fail);
        assert!(g.detail.contains("floor_status missing"));

        let mut e = entry();
        e.retrieved = Some("2999-01-01".into());
        let ledger = [e];
        let g = one(
            "G1",
            GateInputs {
                ledger: Some(&ledger),
                ..Default::default()
            },
        );
        assert_eq!(g.status, GateStatus::Fail);
        assert!(g.detail.contains("in the future"));
    }

    #[test]
    fn unstated_floor_is_a_failure_not_permissive() {
        let mut e = entry();
        e.tier_shape = Some("unknown".into());
        e.floor_status = Some("unstated".into());
        e.floor_severity = None;
        let ledger = [e];
        let g = one(
            "G1",
            GateInputs {
                ledger: Some(&ledger),
                ..Default::default()
            },
        );
        assert_eq!(g.status, GateStatus::Fail);
        assert!(g.detail.contains("unstated"));
    }

    #[test]
    fn a_multi_program_ledger_needs_a_program() {
        let mut b = entry();
        b.id = "other".into();
        let ledger = [entry(), b];
        let g = one(
            "G1",
            GateInputs {
                ledger: Some(&ledger),
                ..Default::default()
            },
        );
        assert_eq!(g.status, GateStatus::Unknown);
        assert!(g.detail.contains("--program"));
        let g = one(
            "G1",
            GateInputs {
                ledger: Some(&ledger),
                program: Some("other"),
                ..Default::default()
            },
        );
        assert_eq!(g.status, GateStatus::Pass);
    }

    #[test]
    fn g2_tests_the_exclusion_patterns_against_the_finding() {
        let mut e = entry();
        e.exclusions = vec![Exclusion {
            class: "privileged_role".into(),
            clause: Some("Attacks requiring privileged access".into()),
            pattern: Some("(owner|admin) (can|must)".into()),
        }];
        let ledger = [e];
        let hit = GateInputs {
            ledger: Some(&ledger),
            finding: Some("The admin can drain the pool by setting the fee."),
            ..Default::default()
        };
        let g = one("G2", hit);
        assert_eq!(g.status, GateStatus::Fail, "{}", g.detail);
        assert!(g.detail.contains("privileged_role"));
        let miss = GateInputs {
            finding: Some("Any caller re-enters withdraw and drains the pool."),
            ..hit
        };
        assert_eq!(one("G2", miss).status, GateStatus::Pass);
    }

    #[test]
    fn g2_reports_a_regex_vg_cannot_compile() {
        let mut e = entry();
        e.exclusions = vec![Exclusion {
            class: "privileged_role".into(),
            clause: Some("c".into()),
            pattern: Some("(?<!un)privileged".into()),
        }];
        let ledger = [e];
        let g = one(
            "G2",
            GateInputs {
                ledger: Some(&ledger),
                ..Default::default()
            },
        );
        assert_eq!(g.status, GateStatus::Fail);
        assert!(g.detail.contains("lookaround"));
    }

    #[test]
    fn g3_traces_the_cited_paths_into_the_scope() {
        let scope = vec!["src/Vault.sol".to_string(), "src/oracle/".to_string()];
        let base = GateInputs {
            scope: Some(&scope),
            ..Default::default()
        };
        let inside = GateInputs {
            finding: Some("The bug is in `Vault.sol:42`, in withdraw."),
            ..base
        };
        assert_eq!(one("G3", inside).status, GateStatus::Pass);
        let dir = GateInputs {
            finding: Some("The root cause is src/oracle/Feed.sol."),
            ..base
        };
        assert_eq!(one("G3", dir).status, GateStatus::Pass);
        let stem = GateInputs {
            finding: Some("Vault.withdraw sends before it zeroes the balance."),
            ..base
        };
        assert_eq!(one("G3", stem).status, GateStatus::Pass);
        let outside = GateInputs {
            finding: Some("The root cause is script/Deploy.s.sol, which sets the oracle."),
            ..base
        };
        assert_eq!(one("G3", outside).status, GateStatus::Fail);
        let none = GateInputs {
            finding: Some("The withdraw path sends funds before it updates state."),
            ..base
        };
        assert_eq!(one("G3", none).status, GateStatus::Unknown);
    }

    #[test]
    fn g3_ignores_urls_and_the_poc_section() {
        let scope = vec!["src/Vault.sol".to_string()];
        let inp = GateInputs {
            scope: Some(&scope),
            finding: Some(
                "Bug in Vault.sol, see https://example.org/report.pdf for context.\n\n## Proof of concept\n\ntest/Exploit.t.sol\n",
            ),
            ..Default::default()
        };
        let g = one("G3", inp);
        assert_eq!(g.status, GateStatus::Pass);
        assert!(!g.detail.contains("report.pdf"), "{}", g.detail);
        assert!(!g.detail.contains("Exploit"), "{}", g.detail);
    }

    #[test]
    fn g5_ties_intent_markers_to_the_finding() {
        let files = vec![
            (
                "test/Fee.t.sol".to_string(),
                "function test_fee() {\n  // the double fee in chargeFee is by design\n}\n"
                    .to_string(),
            ),
            (
                "test/Other.t.sol".to_string(),
                "// guard against unintended reentry\n// this is intended\n".to_string(),
            ),
        ];
        let base = GateInputs {
            tests: Some(&files),
            ..Default::default()
        };
        let tied = GateInputs {
            finding: Some("`chargeFee` charges the fee twice."),
            ..base
        };
        let g = one("G5", tied);
        assert_eq!(g.status, GateStatus::Fail, "{}", g.detail);
        assert!(g.detail.contains("test/Fee.t.sol:2"));
        let untied = GateInputs {
            finding: Some("`swapExact` skips the slippage check."),
            ..base
        };
        assert_eq!(one("G5", untied).status, GateStatus::Pass);
    }

    #[test]
    fn g5_does_not_treat_unintended_as_intended() {
        let files = vec![(
            "t.sol".to_string(),
            "// guard against unintended reentry\n".to_string(),
        )];
        let g = one(
            "G5",
            GateInputs {
                tests: Some(&files),
                ..Default::default()
            },
        );
        assert_eq!(g.status, GateStatus::Pass, "{}", g.detail);
    }

    #[test]
    fn g6_accepts_none_in_any_case() {
        for v in ["none", "None", "NO", " no ", ""] {
            assert_eq!(gate_fork(Some(v)).0, GateStatus::Pass, "{v:?}");
        }
        assert_eq!(gate_fork(Some("upstream-lib")).0, GateStatus::Fail);
    }

    #[test]
    fn g7_fails_closed_without_a_log_and_passes_an_empty_one() {
        assert_eq!(gate_own_history(None, Some("text")).0, GateStatus::Unknown);
        assert_eq!(
            gate_own_history(Some(&[]), Some("text")).0,
            GateStatus::Pass
        );
    }

    fn log(family: &str) -> Vec<LogEntry> {
        vec![LogEntry {
            family: family.into(),
            outcome: "rejected".into(),
            note: Some("closed as a known design tradeoff".into()),
            date: None,
        }]
    }

    #[test]
    fn g7_flags_a_repeat_inside_a_realistic_length_finding() {
        let text = "# Stale cached exchange rate lets an attacker mint underpriced shares\n\n\
            The vault caches the exchange rate between the underlying asset and the share token \
            and only refreshes the cached value when a deposit crosses the rebalance threshold. \
            Between refreshes the cached exchange rate becomes stale relative to the live oracle. \
            An attacker who observes the oracle moving upward can deposit at the stale cached \
            exchange rate, receive more shares than the deposit is worth, and redeem them after \
            the next refresh. In the fork test the attacker gains 3,100 USDC per cycle.";
        let entries = log("stale cached exchange rate");
        let (status, detail) = gate_own_history(Some(&entries), Some(text));
        assert_eq!(status, GateStatus::Fail, "{detail}");
        let unrelated = log("signature replay across chains");
        assert_eq!(
            gate_own_history(Some(&unrelated), Some(text)).0,
            GateStatus::Pass
        );
    }

    #[test]
    fn g7_flags_a_probable_repeat() {
        let entries = log("stale oracle price bound");
        let text = "A stale oracle price bound lets the attacker replay a withdrawal.";
        let (status, detail) = gate_own_history(Some(&entries), Some(text));
        assert_eq!(status, GateStatus::Fail);
        assert!(detail.contains("probable repeat"));
    }

    #[test]
    fn d1_runs_when_a_corpus_is_supplied() {
        let issues = vec![ki("KI-1", &["dust.{0,30}stuck"])];
        let hit = one(
            "D1",
            GateInputs {
                known_issues: Some(&issues),
                finding: Some("Dust is left stuck in the router."),
                ..Default::default()
            },
        );
        assert_eq!(hit.status, GateStatus::Fail);
        let miss = one(
            "D1",
            GateInputs {
                known_issues: Some(&issues),
                finding: Some("Reentrancy drains the vault."),
                ..Default::default()
            },
        );
        assert_eq!(miss.status, GateStatus::Pass);
        let unusable = vec![ki("KI-2", &[])];
        let bad = one(
            "D1",
            GateInputs {
                known_issues: Some(&unusable),
                finding: Some("anything"),
                ..Default::default()
            },
        );
        assert_eq!(bad.status, GateStatus::Fail);
    }

    #[test]
    fn severity_is_read_from_a_line_or_a_table_row() {
        assert_eq!(
            claimed_severity("**Severity:** High\n"),
            Some(Severity::High)
        );
        assert_eq!(
            claimed_severity("## Severity: critical\n"),
            Some(Severity::Critical)
        );
        assert_eq!(
            claimed_severity("| Severity | Medium |\n"),
            Some(Severity::Medium)
        );
        assert_eq!(
            claimed_severity("The severity of this is debatable.\n"),
            None
        );
    }

    #[test]
    fn iso_dates_are_strict() {
        assert!(parse_iso_date("2026-02-29").is_none());
        assert!(parse_iso_date("2024-02-29").is_some());
        assert!(parse_iso_date("2026-13-01").is_none());
        assert!(parse_iso_date("2026-1-01").is_none());
        assert_eq!(parse_iso_date("1970-01-01"), Some(0));
    }

    // ------------------------------------------------------ known issues

    fn ki(id: &str, patterns: &[&str]) -> KnownIssue {
        KnownIssue {
            id: id.into(),
            summary: "a previously accepted behaviour".into(),
            patterns: patterns.iter().map(|s| s.to_string()).collect(),
            source: Some("prior audit ref".into()),
        }
    }

    #[test]
    fn known_issue_matches_and_is_hard() {
        let issues = vec![ki("KI-1", &["burned amount", "re-?lock"])];
        let (findings, warnings) = match_known_issues(
            &issues,
            "The burned amount persists across unlock and re-lock cycles.",
        );
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].verdict, Verdict::Hard);
        assert!(findings[0].rule.starts_with("KI:"));
        assert!(warnings.is_empty(), "{warnings:?}");
    }

    #[test]
    fn known_issue_does_not_match_unrelated_text() {
        let issues = vec![ki("KI-1", &["burned amount"])];
        let (findings, _) =
            match_known_issues(&issues, "A signed order is replayable across vaults.");
        assert!(findings.is_empty());
    }

    #[test]
    fn known_issue_without_patterns_warns_rather_than_silently_passing() {
        let issues = vec![ki("KI-2", &[])];
        let (findings, warnings) = match_known_issues(&issues, "anything at all");
        assert!(findings.is_empty());
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("KI-2"));
    }

    #[test]
    fn known_issue_invalid_regex_and_duplicate_ids_warn() {
        let issues = vec![ki("KI-3", &["(unclosed"]), ki("KI-3", &["never-present"])];
        let (findings, warnings) = match_known_issues(&issues, "some finding text");
        assert!(findings.is_empty());
        assert!(
            warnings.iter().any(|w| w.contains("invalid regex")),
            "{warnings:?}"
        );
        assert!(
            warnings.iter().any(|w| w.contains("duplicate")),
            "{warnings:?}"
        );
    }
}
