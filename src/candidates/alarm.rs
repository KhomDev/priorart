//! PA-B5 calibrated alarm: "this finding shares a root cause with a prior item".
//!
//! The calibration table (`eval/dupbench/weights/alarm-v1.json`, schema
//! [`ALARM_CALIBRATION_SCHEMA`]) is an isotonic map from the PA-B4 ranker
//! score of a finding's **top** candidate to the probability, measured on the
//! Dup-Bench validation split, that the top candidate is the finding's judged
//! primary. It is written by `eval/dupbench/calibrate_alarm.py`, which records
//! the exact precision definition in the file.
//!
//! The warning fires only when the **calibrated** probability is at or above
//! the table's threshold. The threshold is the lowest isotonic block value
//! whose validation precision has a Wilson lower bound >= 0.80; a table whose
//! threshold is `null` ships the warning off, and the alarm then never fires.
//! Comparing the raw ranker score with the threshold is a defect, not a
//! shortcut: the two are on different scales.
//!
//! A table that is not monotone, has a threshold that is not one of its block
//! values, or holds a non-finite or out-of-range number is a usage error.

use serde::{Deserialize, Serialize};
use std::path::Path;

use super::{Candidate, MAX_JSON_BYTES};

/// Schema id of the committed alarm calibration table.
pub const ALARM_CALIBRATION_SCHEMA: &str = "priorart.alarm-calibration/v1";

/// The wording of the warning when it fires.
pub const ALARM_WARNING: &str = "shares a root cause";

/// One isotonic block: every raw top score in `[score_lo, score_hi]` was
/// observed with true-alarm rate `value`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AlarmBlock {
    pub score_lo: f64,
    pub score_hi: f64,
    pub n: u64,
    pub k: u64,
    pub value: f64,
}

/// The calibration table as committed. Fields beyond what inference needs
/// (definitions, the full threshold table, provenance) are kept in the file
/// for reproduction and ignored here.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AlarmCalibration {
    pub schema: String,
    #[serde(default)]
    pub version: u64,
    pub blocks: Vec<AlarmBlock>,
    /// The calibrated threshold, or `null` when the warning ships off.
    pub threshold: Option<f64>,
    #[serde(default)]
    pub validation_contests: Vec<String>,
}

impl AlarmCalibration {
    /// Validate the documented rules; every message names the rule.
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != ALARM_CALIBRATION_SCHEMA {
            return Err(format!(
                "alarm calibration schema is {:?}; expected {:?}",
                self.schema, ALARM_CALIBRATION_SCHEMA
            ));
        }
        if self.blocks.is_empty() {
            return Err("alarm calibration has no blocks".to_string());
        }
        for (i, block) in self.blocks.iter().enumerate() {
            let finite =
                block.score_lo.is_finite() && block.score_hi.is_finite() && block.value.is_finite();
            if !finite || !(0.0..=1.0).contains(&block.value) {
                return Err(format!(
                    "alarm calibration block {} holds a non-finite or out-of-range number",
                    i + 1
                ));
            }
            if block.score_lo > block.score_hi {
                return Err(format!(
                    "alarm calibration block {} has score_lo above score_hi",
                    i + 1
                ));
            }
            if block.n == 0 || block.k > block.n {
                return Err(format!(
                    "alarm calibration block {} has n {} and k {}; expected 0 <= k <= n, n > 0",
                    i + 1,
                    block.n,
                    block.k
                ));
            }
        }
        for (i, pair) in self.blocks.windows(2).enumerate() {
            if pair[1].score_lo <= pair[0].score_hi {
                return Err(format!(
                    "alarm calibration blocks {} and {} overlap or are out of order",
                    i + 1,
                    i + 2
                ));
            }
            if pair[1].value < pair[0].value {
                return Err(format!(
                    "alarm calibration is not monotone: block {} value {} is below block {} value {}",
                    i + 2,
                    pair[1].value,
                    i + 1,
                    pair[0].value
                ));
            }
        }
        if let Some(threshold) = self.threshold {
            if !self.blocks.iter().any(|b| b.value == threshold) {
                return Err(format!(
                    "alarm threshold {threshold} is not one of the calibration block values"
                ));
            }
        }
        Ok(())
    }

    /// The calibrated probability of a raw ranker score: the value of the last
    /// block whose `score_lo` is at or below the score; below the first block,
    /// the first block's value.
    pub fn calibrate(&self, raw: f64) -> f64 {
        let mut value = self.blocks[0].value;
        for block in &self.blocks {
            if block.score_lo <= raw {
                value = block.value;
            } else {
                break;
            }
        }
        value
    }

    /// Whether the warning fires for a calibrated probability.
    pub fn fires(&self, calibrated: f64) -> bool {
        match self.threshold {
            Some(threshold) => calibrated >= threshold,
            None => false,
        }
    }
}

/// A loaded, validated calibration table.
#[derive(Debug, Clone)]
pub struct Alarm {
    calibration: AlarmCalibration,
}

/// The size cap for a calibration table: up to and including
/// [`MAX_JSON_BYTES`] is read; one byte more is a usage error.
pub(crate) fn check_size(len: u64, path: &Path) -> Result<(), String> {
    if len > MAX_JSON_BYTES {
        return Err(format!(
            "alarm calibration {} is {}; priorart reads up to {} MiB",
            path.display(),
            crate::dossier::import::mib(len),
            MAX_JSON_BYTES / (1024 * 1024)
        ));
    }
    Ok(())
}

impl Alarm {
    /// Validate and wrap an in-memory table.
    pub(crate) fn from_calibration(calibration: AlarmCalibration) -> Result<Self, String> {
        calibration.validate()?;
        Ok(Self { calibration })
    }

    /// Read and validate a calibration table. A missing file, malformed JSON
    /// or an invalid table is a usage error naming the file.
    pub fn load(path: &Path) -> Result<Self, String> {
        let meta = crate::dossier::import::require_regular_file_from(path, "alarm calibration")?;
        check_size(meta.len(), path)?;
        let bytes = std::fs::read(path)
            .map_err(|e| format!("cannot read alarm calibration {}: {e}", path.display()))?;
        let calibration: AlarmCalibration = serde_json::from_slice(&bytes).map_err(|e| {
            format!(
                "{}: not an alarm calibration table: {e}. Write one with eval/dupbench/calibrate_alarm.py.",
                path.display()
            )
        })?;
        Self::from_calibration(calibration).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// The table as loaded.
    pub fn calibration(&self) -> &AlarmCalibration {
        &self.calibration
    }

    /// The alarm for a ranker-ordered candidate list (top candidate first).
    pub fn evaluate(&self, candidates: &[Candidate]) -> AlarmReport {
        let threshold = self.calibration.threshold;
        let status = if threshold.is_some() { "on" } else { "off" };
        let top = candidates
            .first()
            .and_then(|c| c.ranker_score.map(|s| (c, s)));
        match top {
            Some((candidate, raw)) => {
                let calibrated = self.calibration.calibrate(raw);
                let fires = self.calibration.fires(calibrated);
                AlarmReport {
                    schema: ALARM_CALIBRATION_SCHEMA.to_string(),
                    status: status.to_string(),
                    threshold,
                    top_candidate: Some(candidate.prior_id.clone()),
                    top_score: Some(raw),
                    calibrated: Some(calibrated),
                    fires,
                    warning: fires.then(|| ALARM_WARNING.to_string()),
                }
            }
            None => AlarmReport {
                schema: ALARM_CALIBRATION_SCHEMA.to_string(),
                status: status.to_string(),
                threshold,
                top_candidate: None,
                top_score: None,
                calibrated: None,
                fires: false,
                warning: None,
            },
        }
    }
}

/// The alarm block of a candidates report; present only when `--alarm` was
/// supplied, so every other output is unchanged byte for byte.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AlarmReport {
    pub schema: String,
    /// `on` when the table ships a threshold, `off` when it does not.
    pub status: String,
    pub threshold: Option<f64>,
    pub top_candidate: Option<String>,
    /// The raw PA-B4 score of the top candidate.
    pub top_score: Option<f64>,
    /// The calibrated probability the threshold is compared with.
    pub calibrated: Option<f64>,
    pub fires: bool,
    /// [`ALARM_WARNING`] when the alarm fires.
    pub warning: Option<String>,
}
