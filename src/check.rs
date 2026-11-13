//! Assemble the evidence for one finding.
//!
//! This is the only place that decides what `priorart check` reports. Each gate
//! contributes its own evidence and nothing is collapsed into a verdict.

use crate::dossier::import;
use crate::gates;
use crate::matching::{self, Severity};
use crate::model::{CheckReport, Dossier, DossierRef, FindingRef, CHECK_SCHEMA};
use std::path::Path;

/// Build the report for a finding and dossier.
pub fn build(
    finding_path: &Path,
    finding_text: &str,
    finding_sha256: &str,
    dossier_path: &Path,
    dossier: &Dossier,
    top_k: usize,
) -> CheckReport {
    let profile = matching::Profile::from_text(finding_text);
    let claimed = matching::claimed_severity(finding_text);
    let mechanism = matching::mechanism_of(finding_text);
    let ranked = matching::rank(&dossier.items, finding_text, top_k);
    let prior_art = gates::prior_art(dossier, &ranked, mechanism);
    let exclusions = gates::exclusions(dossier, finding_text);
    let payout = gates::payout(dossier, claimed);
    let scope_evidence = gates::scope(dossier, finding_text);
    let coverage = gates::prior_audit_coverage(dossier, finding_text);

    let mut notes = Vec::new();
    for line in coverage {
        notes.push(format!("G4 prior audits: {line}"));
    }
    for line in scope_evidence.lines() {
        notes.push(format!("G3 scope: {line}"));
    }
    if profile.symbols.is_empty() {
        notes.push(
            "the finding names no strong code identifiers (file paths, snake_case, camelCase, or backticked spans), so no prior item can be ranked; priorart only scores identifiers"
                .to_string(),
        );
    } else if profile.symbols.len() < matching::MIN_SHARED_IDENTIFIERS {
        notes.push(format!(
            "the finding names {} strong identifier(s); at least {} are needed before an item can be ranked",
            profile.symbols.len(),
            matching::MIN_SHARED_IDENTIFIERS
        ));
    }
    if ranked.is_empty()
        && !dossier.items.is_empty()
        && profile.symbols.len() >= matching::MIN_SHARED_IDENTIFIERS
    {
        notes.push(
            "no prior item shares two symbol identifiers with the finding. That is not evidence of novelty: the dossier only holds what you saved."
                .to_string(),
        );
    }
    if dossier.items.is_empty() {
        notes.push("the dossier holds no items; rebuild it from a folder that has a program page, known issues or audits".to_string());
    }
    notes.push(
        "priorart does not decide duplicates, exclusion, or payment. Read the quoted lines."
            .to_string(),
    );

    CheckReport {
        schema: CHECK_SCHEMA.to_string(),
        tool_version: env!("CARGO_PKG_VERSION").to_string(),
        finding: FindingRef {
            path: finding_path.to_string_lossy().replace('\\', "/"),
            sha256: finding_sha256.to_string(),
            identifiers: profile.all_identifiers(),
            claimed_severity: claimed.map(|s| s.label().to_string()),
            mechanism_class: mechanism.map(str::to_string),
        },
        dossier: DossierRef {
            path: dossier_path.to_string_lossy().replace('\\', "/"),
            schema: dossier.schema.clone(),
            program_id: dossier.program.id.clone(),
        },
        prior_art,
        exclusions,
        payout,
        notes,
    }
}

/// The exit code a check run implies, given the opt-in failure flags.
pub fn failure_exit(
    report: &CheckReport,
    fail_on_excluded: bool,
    fail_on_unpayable: bool,
) -> Option<u8> {
    if fail_on_excluded && !report.exclusions.is_empty() {
        return Some(crate::error::EVIDENCE_EXIT);
    }
    if fail_on_unpayable && report.payout.payable == Some(false) {
        return Some(crate::error::EVIDENCE_EXIT);
    }
    None
}

/// Read a finding file, in UTF-8 with a BOM tolerated.
pub fn read_finding(path: &Path) -> Result<(String, String), String> {
    let bytes = std::fs::read(path).map_err(|e| {
        format!(
            "cannot read finding {}: {e}. Pass a markdown file with the finding text.",
            path.display()
        )
    })?;
    let text = import::strip_bom(&String::from_utf8_lossy(&bytes)).replace("\r\n", "\n");
    Ok((text, crate::dossier::sha256::hex(&bytes)))
}

/// Is this a severity the payout gate can compare? Used by the CLI to reject a
/// bogus --severity value.
pub fn parse_severity(raw: &str) -> Option<Severity> {
    Severity::parse(raw)
}
