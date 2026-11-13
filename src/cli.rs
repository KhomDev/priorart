//! The command line. Every flag here is documented in docs/cli.md, and every
//! command and exit code is exercised by tests/cli.rs.

use crate::check;
use crate::dossier::{build, load};
use crate::error::EVIDENCE_EXIT;
use crate::ledger;
use crate::matching::{DEFAULT_TOP_K, MAX_TOP_K};
use crate::model::Dossier;
use crate::render::{json, text};
use crate::{init, rejections, vocab};
use clap::{CommandFactory, FromArgMatches, Parser, Subcommand};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Parser)]
#[command(
    name = "priorart",
    version,
    about = "Offline prior-art dossiers for audit findings: has this been found, is it excluded, can it be paid?",
    long_about = "priorart answers three questions about a finding with quoted evidence and page\n\
                  numbers, never with a verdict:\n\n\
                  1. Has this been found?  ranked prior items sharing at least two strong code\n\
                     identifiers with your finding.\n\
                  2. Is it excluded?      the program's own exclusion clauses, classified into\n\
                     the 14 clause classes, against your preconditions.\n\
                  3. Can it be paid?      your claimed severity against the program's payout\n\
                     tier shape and floor.\n\n\
                  Everything is read from files you saved. The tool never touches the network.\n\
                  Build a dossier once per program (priorart build), then run every finding\n\
                  against it (priorart check)."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Build one deterministic dossier from a folder of saved, offline inputs
    Build {
        /// Folder holding scope.md, known-issues.*, audits/* and optional program.yaml
        program_dir: PathBuf,
        /// Output file (default: <program-dir>/<program-id>.dossier)
        #[arg(long, value_name = "FILE")]
        out: Option<PathBuf>,
        /// Program id override (default: program.yaml id, else the folder name)
        #[arg(long, value_name = "ID")]
        program: Option<String>,
        /// Print the dossier JSON to stdout instead of the summary
        #[arg(long)]
        json: bool,
    },
    /// Report quoted evidence for one finding against a dossier
    Check {
        /// The finding to check, as markdown
        finding: PathBuf,
        /// Dossier built by priorart build (default: the single .dossier beside the finding)
        #[arg(long, value_name = "FILE")]
        dossier: Option<PathBuf>,
        /// How many ranked prior items to show
        #[arg(long, value_name = "N", default_value_t = DEFAULT_TOP_K)]
        top_k: usize,
        /// Print priorart.check/v1 JSON instead of text
        #[arg(long)]
        json: bool,
        /// Exit 1 when any exclusion clause class matches
        #[arg(long)]
        fail_on_excluded: bool,
        /// Exit 1 when the claimed severity is below the program's floor
        #[arg(long)]
        fail_on_unpayable: bool,
    },
    /// Validate a private program ledger against schema v1
    Ledger {
        #[command(subcommand)]
        action: LedgerAction,
    },
    /// Inspect the compiled vocabularies
    Vocab {
        #[command(subcommand)]
        action: VocabAction,
    },
    /// Scaffold a program folder that priorart build can read
    Init {
        /// Folder to scaffold (default: the current directory)
        dir: Option<PathBuf>,
        /// Overwrite files that already exist
        #[arg(long)]
        force: bool,
    },
    /// Explain a clause class, gate or tier shape
    Explain {
        /// A clause-class id, a gate id (G1-G4) or a tier-shape id
        id: String,
    },
    /// Print the shipped rejection dataset's summary
    Rejections {
        #[command(subcommand)]
        action: RejectionsAction,
    },
    /// What the gates check, on one page
    Rules,
}

#[derive(Subcommand)]
enum LedgerAction {
    /// Validate one or more ledger YAML files
    Validate {
        /// Ledger files; a shell glob works
        ledgers: Vec<PathBuf>,
        /// Print the report as JSON
        #[arg(long)]
        json: bool,
        /// Treat warnings as errors
        #[arg(long)]
        strict: bool,
        /// Relax the placeholder-host check (auto for *.example.yaml)
        #[arg(long)]
        example_mode: bool,
        /// Print the shipped vocabularies instead of validating
        #[arg(long)]
        taxonomy: bool,
    },
}

#[derive(Subcommand)]
enum VocabAction {
    /// List the 5 tier shapes, 14 clause classes and the gate cross-reference
    List {
        /// Print JSON instead of text
        #[arg(long)]
        json: bool,
    },
    /// Show one vocabulary in full: tier-shapes, clause-classes or gate-crossref
    Show {
        /// tier-shapes | clause-classes | gate-crossref
        name: String,
        /// Print JSON instead of text
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum RejectionsAction {
    /// Print the dataset summary: totals, triage, gates, provenance
    Summary {
        /// Print the dataset JSON (summary.json plus provenance) instead of text
        #[arg(long)]
        json: bool,
    },
}

pub fn main_entry() -> ExitCode {
    let matches = Cli::command().get_matches();
    let cli = Cli::from_arg_matches(&matches).unwrap_or_else(|e| e.exit());
    match run(cli) {
        Ok(code) => code,
        Err(message) => {
            eprintln!("priorart: {}", text::sanitize(&message));
            ExitCode::from(crate::error::USAGE_EXIT)
        }
    }
}

fn run(cli: Cli) -> Result<ExitCode, String> {
    match cli.command {
        Command::Build {
            program_dir,
            out,
            program,
            json,
        } => {
            let outcome = build::build(&program_dir, out.as_deref(), program.as_deref())?;
            if json {
                print!("{}", json::pretty(&outcome.dossier)?);
                for note in &outcome.notes {
                    eprintln!("note: {}", text::sanitize(note));
                }
            } else {
                print!("{}", text::dossier_summary(&outcome));
            }
            Ok(ExitCode::SUCCESS)
        }
        Command::Check {
            finding,
            dossier,
            top_k,
            json,
            fail_on_excluded,
            fail_on_unpayable,
        } => run_check(
            &finding,
            dossier.as_deref(),
            top_k,
            json,
            fail_on_excluded,
            fail_on_unpayable,
        ),
        Command::Ledger { action } => match action {
            LedgerAction::Validate {
                ledgers,
                json,
                strict,
                example_mode,
                taxonomy,
            } => run_ledger(&ledgers, json, strict, example_mode, taxonomy),
        },
        Command::Vocab { action } => match action {
            VocabAction::List { json } => {
                if json {
                    let payload = serde_json::json!({
                        "schema_version": 1,
                        "tier_shapes": {
                            "source": "vocab/tier-shapes.yaml",
                            "count": vocab::tier_shapes().len(),
                            "ids": vocab::tier_shape_ids(),
                        },
                        "clause_classes": {
                            "source": "vocab/clause-classes.yaml",
                            "count": vocab::clause_classes().len(),
                            "ids": vocab::clause_class_ids(),
                        },
                        "gate_crossref": {
                            "source": "vocab/gate-crossref.yaml",
                            "answered_here": vocab::gate_crossref()
                                .answered_here
                                .iter()
                                .map(|(gate, row)| (gate.clone(), row.catalog.clone()))
                                .collect::<std::collections::BTreeMap<_, _>>(),
                        },
                    });
                    print!("{}", json::pretty(&payload)?);
                } else {
                    print!("{}", text::vocab_list());
                }
                Ok(ExitCode::SUCCESS)
            }
            VocabAction::Show { name, json } => {
                if json {
                    let payload = match name.as_str() {
                        "tier-shapes" => serde_json::to_value(vocab::tier_shapes_doc()),
                        "clause-classes" => serde_json::to_value(vocab::clause_classes_doc()),
                        "gate-crossref" => serde_json::to_value(vocab::gate_crossref()),
                        other => {
                            return Err(format!(
                                "no vocabulary named {other:?}. Use one of: tier-shapes, clause-classes, gate-crossref."
                            ))
                        }
                    }
                    .map_err(|e| format!("cannot serialise vocabulary: {e}"))?;
                    print!("{}", json::pretty(&payload)?);
                } else {
                    print!("{}", text::vocab_show(&name)?);
                }
                Ok(ExitCode::SUCCESS)
            }
        },
        Command::Init { dir, force } => {
            let dir = dir.unwrap_or_else(|| PathBuf::from("."));
            let wrote = init::run(&dir, force)?;
            println!("priorart {} - scaffolded {}\n", VERSION, dir.display());
            for rel in wrote {
                println!("  {rel}");
            }
            println!("\nNext: save the program's real page over scope.md, then run");
            println!("  priorart build {}", dir.display());
            Ok(ExitCode::SUCCESS)
        }
        Command::Explain { id } => {
            print!("{}", text::explain(&id)?);
            Ok(ExitCode::SUCCESS)
        }
        Command::Rejections { action } => match action {
            RejectionsAction::Summary { json } => {
                if json {
                    let payload = serde_json::json!({
                        "schema": crate::model::REJECTIONS_SCHEMA,
                        "dataset": rejections::summary(),
                        "provenance": rejections::provenance(),
                    });
                    print!("{}", json::pretty(&payload)?);
                } else {
                    print!("{}", text::rejections_summary());
                }
                Ok(ExitCode::SUCCESS)
            }
        },
        Command::Rules => {
            print!("{}", text::rules());
            Ok(ExitCode::SUCCESS)
        }
    }
}

fn run_check(
    finding: &Path,
    dossier_path: Option<&Path>,
    top_k: usize,
    as_json: bool,
    fail_on_excluded: bool,
    fail_on_unpayable: bool,
) -> Result<ExitCode, String> {
    if top_k == 0 || top_k > MAX_TOP_K {
        return Err(format!(
            "--top-k must be between 1 and {MAX_TOP_K}, got {top_k}"
        ));
    }
    let (finding_text, finding_sha) = check::read_finding(finding)?;
    let resolved: PathBuf = match dossier_path {
        Some(p) => {
            if !p.exists() {
                return Err(format!(
                    "{} does not exist. Build one with priorart build <program-dir>.",
                    p.display()
                ));
            }
            p.to_path_buf()
        }
        None => match load::discover_beside(finding)? {
            Some(p) => p,
            None => {
                return Err(format!(
                    "no dossier supplied and no .dossier file beside {}. Pass --dossier <file>, or build one with priorart build <program-dir>.",
                    finding.display()
                ))
            }
        },
    };
    let dossier: Dossier = load::load(&resolved)?;
    let report = check::build(
        finding,
        &finding_text,
        &finding_sha,
        &resolved,
        &dossier,
        top_k,
    );
    if as_json {
        print!("{}", json::pretty(&report)?);
    } else {
        print!("{}", text::check(&report, &dossier, &finding_text));
    }
    let code = check::failure_exit(&report, fail_on_excluded, fail_on_unpayable);
    Ok(match code {
        Some(code) => ExitCode::from(code),
        None => ExitCode::SUCCESS,
    })
}

fn run_ledger(
    ledgers: &[PathBuf],
    as_json: bool,
    strict: bool,
    example_mode: bool,
    taxonomy: bool,
) -> Result<ExitCode, String> {
    if taxonomy {
        if as_json {
            print!("{}", json::pretty(&ledger::taxonomy_payload()?)?);
        } else {
            print!("{}", text::vocab_list());
        }
        return Ok(ExitCode::SUCCESS);
    }
    if ledgers.is_empty() {
        return Err(
            "a ledger path is required (or pass --taxonomy). Pass at least one YAML file."
                .to_string(),
        );
    }
    let report = ledger::validate(ledgers, strict, example_mode)?;
    if as_json {
        print!("{}", json::pretty(&report)?);
    } else {
        for warning in &report.warnings {
            println!("WARN  {}", text::sanitize(warning));
        }
        for error in &report.errors {
            eprintln!("ERROR {}", text::sanitize(error));
        }
        println!();
        if report.ledgers.len() > 1 {
            for entry in &report.ledgers {
                println!(
                    "{}: {} {}, {} error(s), {} warning(s)",
                    entry.ledger,
                    entry.entries,
                    if entry.entries == 1 {
                        "entry"
                    } else {
                        "entries"
                    },
                    entry.errors.len(),
                    entry.warnings.len()
                );
            }
            println!();
        }
        println!(
            "{} {}, {} error(s), {} warning(s)",
            report.entries,
            if report.entries == 1 {
                "entry"
            } else {
                "entries"
            },
            report.errors.len(),
            report.warnings.len()
        );
    }
    Ok(if report.ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(EVIDENCE_EXIT)
    })
}

/// Re-exported so the binary and integration tests agree on the usage exit.
pub const USAGE_EXIT: u8 = crate::error::USAGE_EXIT;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_clap_surface_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn top_k_bounds_are_enforced() {
        assert!(run_check(
            Path::new("does-not-exist.md"),
            Some(Path::new("does-not-exist.dossier")),
            0,
            false,
            false,
            false
        )
        .is_err());
        assert!(run_check(
            Path::new("does-not-exist.md"),
            Some(Path::new("does-not-exist.dossier")),
            MAX_TOP_K + 1,
            false,
            false,
            false
        )
        .is_err());
    }
}
