//! priorart init: scaffold the program folder that priorart build reads.

use std::fs;
use std::path::Path;

/// Files written by init, with their templates.
pub const TEMPLATES: [(&str, &str); 4] = [
    ("program.yaml", PROGRAM_YAML),
    ("scope.md", SCOPE_MD),
    ("known-issues.md", KNOWN_ISSUES_MD),
    ("audits/README.md", AUDITS_README_MD),
];

const PROGRAM_YAML: &str = "# Saved by you; read by priorart build. Every field is optional\n\
# except payout.tier_shape when a payout block is present.\n\
#\n\
# id: the slug used in the dossier filename. Defaults to the folder name.\n\
# name, platform, url, scope_url: free text for the dossier header.\n\
# prior_audits: named audits, with the files and functions they covered.\n\
#\n\
# The payout block is your transcription of the program's rewards page.\n\
# tier_shape is one of the five shapes in vocab/tier-shapes.yaml:\n\
#   critical_only, high_and_above, medium_and_above, low_and_above, unknown\n\
# floor_status is documented, unstated or conflicting. An unstated floor is\n\
# NOT the same as Low paying.\n\
#\n\
# name: Example Program\n\
# platform: immunefi\n\
# url: https://example.invalid/program\n\
# scope_url: https://example.invalid/program/scope\n\
#\n\
# payout:\n\
#   tier_shape: medium_and_above\n\
#   floor_status: documented\n\
#   floor_severity: medium\n\
#   source_url: https://example.invalid/program/rewards\n\
#   retrieved: \"2026-11-01\"\n\
#   section: Rewards\n\
#   quote: \"Medium: $2,000\"\n\
#\n\
# prior_audits:\n\
#   - firm: Example Firm\n\
#     url: https://example.invalid/audit.pdf\n\
#     covers:\n\
#       - src/Vault.sol\n\
#     known_issues:\n\
#       - \"rounding in share price is acknowledged\"\n";

const SCOPE_MD: &str = "# Example Program - scope and rewards\n\
\n\
Save the program's own page here, as markdown, html or text. priorart reads\n\
this file; it never fetches a URL. Replace every line below with the real page.\n\
\n\
## Rewards\n\
\n\
- Critical: $50,000\n\
- High: $10,000\n\
- Medium: $2,000\n\
\n\
## In scope\n\
\n\
- src/Vault.sol\n\
- src/LoanBook.sol\n\
\n\
## Out of scope\n\
\n\
- Attacks requiring access to privileged addresses\n\
- Vulnerabilities exploitable through front-running only\n\
- Best-practice critiques and gas optimisations\n\
- Issues already listed in the known-issues section\n";

const KNOWN_ISSUES_MD: &str = "# Known issues\n\
\n\
The program's own known-issues list, verbatim. One bullet per issue. Any\n\
identifier you copy here (a function, a file, an error name) is what priorart\n\
matches on; paraphrasing it removes the match.\n\
\n\
- Rounding in distributeFees is acknowledged and will not be paid.\n";

const AUDITS_README_MD: &str = "# audits/\n\
\n\
Drop the prior audit reports here as .md, .txt or .pdf. One file per report;\n\
the file name becomes the source title.\n\
\n\
PDFs are read with pdftotext when it is installed. Without it, an uncompressed\n\
PDF is scraped and a compressed one is skipped with a warning - convert it with\n\
pdftotext -layout and save the .txt here instead.\n";

/// Scaffold a program folder. Refuses to overwrite existing files unless
/// force is set.
pub fn run(dir: &Path, force: bool) -> Result<Vec<String>, String> {
    fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    let mut wrote = Vec::new();
    for (rel, body) in TEMPLATES {
        let path = dir.join(rel);
        if path.exists() && !force {
            return Err(format!(
                "{} already exists. Re-run with --force to overwrite the scaffold files.",
                path.display()
            ));
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
        }
        fs::write(&path, body).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
        wrote.push(rel.to_string());
    }
    Ok(wrote)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn templates_are_valid_and_use_the_new_name() {
        for (rel, body) in TEMPLATES {
            assert!(!rel.is_empty());
            assert!(
                !body.contains("viability-gate"),
                "{rel} still mentions the old name"
            );
            assert!(!body.contains("vg "), "{rel} still mentions the old binary");
        }
    }
}
