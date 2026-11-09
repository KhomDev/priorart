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
    /// P23: the report does not say who the attacker is.
    P23Attacker,
    /// P24: preconditions or the victim's action are missing or thin.
    P24Preconditions,
    /// P25: impact is missing or not quantified.
    P25Impact,
    /// P26: the program's own severity rule is not quoted.
    P26SeverityRule,
}

impl Structural {
    pub fn label(self) -> &'static str {
        match self {
            Structural::P19HeadlineVsProof => "p19_headline_vs_proof",
            Structural::P23Attacker => "p23_attacker",
            Structural::P24Preconditions => "p24_preconditions",
            Structural::P25Impact => "p25_impact",
            Structural::P26SeverityRule => "p26_severity_rule",
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

// ---------------------------------------------------------------- prior-finding index

/// One prior finding in a local index.
///
/// Built by `vg import prior` or loaded from a directory of markdown. The
/// quotes are short excerpts, not the whole document. An index of someone
/// else's reports is still their text: keep it local.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IndexedFinding {
    pub id: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub identifiers: Vec<String>,
    #[serde(default)]
    pub root_causes: Vec<String>,
    #[serde(default)]
    pub quotes: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IndexFile {
    pub version: u32,
    pub entries: Vec<IndexedFinding>,
}

/// Optional manifest beside a draft. Fields the author fills in so the claim
/// checks (P23-P26) can see what the prose did not say. Empty strings count
/// as missing. `impact_usd: 0` does not count as a quantified impact.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct FindingManifest {
    #[serde(default)]
    pub attacker: Option<String>,
    #[serde(default)]
    pub victim: Option<String>,
    #[serde(default)]
    pub preconditions: Option<ManifestPreconditions>,
    #[serde(default)]
    pub impact_usd: Option<f64>,
    #[serde(default)]
    pub severity_rule_quoted: Option<String>,
    #[serde(default)]
    pub prior_audit_check: Option<String>,
    #[serde(default)]
    pub duplicate_check: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
}

/// A precondition list, written as one string or as a list.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum ManifestPreconditions {
    One(String),
    Many(Vec<String>),
}

impl ManifestPreconditions {
    pub fn is_blank(&self) -> bool {
        match self {
            ManifestPreconditions::One(s) => s.trim().is_empty(),
            ManifestPreconditions::Many(v) => v.iter().all(|s| s.trim().is_empty()),
        }
    }
}

impl FindingManifest {
    pub fn filled(s: &Option<String>) -> bool {
        s.as_deref().is_some_and(|v| !v.trim().is_empty())
    }

    /// Which claim-check ids this manifest answers. It does not suppress P18
    /// or an index hit: a sentence that says "I checked" is not a check.
    pub fn suppresses(&self, rule: &str) -> bool {
        match rule {
            "P23" => Self::filled(&self.attacker),
            "P24" => {
                Self::filled(&self.victim)
                    && self.preconditions.as_ref().is_some_and(|p| !p.is_blank())
            }
            "P25" => self.impact_usd.is_some_and(|n| n > 0.0),
            "P26" => Self::filled(&self.severity_rule_quoted),
            _ => false,
        }
    }
}

/// Status values `finding.yaml` may record. Anything else is a typo, and a
/// typo must not be stored as if it were a disposition.
pub const MANIFEST_STATUSES: [&str; 9] = [
    "draft",
    "held",
    "submitted",
    "paid",
    "rejected",
    "duplicate",
    "informative",
    "oos",
    "wontfix",
];

// ---------------------------------------------------------------- engagement manifest

/// An engagement file (`audit.yaml`) of the kind a hunt keeps beside a target.
///
/// This is not the bounty-economics ledger. `vg import audit` transcribes the
/// fields below into a ledger, a scope list, and a known-issues file. Unknown
/// keys are ignored: real engagement files grow extra fields, and dropping the
/// file for a key this version does not read would make import useless.
#[derive(Debug, Deserialize)]
pub struct Engagement {
    #[serde(default)]
    pub version: Option<u32>,
    pub id: String,
    #[serde(default)]
    pub program: Option<String>,
    #[serde(default)]
    pub platform: Option<String>,
    /// Lowest severity the program pays: critical, high, medium, low, or none.
    #[serde(default)]
    pub severity_floor: Option<String>,
    #[serde(default)]
    pub floor_status: Option<String>,
    #[serde(default)]
    pub tier_shape: Option<String>,
    #[serde(default)]
    pub source_url: Option<String>,
    #[serde(default)]
    pub retrieved: Option<String>,
    #[serde(default)]
    pub exclusion_clauses: Vec<EngagementClause>,
    #[serde(default)]
    pub known_issues: Vec<EngagementIssue>,
    #[serde(default)]
    pub scope: Vec<String>,
    #[serde(default)]
    pub severity_rules: Vec<EngagementSeverityRule>,
    #[serde(default)]
    pub prior_audits: Vec<PriorAudit>,
}

#[derive(Debug, Deserialize)]
pub struct EngagementClause {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub class: Option<String>,
    /// The clause, verbatim. `clause` is accepted as an alias.
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub clause: Option<String>,
    /// Optional regex. Import does not invent one.
    #[serde(default)]
    pub pattern: Option<String>,
}

impl EngagementClause {
    pub fn body(&self) -> String {
        self.text
            .as_deref()
            .or(self.clause.as_deref())
            .unwrap_or("")
            .trim()
            .to_string()
    }
}

#[derive(Debug, Deserialize)]
pub struct EngagementIssue {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub patterns: Vec<String>,
    #[serde(default)]
    pub identifiers: Vec<String>,
}

impl EngagementIssue {
    pub fn body(&self) -> String {
        self.summary
            .as_deref()
            .or(self.text.as_deref())
            .unwrap_or("")
            .trim()
            .to_string()
    }
}

#[derive(Debug, Deserialize)]
pub struct EngagementSeverityRule {
    #[serde(default)]
    pub tier: Option<String>,
    #[serde(default)]
    pub text: Option<String>,
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
