//! PA-B4 learned ranker: fixed, versioned weights read from a data table.
//!
//! The table (`eval/dupbench/weights/v1.json`, schema
//! [`RANKER_WEIGHTS_SCHEMA`]) records the feature names, the per-feature
//! transform, the standardisation statistics fitted on the Dup-Bench train
//! split, and the logistic-regression coefficients. Inference is one dot
//! product per candidate: deterministic, offline, no model runtime, no LLM,
//! no network. The training script and its seed are in
//! `eval/dupbench/train_ranker.py`.
//!
//! Feature order in the file is the order the coefficients were fitted in;
//! every entry names its raw feature, so reordering the table cannot silently
//! misalign it. A missing (null) value is never silently zero: the table must
//! state a `missing_value`, and the call-graph distance additionally carries
//! a missingness indicator feature (`call_graph_distance_missing`) so the
//! model can learn the "no path" effect. A table that omits a known feature,
//! names an unknown one, repeats one, or leaves a null-valued feature without
//! a `missing_value` is a usage error, not a panic.

use serde::{Deserialize, Serialize};
use std::path::Path;

use super::{Candidate, Features, MAX_CANDIDATE_DISTANCE, MAX_JSON_BYTES};

/// Schema id of the committed weights table.
pub const RANKER_WEIGHTS_SCHEMA: &str = "priorart.ranker-weights/v1";

/// The ordering a report uses when a ranker is supplied. The previous
/// candidate order is the tie-break, so two runs over the same inputs are
/// still byte-identical.
pub const RANKER_ORDERING: [&str; 2] = [
    "ranker score descending (priorart.ranker-weights/v1)",
    "previous candidate order as tie-break",
];

/// The documented missingness value for a null call-graph distance:
/// `MAX_CANDIDATE_DISTANCE + 1`, a value no real distance can take. The
/// weights table records it, and the trainer writes the same constant.
pub const CALL_GRAPH_MISSING: f64 = MAX_CANDIDATE_DISTANCE as f64 + 1.0;

/// Every feature a weights table must carry, exactly once.
pub const RANKER_FEATURE_NAMES: [&str; 13] = [
    "anchor_function_overlap.shared",
    "anchor_function_overlap.ratio",
    "anchor_state_overlap.shared",
    "anchor_state_overlap.ratio",
    "call_graph_distance",
    "call_graph_distance_missing",
    "mechanism_match.shared_root_causes",
    "mechanism_match.same_contract",
    "impact_match.shared_impacts",
    "identifier_score.specific_symbols",
    "identifier_score.shared_symbols",
    "identifier_score.rarity",
    "identifier_score.ranked",
];

/// One fitted feature row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RankerFeature {
    /// The raw feature this row reads, by the name `priorart candidates
    /// --json` uses (plus the derived `call_graph_distance_missing`).
    pub name: String,
    /// The feature's declared kind (`count`, `ratio`, `distance`, `binary`
    /// or `derived`); informational, validated only for the missingness rule.
    #[serde(default)]
    pub kind: Option<String>,
    /// `identity` or `log1p`.
    pub transform: String,
    /// The value substituted when the raw feature is null. Required for the
    /// features that can be null: both overlap ratios and the distance.
    #[serde(default)]
    pub missing_value: Option<f64>,
    /// Mean of the transformed feature over the training pairs.
    pub mean: f64,
    /// Standard deviation over the training pairs; always > 0.
    pub scale: f64,
    /// Logistic-regression coefficient on the standardised feature.
    pub coefficient: f64,
}

/// The whole weights table. Fields the table carries beyond what inference
/// needs (seed, contests, selection grid, calibration) are ignored here but
/// preserved in the committed file for reproduction.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RankerWeights {
    pub schema: String,
    #[serde(default)]
    pub version: u64,
    #[serde(default)]
    pub seed: u64,
    #[serde(default)]
    pub code_commit: Option<String>,
    pub features: Vec<RankerFeature>,
    pub intercept: f64,
}

impl RankerWeights {
    /// Validate the table against the documented rules. Every message names
    /// the offending feature and the rule, never a panic.
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != RANKER_WEIGHTS_SCHEMA {
            return Err(format!(
                "ranker weights schema is {:?}; expected {:?}",
                self.schema, RANKER_WEIGHTS_SCHEMA
            ));
        }
        if !self.intercept.is_finite() {
            return Err("ranker intercept is not a finite number".to_string());
        }
        let mut seen: Vec<&str> = Vec::new();
        for feature in &self.features {
            if !RANKER_FEATURE_NAMES.contains(&feature.name.as_str()) {
                return Err(format!(
                    "ranker weights name an unknown feature {:?}; known features are {}",
                    feature.name,
                    RANKER_FEATURE_NAMES.join(", ")
                ));
            }
            if seen.contains(&feature.name.as_str()) {
                return Err(format!(
                    "ranker weights list the feature {:?} more than once",
                    feature.name
                ));
            }
            seen.push(feature.name.as_str());
            if feature.transform != "identity" && feature.transform != "log1p" {
                return Err(format!(
                    "ranker feature {:?} has transform {:?}; expected identity or log1p",
                    feature.name, feature.transform
                ));
            }
            if !feature.mean.is_finite() || !feature.coefficient.is_finite() {
                return Err(format!(
                    "ranker feature {:?} has a non-finite mean or coefficient",
                    feature.name
                ));
            }
            if !feature.scale.is_finite() || feature.scale <= 0.0 {
                return Err(format!(
                    "ranker feature {:?} has scale {:?}; expected a finite number > 0",
                    feature.name, feature.scale
                ));
            }
            // The features whose raw value can be null must state the
            // documented substitution; a silent zero is exactly what the
            // missingness rule forbids.
            let nullable =
                feature.name.ends_with(".ratio") || feature.name == "call_graph_distance";
            if nullable {
                match feature.missing_value {
                    Some(value) if value.is_finite() => {}
                    _ => {
                        return Err(format!(
                            "ranker feature {:?} can be null but the table states no finite missing_value; record the documented rule instead of imputing silently",
                            feature.name
                        ))
                    }
                }
            }
        }
        for name in RANKER_FEATURE_NAMES {
            if !seen.contains(&name) {
                return Err(format!(
                    "ranker weights are missing the feature {name:?}; the trained table must carry every feature"
                ));
            }
        }
        Ok(())
    }
}

/// A loaded, validated weights table.
#[derive(Debug, Clone)]
pub struct Ranker {
    weights: RankerWeights,
}

impl Ranker {
    /// Validate and wrap a weights table. `load` is the file path; this
    /// constructor is the same check for an in-memory table (used by tests
    /// and available to embedders).
    pub(crate) fn from_weights(weights: RankerWeights) -> Result<Self, String> {
        weights.validate()?;
        Ok(Self { weights })
    }

    /// Read and validate a weights table. A missing file, malformed JSON or
    /// an invalid table is a usage error naming the file.
    pub fn load(path: &Path) -> Result<Self, String> {
        let meta = crate::dossier::import::require_regular_file_from(path, "ranker weights")?;
        if meta.len() > MAX_JSON_BYTES {
            return Err(format!(
                "ranker weights {} are {}; priorart reads up to {} MiB",
                path.display(),
                crate::dossier::import::mib(meta.len()),
                MAX_JSON_BYTES / (1024 * 1024)
            ));
        }
        let bytes = std::fs::read(path)
            .map_err(|e| format!("cannot read ranker weights {}: {e}", path.display()))?;
        let weights: RankerWeights = serde_json::from_slice(&bytes).map_err(|e| {
            format!(
                "{}: not a ranker weights table: {e}. Write one with eval/dupbench/train_ranker.py.",
                path.display()
            )
        })?;
        Self::from_weights(weights).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// The weights as loaded (validated).
    pub fn weights(&self) -> &RankerWeights {
        &self.weights
    }

    /// The logit score of one candidate. Infallible: `load` validated the
    /// table, and every raw feature is derived from the candidate struct.
    pub fn score(&self, candidate: &Candidate) -> f64 {
        let mut z = self.weights.intercept;
        for feature in &self.weights.features {
            let mut value = raw_value(feature, &candidate.features);
            if feature.transform == "log1p" {
                value = value.ln_1p();
            }
            z += feature.coefficient * (value - feature.mean) / feature.scale;
        }
        sigmoid(z)
    }
}

fn raw_value(feature: &RankerFeature, f: &Features) -> f64 {
    let missing = feature.missing_value.unwrap_or(0.0);
    match feature.name.as_str() {
        "anchor_function_overlap.shared" => f.anchor_function_overlap.shared.len() as f64,
        "anchor_function_overlap.ratio" => f.anchor_function_overlap.ratio.unwrap_or(missing),
        "anchor_state_overlap.shared" => f.anchor_state_overlap.shared.len() as f64,
        "anchor_state_overlap.ratio" => f.anchor_state_overlap.ratio.unwrap_or(missing),
        "call_graph_distance" => f
            .call_graph_distance
            .map(f64::from)
            .unwrap_or_else(|| feature.missing_value.unwrap_or(CALL_GRAPH_MISSING)),
        "call_graph_distance_missing" => indicator(f.call_graph_distance.is_none()),
        "mechanism_match.shared_root_causes" => f.mechanism_match.shared_root_causes.len() as f64,
        "mechanism_match.same_contract" => indicator(f.mechanism_match.same_contract),
        "impact_match.shared_impacts" => f.impact_match.shared_impacts.len() as f64,
        "identifier_score.specific_symbols" => f.identifier_score.specific_symbols as f64,
        "identifier_score.shared_symbols" => f.identifier_score.shared_symbols as f64,
        "identifier_score.rarity" => f.identifier_score.rarity as f64,
        "identifier_score.ranked" => indicator(f.identifier_score.ranked),
        // Unreachable after `RankerWeights::validate`; returning a value keeps
        // the function total instead of panicking on a future edit that skips
        // validation.
        _ => 0.0,
    }
}

/// A boolean feature as 0.0 / 1.0.
fn indicator(value: bool) -> f64 {
    if value {
        1.0
    } else {
        0.0
    }
}

fn sigmoid(z: f64) -> f64 {
    let z = z.clamp(-700.0, 700.0);
    1.0 / (1.0 + (-z).exp())
}
