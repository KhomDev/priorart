//! The mechanism and impact taxonomy: 20 root-cause classes, 5 impact
//! classes and 5 asset classes, compiled from `vocab/taxonomy.yaml`.
//!
//! The extractor answers one question about a finding or prior-item text:
//! which root causes does it assert, which impacts does it claim, and which
//! assets does it name? It is a deterministic lexicon over the text - no model,
//! no network, no clock and no file access - and a match is a prompt to read
//! the finding, never a verdict.
//!
//! Matching is case-insensitive on word boundaries. A cue is skipped when the
//! clause that contains it negates it ("no reentrancy risk", "prevents
//! reentrancy"), and a cue that is part of a fix-side anti-cue ("reentrancy
//! guard") is skipped unless the anti-cue is itself described as missing or
//! absent. A second, unnegated clause still asserts the class. One match per
//! root-cause and impact class is reported (the earliest), and one match per
//! asset class and term; every span is a byte range into the input text.

mod extract;

pub use extract::{classify, Extractor, MAX_TEXT_BYTES};

use crate::render::text::sanitize;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

/// The vendored taxonomy, compiled into the binary.
pub const TAXONOMY_YAML: &str = include_str!("../../vocab/taxonomy.yaml");

/// Schema id for one text's mechanism set.
pub const TAXONOMY_SCHEMA: &str = "priorart.taxonomy/v1";
/// Schema id for `priorart taxonomy classify --json`.
pub const TAXONOMY_CLASSIFY_SCHEMA: &str = "priorart.taxonomy-classify/v1";
/// The taxonomy version this build understands.
pub const TAXONOMY_VERSION: u32 = 1;
/// The number of root-cause classes the file must contain.
pub const ROOT_CAUSES: usize = 20;
/// The number of impact classes the file must contain.
pub const IMPACTS: usize = 5;
/// The number of asset classes the file must contain.
pub const ASSETS: usize = 5;
/// The abstention when a text asserts no mechanism at all.
pub const ABSTAIN_NO_MECHANISM: &str = "no-mechanism";

/// The parsed taxonomy.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct Taxonomy {
    pub version: u32,
    pub root_causes: Vec<Class>,
    pub impacts: Vec<Class>,
    pub assets: Vec<Class>,
}

/// One class in any of the three lists.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct Class {
    pub id: String,
    pub name: String,
    pub definition: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cues: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub anti_cues: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub symbols: Vec<String>,
}

/// A byte range into the classified text; `end_byte` is exclusive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
pub struct Span {
    pub start_byte: usize,
    pub end_byte: usize,
}

/// One extracted class mention.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
pub struct Match {
    /// The class id from `vocab/taxonomy.yaml`.
    pub class: String,
    pub span: Span,
    /// The text that matched, verbatim (case preserved).
    pub term: String,
}

/// Everything the extractor found in one text.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct MechanismSet {
    pub schema: String,
    /// Root-cause classes, at most one match per class, ordered by span.
    pub root_causes: Vec<Match>,
    /// Impact classes, at most one match per class, ordered by span.
    pub impacts: Vec<Match>,
    /// Asset classes, one match per class and term, ordered by span.
    pub assets: Vec<Match>,
    /// `no-mechanism` when no class matched at all; `null` otherwise.
    pub abstain_reason: Option<String>,
}

/// One file's classification, as `priorart taxonomy classify --json` emits it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct FileClassification {
    /// The path as given, with forward slashes on every platform.
    pub file: String,
    pub text_sha256: String,
    pub text_bytes: usize,
    pub mechanism: MechanismSet,
}

/// The whole output of `priorart taxonomy classify --json`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct ClassifyReport {
    pub schema: String,
    pub taxonomy_version: u32,
    pub taxonomy_source: String,
    pub results: Vec<FileClassification>,
}

impl ClassifyReport {
    pub fn new(results: Vec<FileClassification>) -> Self {
        Self {
            schema: TAXONOMY_CLASSIFY_SCHEMA.to_string(),
            taxonomy_version: TAXONOMY_VERSION,
            taxonomy_source: "vocab/taxonomy.yaml".to_string(),
            results,
        }
    }
}

fn parsed(yaml: &str) -> Taxonomy {
    serde_saphyr::from_str(yaml)
        .unwrap_or_else(|e| panic!("vendored vocabulary (taxonomy.yaml) did not parse: {e}"))
}

/// The compiled taxonomy, parsed once.
pub fn taxonomy() -> &'static Taxonomy {
    static DOC: OnceLock<Taxonomy> = OnceLock::new();
    DOC.get_or_init(|| parsed(TAXONOMY_YAML))
}

/// The root-cause class ids, in taxonomy order.
pub fn root_cause_ids() -> Vec<&'static str> {
    taxonomy()
        .root_causes
        .iter()
        .map(|c| c.id.as_str())
        .collect()
}

/// The impact class ids, in taxonomy order.
pub fn impact_ids() -> Vec<&'static str> {
    taxonomy().impacts.iter().map(|c| c.id.as_str()).collect()
}

/// The asset class ids, in taxonomy order.
pub fn asset_ids() -> Vec<&'static str> {
    taxonomy().assets.iter().map(|c| c.id.as_str()).collect()
}

/// Find one class by id, in any of the three lists.
pub fn class(id: &str) -> Option<&'static Class> {
    taxonomy()
        .root_causes
        .iter()
        .chain(taxonomy().impacts.iter())
        .chain(taxonomy().assets.iter())
        .find(|c| c.id == id)
}

/// Render one classification as text, for `priorart taxonomy classify`.
pub fn render_text(report: &ClassifyReport) -> String {
    fn quoted(term: &str) -> String {
        format!("{:?}", sanitize(term))
    }
    let mut out = String::new();
    for (i, result) in report.results.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        out.push_str(&format!(
            "{}  bytes {}  sha256 {}\n",
            sanitize(&result.file),
            result.text_bytes,
            result.text_sha256
        ));
        let mechanism = &result.mechanism;
        for m in &mechanism.root_causes {
            out.push_str(&format!(
                "MECHANISM {:<26} {:<28} bytes {}-{}\n",
                sanitize(&m.class),
                quoted(&m.term),
                m.span.start_byte,
                m.span.end_byte
            ));
        }
        for m in &mechanism.impacts {
            out.push_str(&format!(
                "IMPACT    {:<26} {:<28} bytes {}-{}\n",
                sanitize(&m.class),
                quoted(&m.term),
                m.span.start_byte,
                m.span.end_byte
            ));
        }
        for m in &mechanism.assets {
            out.push_str(&format!(
                "ASSET     {:<26} {:<28} bytes {}-{}\n",
                sanitize(&m.class),
                quoted(&m.term),
                m.span.start_byte,
                m.span.end_byte
            ));
        }
        if let Some(reason) = &mechanism.abstain_reason {
            out.push_str(&format!("ABSTAIN   {}\n", sanitize(reason)));
        }
    }
    sanitize(&out)
}

#[cfg(test)]
mod tests;
