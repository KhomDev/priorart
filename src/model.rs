//! The two JSON documents priorart emits, and the inputs the gates read.
//!
//! `Dossier` is `priorart.dossier/v1`: one deterministic file per program.
//! `CheckReport` is `priorart.check/v1`: the evidence for one finding.
//!
//! Both are serialised with every key present so a consumer never has to
//! distinguish "absent" from "null". Ordering is fixed by construction: sources
//! are sorted by path, items follow document order.

use serde::{Deserialize, Serialize};

/// Schema id for a dossier file.
pub const DOSSIER_SCHEMA: &str = "priorart.dossier/v1";

/// Schema id for `priorart check --json`.
pub const CHECK_SCHEMA: &str = "priorart.check/v1";

/// Schema id for `priorart rejections summary --json`.
pub const REJECTIONS_SCHEMA: &str = "priorart.rejections-summary/v1";

// ------------------------------------------------------------------ dossier

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Dossier {
    pub schema: String,
    pub tool: ToolInfo,
    pub program: Program,
    pub sources: Vec<Source>,
    pub items: Vec<Item>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolInfo {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Program {
    /// Stable slug used in the default output filename.
    pub id: String,
    pub name: Option<String>,
    pub platform: Option<String>,
    pub url: Option<String>,
    pub scope_url: Option<String>,
    /// The program's payout terms, transcribed from the page the user saved.
    /// Absent means the payout gate did not run - not that the program pays.
    pub payout: Option<Payout>,
    /// Prior audits named in program.yaml, for the prior-audit gate.
    pub prior_audits: Vec<PriorAudit>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Payout {
    /// One of the five tier shapes in vocab/tier-shapes.yaml.
    pub tier_shape: String,
    pub floor_status: Option<String>,
    pub floor_severity: Option<String>,
    /// The page the terms were read from. A payout claim without one is a
    /// guess, and the gate says so.
    pub source_url: Option<String>,
    pub retrieved: Option<String>,
    /// Heading the quote was read under, e.g. "Rewards".
    pub section: Option<String>,
    /// Verbatim line from the saved page.
    pub quote: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct PriorAudit {
    pub firm: Option<String>,
    pub url: Option<String>,
    pub covers: Vec<String>,
    pub known_issues: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Source {
    /// Stable id, `src-0001` in path order.
    pub id: String,
    /// `program-page` | `known-issues` | `audit`.
    pub kind: String,
    /// File name or document title.
    pub title: Option<String>,
    /// Path relative to the program directory, forward slashes.
    pub path: String,
    /// SHA-256 of the file's bytes, lowercase hex.
    pub sha256: String,
    /// Page count: 1 + the number of form feeds (pdftotext's page break).
    pub pages: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Item {
    /// Stable id, `item-0001` in document order.
    pub id: String,
    pub source_id: String,
    /// `audit-finding` | `audit-section` | `known-issue` | `scope-clause`
    /// | `scope-path` | `payout-tier` | `page-section`.
    pub kind: String,
    pub title: Option<String>,
    pub section: Option<String>,
    /// 1-based page the item starts on.
    pub page: usize,
    /// Verbatim excerpt of the source, whitespace collapsed.
    pub quote: String,
    /// Strong code identifiers found in the item text.
    pub identifiers: Vec<String>,
    /// Lexical mechanism estimate. Never used for ranking.
    pub mechanism_class: Option<String>,
    /// For scope-clause items: the clause class from the taxonomy, or null
    /// when the clause text matches no suggested pattern.
    pub clause_class: Option<String>,
    /// The item's text as read from the source.
    pub text: String,
}

// -------------------------------------------------------------------- check

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CheckReport {
    pub schema: String,
    pub tool_version: String,
    pub finding: FindingRef,
    pub dossier: DossierRef,
    pub prior_art: Vec<PriorArt>,
    pub exclusions: Vec<ExclusionEvidence>,
    pub payout: PayoutEvidence,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FindingRef {
    pub path: String,
    pub sha256: String,
    pub identifiers: Vec<String>,
    pub claimed_severity: Option<String>,
    /// Lexical mechanism estimate for the finding, so a prior item's mechanism
    /// can be compared side by side.
    pub mechanism_class: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DossierRef {
    pub path: String,
    pub schema: String,
    pub program_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PriorArt {
    pub item_id: String,
    pub source_id: String,
    pub source_kind: String,
    pub source_title: Option<String>,
    pub path: String,
    pub page: usize,
    pub section: Option<String>,
    pub quote: String,
    pub shared_identifiers: Vec<String>,
    pub mechanism_class: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExclusionEvidence {
    /// Clause class id from vocab/clause-classes.yaml, or null when the clause
    /// could not be classified.
    pub clause_class: Option<String>,
    /// The precondition in the finding that triggered the match.
    pub precondition: String,
    /// The program's clause, verbatim.
    pub clause: String,
    /// The item the clause came from.
    pub item_id: String,
    pub source_id: String,
    pub page: usize,
    pub section: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PayoutEvidence {
    pub tier_shape: Option<String>,
    pub floor: Option<String>,
    pub claimed_severity: Option<String>,
    /// `true` payable, `false` below the floor, `null` not checkable.
    pub payable: Option<bool>,
    pub citation: Option<String>,
    pub section: Option<String>,
    pub quote: Option<String>,
    /// Why the gate could not decide, when `payable` is null.
    pub detail: Option<String>,
}

// -------------------------------------------------------------------- scope

/// Gate ids, in the order `priorart rules` lists them.
pub const GATES: [GateSpec; 4] = [
    GateSpec {
        id: "G1",
        question: "Can it be paid?",
        checks: "the claimed severity against the program's payout tier shape and floor",
    },
    GateSpec {
        id: "G2",
        question: "Is it excluded?",
        checks: "the finding's preconditions against the program's exclusion clause classes",
    },
    GateSpec {
        id: "G3",
        question: "Is the root cause in scope?",
        checks: "the paths the finding cites against the in-scope file list",
    },
    GateSpec {
        id: "G4",
        question: "Has this been found?",
        checks: "prior audit coverage and ranked prior items sharing strong identifiers",
    },
];

#[derive(Debug, Clone, Copy)]
pub struct GateSpec {
    pub id: &'static str,
    pub question: &'static str,
    pub checks: &'static str,
}
