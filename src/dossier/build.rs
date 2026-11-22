//! Assemble and write one dossier.
//!
//! The output is deterministic: sources sorted by path, items in document
//! order, ids assigned from those orders, and no wall-clock field anywhere. Two
//! builds over the same folder are byte-identical, which a test asserts.

use crate::dossier::import::{self, ImportOptions, Imported};
use crate::dossier::load::MAX_DOSSIER_BYTES;
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

/// Build a dossier from a program folder and write it, with the default
/// options (pdftotext enabled as a fallback).
///
/// `out` overrides the output file; the default is
/// `<program-dir>/<program-id>.dossier`.
pub fn build(
    program_dir: &Path,
    out: Option<&Path>,
    program_id: Option<&str>,
) -> Result<BuildOutcome, String> {
    build_with_options(program_dir, out, program_id, &ImportOptions::default())
}

/// Build a dossier with explicit import options.
pub fn build_with_options(
    program_dir: &Path,
    out: Option<&Path>,
    program_id: Option<&str>,
    options: &ImportOptions,
) -> Result<BuildOutcome, String> {
    let Imported {
        program,
        sources,
        items,
        notes,
    } = import::read_program_dir_with(program_dir, program_id, options)?;
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
///
/// The serialized body is checked against the same limit `check` applies
/// before the file is written, so a dossier this build writes can be read.
pub fn write_dossier(dossier: &Dossier, path: &Path) -> Result<(), String> {
    write_dossier_with_limit(dossier, path, MAX_DOSSIER_BYTES)
}

/// The write path with an explicit limit, so a test can exercise the real
/// call site without building a 64 MiB dossier.
fn write_dossier_with_limit(dossier: &Dossier, path: &Path, limit: u64) -> Result<(), String> {
    let body = crate::render::json::pretty(dossier)?;
    check_dossier_body_size(body.len() as u64, path, limit)?;
    let tmp = path.with_extension("dossier.tmp");
    fs::write(&tmp, body).map_err(|e| format!("cannot write {}: {e}", tmp.display()))?;
    fs::rename(&tmp, path).map_err(|e| format!("cannot rename into {}: {e}", path.display()))?;
    Ok(())
}

fn check_dossier_body_size(len: u64, path: &Path, limit: u64) -> Result<(), String> {
    if len > limit {
        return Err(format!(
            "the dossier for {} would be {}; priorart check reads a dossier up to {} MiB. Split the program folder (for example, one folder per audit set) and build again.",
            path.display(),
            import::mib(len),
            limit / (1024 * 1024)
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Program;

    fn scratch(label: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("priorart-build-{}-{label}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create scratch dir");
        dir
    }

    fn minimal_dossier() -> Dossier {
        Dossier {
            schema: DOSSIER_SCHEMA.to_string(),
            tool: ToolInfo {
                name: crate::TOOL.to_string(),
                version: "0".to_string(),
            },
            program: Program::default(),
            sources: Vec::new(),
            items: Vec::new(),
        }
    }

    #[test]
    fn a_dossier_over_the_check_limit_is_refused_before_it_is_written() {
        let path = Path::new("out.dossier");
        let err = check_dossier_body_size(MAX_DOSSIER_BYTES + 1, path, MAX_DOSSIER_BYTES)
            .expect_err("one byte over the limit");
        assert!(err.contains("out.dossier"), "{err}");
        assert!(err.contains("64 MiB"), "{err}");
        assert!(err.contains("check reads a dossier"), "{err}");
        assert!(err.contains("Split the program folder"), "{err}");
        assert!(check_dossier_body_size(MAX_DOSSIER_BYTES, path, MAX_DOSSIER_BYTES).is_ok());
    }

    #[test]
    fn the_write_path_checks_the_limit_before_writing_anything() {
        let dir = scratch("limit-call-site");
        let path = dir.join("out.dossier");
        let tmp = path.with_extension("dossier.tmp");
        let err = write_dossier_with_limit(&minimal_dossier(), &path, 1)
            .expect_err("the serialized body is over a one-byte limit");
        assert!(err.contains("would be"), "{err}");
        assert!(!path.exists(), "the target was written: {err}");
        assert!(!tmp.exists(), "the temp file was written: {err}");
        let _ = fs::remove_dir_all(&dir);
    }
}
