//! The compiled vocabularies: 5 payout tier shapes, 14 exclusion clause
//! classes, and the gate cross-reference.
//!
//! The YAML is moved verbatim from bounty-economics v0.6.1 (MIT) and compiled
//! into the binary with `include_str!%%, so the vocabulary cannot drift from the
//! code and the tool needs no data directory at runtime. The one edited file is
//! `gate-crossref.yaml`: its rows named viability-gate's P1-P22 engine, which
//! priorart deletes, so those rows are gone. See docs/vocab.md.

use crate::model::GateSpec;
use serde::Deserialize;
use std::sync::OnceLock;

pub const TIER_SHAPES_YAML: &str = include_str!("../../vocab/tier-shapes.yaml");
pub const CLAUSE_CLASSES_YAML: &str = include_str!("../../vocab/clause-classes.yaml");
pub const GATE_CROSSREF_YAML: &str = include_str!("../../vocab/gate-crossref.yaml");

/// The number of tier shapes the taxonomy must contain.
pub const TIER_SHAPES: usize = 5;
/// The number of exclusion clause classes the taxonomy must contain.
pub const CLAUSE_CLASSES: usize = 14;

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
pub struct TierShapes {
    pub version: u32,
    pub shapes: Vec<TierShape>,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
pub struct TierShape {
    pub id: String,
    #[serde(default)]
    pub pays: Vec<String>,
    #[serde(default)]
    pub floor: Option<String>,
    #[serde(default)]
    pub implication: Option<String>,
    #[serde(default)]
    pub risk: Option<String>,
    #[serde(default)]
    pub detection: Vec<String>,
    #[serde(default)]
    pub verify: Option<String>,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
pub struct ClauseClasses {
    pub version: u32,
    pub classes: Vec<ClauseClass>,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
pub struct ClauseClass {
    pub id: String,
    #[serde(default)]
    pub kills: Option<String>,
    #[serde(default)]
    pub typical_wording: Vec<String>,
    #[serde(default)]
    pub escape: Option<String>,
    #[serde(default)]
    pub suggested_pattern: Option<String>,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
pub struct GateCrossref {
    pub version: u32,
    #[serde(default)]
    pub answered_here: std::collections::BTreeMap<String, CrossrefRow>,
    #[serde(default)]
    pub not_program_terms: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
pub struct CrossrefRow {
    pub catalog: String,
    #[serde(default)]
    pub read: Option<String>,
}

fn parsed<T: for<'de> Deserialize<'de>>(yaml: &str, what: &str) -> T {
    serde_saphyr::from_str(yaml)
        .unwrap_or_else(|e| panic!("vendored vocabulary ({what}) did not parse: {e}"))
}

pub fn tier_shapes_doc() -> &'static TierShapes {
    static DOC: OnceLock<TierShapes> = OnceLock::new();
    DOC.get_or_init(|| parsed(TIER_SHAPES_YAML, "tier-shapes.yaml"))
}

pub fn clause_classes_doc() -> &'static ClauseClasses {
    static DOC: OnceLock<ClauseClasses> = OnceLock::new();
    DOC.get_or_init(|| parsed(CLAUSE_CLASSES_YAML, "clause-classes.yaml"))
}

pub fn gate_crossref() -> &'static GateCrossref {
    static DOC: OnceLock<GateCrossref> = OnceLock::new();
    DOC.get_or_init(|| parsed(GATE_CROSSREF_YAML, "gate-crossref.yaml"))
}

pub fn tier_shapes() -> &'static [TierShape] {
    &tier_shapes_doc().shapes
}

pub fn clause_classes() -> &'static [ClauseClass] {
    &clause_classes_doc().classes
}

pub fn tier_shape(id: &str) -> Option<&'static TierShape> {
    tier_shapes().iter().find(|s| s.id == id)
}

pub fn clause_class(id: &str) -> Option<&'static ClauseClass> {
    clause_classes().iter().find(|c| c.id == id)
}

/// The floor a tier shape implies, as a severity. `unknown` implies none.
pub fn floor_of_shape(shape: &str) -> Option<&'static str> {
    tier_shape(shape).and_then(|s| s.floor.as_deref())
}

/// Normalise a clause for comparison: lowercase, hyphens to spaces, collapsed
/// whitespace.
fn norm(s: &str) -> String {
    s.to_lowercase()
        .replace(['-', '\u{2013}', '/'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Keywords per class, used when the taxonomy's own wording does not appear
/// verbatim. Normalised the same way as the clause.
const CLASS_KEYWORDS: [(&str, &[&str]); 14] = [
    ("privileged_role", &["privileged", "centralization"]),
    (
        "human_error",
        &[
            "human error",
            "human mistake",
            "rogue admin",
            "administrator error",
        ],
    ),
    (
        "admin_input",
        &[
            "admin input",
            "admin parameter",
            "misconfiguration",
            "protocol team",
        ],
    ),
    (
        "dos_no_extraction",
        &[
            "denial of service",
            "dos",
            "freeze",
            "liveness",
            "grief",
            "lock functionality",
        ],
    ),
    (
        "front_run_only",
        &[
            "front running",
            "frontrun",
            "mempool",
            "sandwich",
            "ordering race",
        ],
    ),
    (
        "best_practice",
        &[
            "best practice",
            "gas",
            "code style",
            "convention",
            "optimisation",
            "optimization",
        ],
    ),
    (
        "design_issue",
        &[
            "design issue",
            "design choice",
            "trade off",
            "by design",
            "intended behaviour",
            "intentional",
        ],
    ),
    (
        "off_chain",
        &[
            "off chain",
            "relayer",
            "backend",
            "indexer",
            "node logic",
            "infrastructure",
        ],
    ),
    (
        "third_party",
        &[
            "third party",
            "external dependency",
            "external protocol",
            "oracle",
            "bridge",
        ],
    ),
    (
        "user_input",
        &["user input", "self inflicted", "malformed input"],
    ),
    (
        "known_issue",
        &[
            "known issue",
            "known issues",
            "previously reported",
            "already known",
            "acknowledged",
        ],
    ),
    (
        "testing_limits",
        &[
            "mainnet",
            "public testnet",
            "live network",
            "denial of service testing",
        ],
    ),
    (
        "non_standard_token",
        &["fee on transfer", "rebasing", "non standard token"],
    ),
    (
        "scope_boundary",
        &[
            "in scope",
            "out of scope",
            "scope is limited",
            "limited to the listed",
            "files in scope",
        ],
    ),
];

/// Classify a verbatim out-of-scope clause into a clause class.
///
/// Order: the taxonomy's own typical wording, then its suggested_pattern regex,
/// then a documented keyword table. Unclassified stays None, and the clause
/// text is always shown beside the classification. A match is a prompt to read
/// the clause, never a verdict.
pub fn classify_clause(clause: &str) -> Option<&'static str> {
    let clause_norm = norm(clause);
    if clause_norm.is_empty() {
        return None;
    }
    for class in clause_classes() {
        for wording in &class.typical_wording {
            let w = norm(wording);
            if w.chars().count() >= 12 && clause_norm.contains(&w) {
                return Some(&class.id);
            }
        }
    }
    for class in clause_classes() {
        let Some(pattern) = class.suggested_pattern.as_deref() else {
            continue;
        };
        let Ok(re) = regex::Regex::new(&format!("(?i){pattern}")) else {
            continue;
        };
        if re.is_match(clause) {
            return Some(&class.id);
        }
    }
    for (class, keywords) in CLASS_KEYWORDS {
        if keywords.iter().any(|k| clause_norm.contains(&norm(k))) {
            return Some(class);
        }
    }
    None
}

/// The tier-shape ids, in taxonomy order (narrowest floor first).
pub fn tier_shape_ids() -> Vec<&'static str> {
    tier_shapes().iter().map(|s| s.id.as_str()).collect()
}

/// The clause-class ids, in taxonomy order.
pub fn clause_class_ids() -> Vec<&'static str> {
    clause_classes().iter().map(|c| c.id.as_str()).collect()
}

/// Is this a gate id priorart implements?
pub fn is_gate(id: &str) -> bool {
    crate::model::GATES.iter().any(|g| g.id == id)
}

/// Find a gate spec by id.
pub fn gate(id: &str) -> Option<&'static GateSpec> {
    crate::model::GATES.iter().find(|g| g.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn five_tier_shapes_and_fourteen_clause_classes() {
        assert_eq!(tier_shapes().len(), TIER_SHAPES, "tier shapes");
        assert_eq!(clause_classes().len(), CLAUSE_CLASSES, "clause classes");
        assert_eq!(tier_shapes_doc().version, 1, "tier-shapes.yaml version");
        assert_eq!(
            clause_classes_doc().version,
            1,
            "clause-classes.yaml version"
        );
    }

    #[test]
    fn there_is_no_any_shape_and_no_informational_floor() {
        assert!(
            tier_shape("any").is_none(),
            "the `any` shape must not come back"
        );
        for shape in tier_shapes() {
            assert_ne!(
                shape.floor.as_deref(),
                Some("informational"),
                "{}",
                shape.id
            );
        }
    }

    #[test]
    fn unknown_is_the_only_empty_paying_shape() {
        for shape in tier_shapes() {
            if shape.id == "unknown" {
                assert!(shape.pays.is_empty(), "unknown must pay nothing");
            } else {
                assert!(
                    !shape.pays.is_empty(),
                    "{} must declare what it pays",
                    shape.id
                );
            }
        }
    }

    #[test]
    fn every_clause_class_says_what_it_kills() {
        for class in clause_classes() {
            assert!(class.kills.is_some(), "{} has no kills text", class.id);
            assert!(
                class.escape.is_some() || class.id == "known_issue",
                "{} has no escape text",
                class.id
            );
        }
    }

    #[test]
    fn every_suggested_pattern_compiles() {
        for class in clause_classes() {
            if let Some(p) = &class.suggested_pattern {
                assert!(
                    regex::Regex::new(&format!("(?i){p}")).is_ok(),
                    "{} pattern does not compile: {p}",
                    class.id
                );
            }
        }
    }

    #[test]
    fn classification_uses_the_taxonomy() {
        assert_eq!(
            classify_clause("Attacks requiring access to privileged addresses"),
            Some("privileged_role")
        );
        assert_eq!(
            classify_clause("Issues related to fee-on-transfer tokens"),
            Some("non_standard_token")
        );
        assert_eq!(classify_clause("Something nobody wrote a class for"), None);
        assert_eq!(
            classify_clause("Vulnerabilities exploitable through front-running only"),
            Some("front_run_only")
        );
        assert_eq!(
            classify_clause("Issues already listed in the known-issues section"),
            Some("known_issue")
        );
        assert_eq!(
            classify_clause("Best-practice critiques and gas optimisations"),
            Some("best_practice")
        );
    }

    #[test]
    fn crossref_names_real_catalogs() {
        let doc = gate_crossref();
        assert_eq!(doc.version, 1);
        let catalogs = ["tier-shapes", "clause-classes", "prior-audit-surface"];
        for (gate, row) in &doc.answered_here {
            assert!(
                catalogs.contains(&row.catalog.as_str()),
                "{gate} names unknown catalog {}",
                row.catalog
            );
        }
        assert!(doc.answered_here.contains_key("G1_severity_floor"));
        assert!(doc.answered_here.contains_key("G2_exclusion_clause"));
        assert!(doc.not_program_terms.contains_key("G3_scope_tree"));
    }
}
