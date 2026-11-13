//! Load and validate a dossier file.

use crate::model::{Dossier, DOSSIER_SCHEMA};
use std::fs;
use std::path::{Path, PathBuf};

/// Read a dossier. Rejects a file whose schema id is not DOSSIER_SCHEMA, so a
/// stale or hand-written file cannot be read as if it were current.
pub fn load(path: &Path) -> Result<Dossier, String> {
    let text = fs::read_to_string(path).map_err(|e| {
        format!(
            "cannot read dossier {}: {e}. Build one with priorart build <program-dir>.",
            path.display()
        )
    })?;
    // Read the schema id first, so a foreign or stale file gets a migration
    // message rather than a field-missing error from the current struct.
    let value: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| format!("{}: not a dossier JSON file: {e}", path.display()))?;
    let schema = value.get("schema").and_then(|s| s.as_str());
    if schema != Some(DOSSIER_SCHEMA) {
        return Err(format!(
            "{}: schema {schema:?} is not {DOSSIER_SCHEMA}. Rebuild it with this priorart version.",
            path.display()
        ));
    }
    serde_json::from_value(value)
        .map_err(|e| format!("{}: not a valid {DOSSIER_SCHEMA} file: {e}", path.display()))
}

/// Find the dossier to use when the user passed no --dossier: exactly one
/// .dossier file beside the finding. More than one is an error that names the
/// candidates, because guessing would be worse.
pub fn discover_beside(finding: &Path) -> Result<Option<PathBuf>, String> {
    let dir = finding.parent().unwrap_or_else(|| Path::new("."));
    let mut candidates = Vec::new();
    let entries = fs::read_dir(dir).map_err(|e| {
        format!(
            "cannot read {} to look for a dossier: {e}. Pass --dossier <file>.",
            dir.display()
        )
    })?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("dossier") && path.is_file() {
            candidates.push(path);
        }
    }
    candidates.sort();
    match candidates.len() {
        0 => Ok(None),
        1 => Ok(candidates.pop()),
        _ => Err(format!(
            "{} dossier(s) beside {}: {}. Pass --dossier <file> to choose one.",
            candidates.len(),
            finding.display(),
            candidates
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}
