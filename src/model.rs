//! Data model for the rule catalog and the ledger.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------- rules

/// The only catalog / gate-file format version this build understands.
pub const CATALOG_VERSION: u32 = 1;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuleFile {
    pub version: u32,
    pub rules: Vec<Rule>,
}

/// How a rule's provenance is supported.
#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Evidence {
    /// Derived from rows in the author's own rejection log.
    InSample,
    /// No supporting rows. The rule is a judgement, and says so.
    #[default]
    None,
}

impl Evidence {
    pub fn label(self) -> &'static str {
        match self {
            Evidence::InSample => "in-sample",
            Evidence::None => "none",
        }
    }
}

/// Where a `none_of` match suppresses a rule.
#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum NoneOfScope {
    /// A match anywhere in the document suppresses the rule.
    #[default]
    Document,
    /// Only a match in the same sentence as the `any_of` hit suppresses it.
    Sentence,
}

impl NoneOfScope {
    pub fn label(self) -> &'static str {
        match self {
            NoneOfScope::Document => "document",
            NoneOfScope::Sentence => "sentence",
        }
    }
}

/// The kind of a markdown section, decided from its heading.
#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SectionKind {
    /// Proof of concept, reproduction steps, tests.
    Poc,
    /// Title, summary, description, impact, severity.
    Claim,
    /// Recommendation, mitigation, remediation, fix.
    Recommendation,
    /// Anything else.
    Other,
}

impl SectionKind {
    pub fn label(self) -> &'static str {
        match self {
            SectionKind::Poc => "poc",
            SectionKind::Claim => "claim",
            SectionKind::Recommendation => "recommendation",
            SectionKind::Other => "other",
        }
    }
}

/// Hand-written structural matchers a rule can dispatch to.
#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Structural {
    /// P19: the headline claims a loss that the proof section contradicts.
    P19HeadlineVsProof,
}

impl Structural {
    pub fn label(self) -> &'static str {
        match self {
            Structural::P19HeadlineVsProof => "p19_headline_vs_proof",
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    pub id: String,
    pub title: String,
    pub group: String,
    pub verdict: Verdict,
    pub weight: u32,
    #[serde(default)]
    pub any_of: Vec<String>,
    #[serde(default)]
    pub all_of: Vec<String>,
    #[serde(default)]
    pub none_of: Vec<String>,
    /// Strong forms. When one of these matches, the rule carries `escalate_to`
    /// (default: hard) even if its base verdict is softer.
    #[serde(default)]
    pub escalate_on: Vec<String>,
    #[serde(default)]
    pub escalate_to: Option<Verdict>,
    /// Weak forms. When the ONLY thing that matched is one of these, the rule is
    /// dropped entirely rather than firing hard. This is how a rule says "this
    /// signal alone is not enough".
    #[serde(default)]
    pub downgrade_on: Vec<String>,
    /// Dispatch a rule to a hand-written structural matcher, e.g.
    /// `p19_headline_vs_proof`. Structural rules ignore any_of/all_of.
    #[serde(default)]
    pub structural: Option<Structural>,
    /// `document` (default) applies none_of to the whole text; `sentence`
    /// applies it only to the sentence that produced the any_of match.
    #[serde(default)]
    pub none_of_scope: NoneOfScope,
    /// Section kinds to skip when matching: `poc`, `claim`,
    /// `recommendation`, `other`.
    #[serde(default)]
    pub ignore_sections: Vec<SectionKind>,
    pub fix: String,
    /// Public documentation for this rule, e.g. `docs/rules/P19.md`.
    pub doc: String,
    /// How many rows of the author's log this rule was derived from. 0 means
    /// "not measured", not "zero rows".
    #[serde(default)]
    pub origin_rows: u32,
    #[serde(default)]
    pub evidence: Evidence,
    /// ISO date the rule was last checked against a fixture set.
    #[serde(default)]
    pub last_validated: Option<String>,
    /// False-positive rate once a control set exists. None = unmeasured, and it
    /// stays unmeasured until a paid/accepted corpus is available.
    #[serde(default)]
    pub fp_rate: Option<f64>,
    /// Whether the engine's negation window applies to this rule. Rules that
    /// already handle negation through `none_of` set this to false, so the two
    /// mechanisms do not fight each other.
    #[serde(default = "default_true")]
    pub negation_aware: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Verdict {
    /// Hard fail: kills the submission.
    Hard,
    /// Soft fail: hold in notes, do not submit yet.
    Soft,
    /// Note only; does not change the verdict.
    Warn,
}

impl Verdict {
    pub fn label(self) -> &'static str {
        match self {
            Verdict::Hard => "KILL",
            Verdict::Soft => "HOLD",
            Verdict::Warn => "NOTE",
        }
    }
}

/// The report-level outcome.
///
/// This is NOT the same type as a rule's `Verdict`. A rule's verdict says how
/// bad that one rule's finding is; the outcome is the decision rule applied
/// across all of them, plus the gate states.
///
/// There is no `Submit`. The tool cannot tell you to submit: it has no positive
/// evidence, only the absence of a blocker it knows how to look for.
#[derive(Debug, Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum Outcome {
    /// At least one hard rule fired.
    Kill,
    /// At least one soft rule fired, a gate failed, or the score is under the
    /// threshold.
    Hold,
    /// Only warnings fired.
    Note,
    /// Nothing fired and no gate failed, but at least one gate could not run.
    Incomplete,
    /// Nothing fired, no gate failed, and every gate ran.
    Clear,
}

impl Outcome {
    pub fn word(self) -> &'static str {
        match self {
            Outcome::Kill => "KILL",
            Outcome::Hold => "HOLD",
            Outcome::Note => "NOTE",
            Outcome::Incomplete => "INCOMPLETE",
            Outcome::Clear => "CLEAR",
        }
    }

    /// One line explaining what the word does and does not mean.
    pub fn meaning(self) -> &'static str {
        match self {
            Outcome::Kill => "A hard blocker fired. Do not submit as written.",
            Outcome::Hold => {
                "A soft blocker fired, a gate failed, or the score is below threshold. Hold in notes."
            }
            Outcome::Note => "Only notes fired. This is not a clearance - read them.",
            Outcome::Incomplete => {
                "No known blocker found, but at least one gate could not run. Unknown is not pass."
            }
            Outcome::Clear => "No known blocker found by this tool. That is all it means.",
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GateFile {
    pub version: u32,
    pub gates: Vec<Gate>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Gate {
    pub id: String,
    pub title: String,
    pub question: String,
    pub needs: String,
    pub blocks: String,
    pub weight: u32,
    #[serde(default)]
    pub checks: Vec<GateCheck>,
    /// Whether this gate has rows in the author's log behind it.
    #[serde(default)]
    pub evidence: Evidence,
    /// How many log rows support this gate. 0 = not measured.
    #[serde(default)]
    pub origin_rows: u32,
    /// The flag that supplies this gate's input when it is a corpus rather than
    /// a target-side input, e.g. `--known-issues` for D1. Informational only.
    #[serde(default)]
    pub implemented_by: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct GateCheck {
    pub name: String,
    pub test: String,
    pub fail: String,
}

// ---------------------------------------------------------------- ledger

/// The researcher's own private transcription of a program's terms.
/// Never bundled with the tool; always supplied at runtime.
///
/// Deliberately NOT `deny_unknown_fields`: the ledger schema is owned by
/// bounty-economics, whose contract is that new fields are additive and an
/// older reader ignores them.
#[derive(Debug, Deserialize)]
pub struct LedgerEntry {
    pub id: String,
    pub tier_shape: Option<String>,
    #[serde(default)]
    pub floor_severity: Option<String>,
    /// `documented` | `unstated` | `conflicting`. An unstated floor is not a
    /// Low floor; it is a page that has not been read.
    #[serde(default)]
    pub floor_status: Option<String>,
    #[serde(default)]
    pub source_url: Option<String>,
    #[serde(default)]
    pub retrieved: Option<String>,
    #[serde(default)]
    pub conflicts: Vec<String>,
    #[serde(default)]
    pub exclusions: Vec<Exclusion>,
    #[serde(default)]
    pub prior_audits: Vec<PriorAudit>,
    #[serde(default)]
    pub post_audit_delta: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Exclusion {
    pub class: String,
    #[serde(default)]
    pub clause: Option<String>,
    #[serde(default)]
    pub pattern: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PriorAudit {
    #[serde(default)]
    pub firm: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub covers: Vec<String>,
    #[serde(default)]
    pub known_issues: Vec<String>,
}

// ---------------------------------------------------------------- known issues

/// A known-issue corpus supplied by the user at runtime.
///
/// This addresses the largest rejection category: duplicate and prior-audit
/// matches. It is a KNOWN-ISSUE MATCHER, not a duplicate catcher - it can only
/// see issues the user already transcribed, so it cannot catch a concurrent
/// duplicate, which by definition is not in any corpus yet.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KnownIssue {
    pub id: String,
    pub summary: String,
    /// Case-insensitive regexes matched against the finding text.
    #[serde(default)]
    pub patterns: Vec<String>,
    #[serde(default)]
    pub source: Option<String>,
}

// ---------------------------------------------------------------- own history

/// One entry in the researcher's own submission log.
///
/// This is the only input that can catch the author's own repeats. In the source
/// log behind this tool, 12 of 113 rows repeat a family already present, so a
/// local history beats any regex over a finding's prose.
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct LogEntry {
    /// The bug family label, as the researcher writes it. Free text by design:
    /// this file is local, gitignored, and never leaves the machine.
    pub family: String,
    pub outcome: String,
    #[serde(default)]
    pub note: Option<String>,
    #[serde(default)]
    pub date: Option<String>,
}

// ---------------------------------------------------------------- results

#[derive(Debug, Serialize)]
pub struct Finding {
    pub rule: String,
    pub title: String,
    pub group: String,
    pub verdict: Verdict,
    pub weight: u32,
    /// The matched snippets. `matched_sentences[i]` is the sentence that
    /// `evidence[i]` came from.
    pub evidence: Vec<String>,
    /// The sentence each snippet was found in, so the user can argue with it.
    pub matched_sentences: Vec<String>,
    pub fix: String,
    pub doc: String,
    pub evidence_kind: Evidence,
    pub origin_rows: u32,
}

#[derive(Debug, Serialize)]
pub struct GateOutcome {
    pub gate: String,
    pub title: String,
    pub question: String,
    pub needs: String,
    /// What a failure of this gate prevents.
    pub blocks: String,
    pub weight: u32,
    pub status: GateStatus,
    pub detail: String,
    /// The individual checks this gate ran, so the user can see what was tested.
    pub checks: Vec<String>,
    pub evidence: Evidence,
    pub origin_rows: u32,
}

#[derive(Debug, Serialize, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum GateStatus {
    Pass,
    Fail,
    Unknown,
}

impl GateStatus {
    pub fn is_pass(self) -> bool {
        matches!(self, GateStatus::Pass)
    }
}

#[derive(Debug, Serialize)]
pub struct Report {
    pub tool: String,
    pub version: String,
    pub subject: String,
    pub outcome: Outcome,
    /// 100 minus the hand-set weights of the fired rules.
    ///
    /// An UNCALIBRATED HEURISTIC. It is not a probability, and it is not
    /// "payout odds". Gates do not deduct from it.
    pub viability_score: u32,
    pub threshold: u32,
    pub findings: Vec<Finding>,
    pub passed: Vec<String>,
    pub gates: Vec<GateOutcome>,
    pub disclaimer: String,
    /// Non-fatal problems with the supplied inputs.
    pub warnings: Vec<String>,
}
