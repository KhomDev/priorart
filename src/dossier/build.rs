//! Assemble and write one dossier.
//!
//! The output is deterministic: sources sorted by path, items in document
//! order, ids assigned from those orders, and no wall-clock field anywhere. Two
//! builds over the same folder are byte-identical, which a test asserts.

use crate::dossier::import::{self, Imported};
use crate::model::{Dossier, ToolInfo, DOSSIER_SCHEMA};
use std::fs;
use std::path::{Path, PathBuf};

/// One finished build.
#[derive(Debug)]
pub struct BuildOutcome {
    pub dossier: Dossier,
    pub path: PathBuf,
    pub notes: Vec<String>,
}

/// Build a dossier from a program folder and write it.
///
/// `out` overrides the output file; the default is
/// `<program-dir>/<program-id>.dossier`.
pub fn build(
    program_dir: &Path,
    out: Option<&Path>,
    program_id: Option<&str>,
) -> Result<BuildOutcome, String> {
    let Imported {
        program,
        sources,
        items,
        notes,
    } = import::read_program_dir(program_dir, program_id)?;
    let dossier = Dossier {
        schema: DOSSIER_SCHEMA.to_string(),
        tool: ToolInfo {
            name: crate::TOOL.to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        },
        program,
        sources,
        items,
    };
    let path = match out {
        Some(p) => p.to_path_buf(),
        None => program_dir.join(format!("{}.dossier", dossier.program.id)),
    };
    write_dossier(&dossier, &path)?;
    Ok(BuildOutcome {
        dossier,
        path,
        notes,
    })
}

/// Serialise and write, atomically: a partial dossier is never left behind.
pub fn write_dossier(dossier: &Dossier, path: &Path) -> Result<(), String> {
    let body = crate::render::json::pretty(dossier)?;
    let tmp = path.with_extension("dossier.tmp");
    fs::write(&tmp, body).map_err(|e| format!("cannot write {}: {e}", tmp.display()))?;
    fs::rename(&tmp, path).map_err(|e| format!("cannot rename into {}: {e}", path.display()))?;
    Ok(())
}
