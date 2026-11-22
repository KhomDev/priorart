//! Load and validate a dossier file.

use crate::dossier::import;
use crate::model::{Dossier, DOSSIER_SCHEMA};
use std::fs;
use std::path::{Path, PathBuf};

/// The build refuses to write a serialized dossier over this limit
/// (`dossier::build::write_dossier`), so a dossier this build writes can be
/// read. A pre-0.2.0 dossier over the limit (0.1.3 had no cap) is still
/// refused here.
pub const MAX_DOSSIER_BYTES: u64 = 64 * 1024 * 1024;

/// Read a dossier. Rejects a file whose schema id is not DOSSIER_SCHEMA, so a
/// stale or hand-written file cannot be read as if it were current.
///
/// The path must be a regular file, and the file must fit `MAX_DOSSIER_BYTES`;
/// both are checked before the bytes are read.
pub fn load(path: &Path) -> Result<Dossier, String> {
    let meta = import::require_regular_file_from(path, "dossier").map_err(|e| {
        format!(
            "{}. Build one with priorart build <program-dir>.",
            e.trim_end_matches('.')
        )
    })?;
    check_dossier_size(path, meta.len())?;
    let bytes = fs::read(path).map_err(|e| {
        format!(
            "cannot read dossier {}: {e}. Build one with priorart build <program-dir>.",
            path.display()
        )
    })?;
    check_dossier_size(path, bytes.len() as u64)?;
    let text = String::from_utf8(bytes).map_err(|e| {
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
    let dossier: Dossier = serde_json::from_value(value)
        .map_err(|e| format!("{}: not a valid {DOSSIER_SCHEMA} file: {e}", path.display()))?;
    // The page number is part of the evidence. A dossier whose item cites a
    // page the source does not have has been edited or built by another
    // version; reading it silently would turn a citation into a guess.
    for item in &dossier.items {
        let source = dossier
            .sources
            .iter()
            .find(|s| s.id == item.source_id)
            .ok_or_else(|| {
                format!(
                    "{}: item {} names source {}, which the dossier does not contain. Rebuild it with this priorart version.",
                    path.display(),
                    item.id,
                    item.source_id
                )
            })?;
        if item.page == 0 || item.page > source.pages {
            return Err(format!(
                "{}: item {} cites page {} but source {} has {} page(s). The dossier may have been edited; rebuild it with priorart build.",
                path.display(),
                item.id,
                item.page,
                source.id,
                source.pages
            ));
        }
    }
    Ok(dossier)
}

fn check_dossier_size(path: &Path, len: u64) -> Result<(), String> {
    if len > MAX_DOSSIER_BYTES {
        return Err(format!(
            "dossier {} is {}; priorart reads a dossier up to {} MiB. Rebuild it from a smaller program folder, or split the folder.",
            path.display(),
            import::mib(len),
            MAX_DOSSIER_BYTES / (1024 * 1024)
        ));
    }
    Ok(())
}

/// Find the dossier to use when the user passed no --dossier: exactly one
/// .dossier file beside the finding. More than one is an error that names the
/// candidates, because guessing would be worse.
pub fn discover_beside(finding: &Path) -> Result<Option<PathBuf>, String> {
    // `Path::new("finding.md").parent()` is Some(""), and read_dir("") fails;
    // a bare file name means the current directory.
    let dir = finding
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut candidates = Vec::new();
    let mut skipped = Vec::new();
    let entries = fs::read_dir(dir).map_err(|e| {
        format!(
            "cannot read {} to look for a dossier: {e}. Pass --dossier <file>.",
            dir.display()
        )
    })?;
    for entry in entries.flatten() {
        let path = entry.path();
        let is_dossier = path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("dossier"));
        if !is_dossier {
            continue;
        }
        match fs::symlink_metadata(&path) {
            Ok(meta) if meta.file_type().is_file() => candidates.push(path),
            Ok(_) => skipped.push(path),
            Err(_) => {}
        }
    }
    candidates.sort();
    match candidates.len() {
        0 if !skipped.is_empty() => Err(format!(
            "{} beside {} is not a regular file; priorart reads regular files only. Pass --dossier with a regular file.",
            skipped[0].display(),
            finding.display()
        )),
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
