//! The command line. Every flag here is documented in docs/cli.md, and every
//! command and exit code is exercised by tests/cli.rs.

use crate::anchors::{bench, parse_repo, resolve_text, BenchOptions, IndexOptions, IndexSet};
use crate::candidates;
use crate::check;
use crate::dossier::{build, import, load};
use crate::error::EVIDENCE_EXIT;
use crate::ledger;
use crate::matching::{DEFAULT_TOP_K, MAX_TOP_K};
use crate::model::Dossier;
use crate::render::{json, text};
use crate::{init, rejections, taxonomy, vocab};
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
        /// Never run pdftotext: a PDF the built-in scrape cannot read is
        /// reported unread, so poppler never parses an untrusted file
        #[arg(long)]
        no_pdftotext: bool,
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
    /// Resolve the code a finding cites to spans in an audited repository
    Anchors {
        #[command(subcommand)]
        action: AnchorsAction,
    },
    /// Classify finding or prior-item text into the mechanism taxonomy
    Taxonomy {
        #[command(subcommand)]
        action: TaxonomyAction,
    },
    /// List prior items that share an anchor, a call-graph neighbour or a
    /// mechanism in the same contract with a finding
    Candidates {
        /// The finding to generate candidates for, as markdown
        #[arg(value_name = "FINDING")]
        finding: Option<PathBuf>,
        /// A .dossier, a directory of .md/.txt prior items, or a prepared corpus JSON
        #[arg(long, value_name = "PATH")]
        corpus: Option<PathBuf>,
        /// Write a prepared corpus to FILE and stop; omit the finding
        #[arg(long, value_name = "FILE")]
        prepare: Option<PathBuf>,
        /// Repository root to index; repeat for several checkouts
        #[arg(long = "repo", value_name = "DIR")]
        repos: Vec<PathBuf>,
        /// Repository index JSON written by `priorart anchors index --json`; repeatable
        #[arg(long = "index", value_name = "FILE")]
        indexes: Vec<PathBuf>,
        /// Repository id, `owner/name` (single --repo runs only)
        #[arg(long, value_name = "ID")]
        repo_id: Option<String>,
        /// Pinned commit the checkout is at (single --repo runs only)
        #[arg(long, value_name = "SHA", default_value = "")]
        commit: String,
        /// How many candidates to show
        #[arg(long, value_name = "N", default_value_t = candidates::DEFAULT_TOP_K)]
        top_k: usize,
        /// Prior item id to leave out; repeatable
        #[arg(long = "exclude", value_name = "ID")]
        exclude: Vec<String>,
        /// Print priorart.candidates/v1 JSON
        #[arg(long)]
        json: bool,
    },
    /// What the gates check, on one page
    Rules,
}

#[derive(Subcommand)]
enum TaxonomyAction {
    /// Extract root-cause, impact and asset classes from one or more text files
    Classify {
        /// UTF-8 text files to classify
        #[arg(value_name = "FILE", required = true)]
        files: Vec<PathBuf>,
        /// Print priorart.taxonomy-classify/v1 JSON
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum AnchorsAction {
    /// Parse a Solidity repository and print its contracts, functions, state
    /// variables, call graph and read/write sets
    Index {
        /// Repository root
        repo: PathBuf,
        /// Repository id, `owner/name` (default: the directory name)
        #[arg(long, value_name = "ID")]
        repo_id: Option<String>,
        /// Pinned commit the checkout is at
        #[arg(long, value_name = "SHA", default_value = "")]
        commit: String,
        /// Print the index JSON
        #[arg(long)]
        json: bool,
    },
    /// Resolve every code citation in one finding or prior-item text
    Resolve {
        /// Repository root; repeat for several checkouts
        #[arg(long = "repo", value_name = "DIR", required = true)]
        repos: Vec<PathBuf>,
        /// Repository id, `owner/name` (single-repository runs only)
        #[arg(long, value_name = "ID")]
        repo_id: Option<String>,
        /// Pinned commit the checkout is at
        #[arg(long, value_name = "SHA", default_value = "")]
        commit: String,
        /// Text file to resolve
        #[arg(long, value_name = "FILE")]
        text_file: PathBuf,
        /// Print priorart.anchors/v1 JSON
        #[arg(long)]
        json: bool,
    },
    /// Measure code-citation resolution over the DEV corpora under eval/benchmark
    Bench {
        /// Benchmark root (the directory holding tapioca's and the five corpus directories)
        #[arg(long, value_name = "DIR", default_value = "eval/benchmark")]
        benchmark_root: PathBuf,
        /// Fetch cache root written by eval/benchmark/fetch.py
        #[arg(long, value_name = "DIR")]
        cache_root: PathBuf,
        /// Directory holding one checkout per corpus: <sources-root>/<corpus>/
        #[arg(long, value_name = "DIR")]
        sources_root: PathBuf,
        /// Corpus to measure; repeat to override the default six
        #[arg(long = "corpus", value_name = "NAME")]
        corpora: Vec<String>,
        /// Accept a checkout whose git HEAD is not the pinned commit
        #[arg(long)]
        allow_unverified_commit: bool,
        /// Print priorart.anchors-bench/v1 JSON
        #[arg(long)]
        json: bool,
    },
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
            no_pdftotext,
        } => {
            let options = import::ImportOptions {
                use_pdftotext: !no_pdftotext,
            };
            let outcome = build::build_with_options(
                &program_dir,
                out.as_deref(),
                program.as_deref(),
                &options,
            )?;
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
        Command::Anchors { action } => run_anchors(action),
        Command::Taxonomy { action } => match action {
            TaxonomyAction::Classify { files, json } => run_taxonomy_classify(&files, json),
        },
        Command::Candidates {
            finding,
            corpus,
            prepare,
            repos,
            indexes,
            repo_id,
            commit,
            top_k,
            exclude,
            json,
        } => run_candidates(
            finding.as_deref(),
            corpus.as_deref(),
            prepare.as_deref(),
            &repos,
            &indexes,
            repo_id.as_deref(),
            &commit,
            top_k,
            &exclude,
            json,
        ),
        Command::Rules => {
            print!("{}", text::rules());
            Ok(ExitCode::SUCCESS)
        }
    }
}

fn run_anchors(action: AnchorsAction) -> Result<ExitCode, String> {
    match action {
        AnchorsAction::Index {
            repo,
            repo_id,
            commit,
            json,
        } => {
            let id = repo_id.unwrap_or_else(|| repo_id_from_dir(&repo));
            let index = parse_repo(&repo, &id, &commit, &IndexOptions::default())?;
            if json {
                print!("{}", json::pretty(&index)?);
            } else {
                print!("{}", text::anchors_index(&index));
            }
            Ok(ExitCode::SUCCESS)
        }
        AnchorsAction::Resolve {
            repos,
            repo_id,
            commit,
            text_file,
            json,
        } => {
            if repos.len() > 1 && repo_id.is_some() {
                return Err("--repo-id names one repository; pass one --repo with it".to_string());
            }
            if repos.len() > 1 && !commit.is_empty() {
                return Err("--commit names one repository; pass one --repo with it".to_string());
            }
            let mut indexes = Vec::new();
            for (i, repo) in repos.iter().enumerate() {
                let id = if i == 0 {
                    repo_id.clone().unwrap_or_else(|| repo_id_from_dir(repo))
                } else {
                    repo_id_from_dir(repo)
                };
                indexes.push(parse_repo(repo, &id, &commit, &IndexOptions::default())?);
            }
            let text = std::fs::read_to_string(&text_file)
                .map_err(|e| format!("cannot read {}: {e}", text_file.display()))?;
            let set = resolve_text(&text, &IndexSet::new(indexes));
            if json {
                print!("{}", json::pretty(&set)?);
            } else {
                print!("{}", text::anchors_resolve(&set));
            }
            Ok(ExitCode::SUCCESS)
        }
        AnchorsAction::Bench {
            benchmark_root,
            cache_root,
            sources_root,
            corpora,
            allow_unverified_commit,
            json,
        } => {
            let mut options = BenchOptions::new(benchmark_root, cache_root, sources_root);
            options.allow_unverified_commit = allow_unverified_commit;
            if !corpora.is_empty() {
                options.corpora = corpora;
            }
            let report = bench(&options)?;
            if json {
                print!("{}", json::pretty(&report)?);
            } else {
                print!("{}", text::anchors_bench(&report));
            }
            Ok(ExitCode::SUCCESS)
        }
    }
}

fn repo_id_from_dir(repo: &Path) -> String {
    repo.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "repository".to_string())
}

/// `priorart taxonomy classify <file>... [--json]`.
///
/// The extractor is pure; this handler is the only place that reads a file.
/// A non-regular file, a non-UTF-8 file or a file over the cap is a usage
/// error that names the reason, never a panic and never a partial result.
fn run_taxonomy_classify(files: &[PathBuf], as_json: bool) -> Result<ExitCode, String> {
    if files.is_empty() {
        return Err("at least one text file is required".to_string());
    }
    let mut results = Vec::new();
    for file in files {
        let meta =
            crate::dossier::import::require_regular_file_from(file, "text").map_err(|e| {
                format!(
                    "{}. Pass a UTF-8 text file with the finding.",
                    e.trim_end_matches('.')
                )
            })?;
        let refuse = |len: u64| -> Result<(), String> {
            if len > taxonomy::MAX_TEXT_BYTES as u64 {
                return Err(format!(
                    "text {} is {}; priorart classifies text up to {} MiB. Quote the relevant section in a smaller file.",
                    file.display(),
                    crate::dossier::import::mib(len),
                    taxonomy::MAX_TEXT_BYTES / (1024 * 1024)
                ));
            }
            Ok(())
        };
        refuse(meta.len())?;
        let bytes =
            std::fs::read(file).map_err(|e| format!("cannot read text {}: {e}", file.display()))?;
        refuse(bytes.len() as u64)?;
        let text = String::from_utf8(bytes).map_err(|_| {
            format!(
                "{} is not UTF-8 text; priorart classifies UTF-8 text files only.",
                file.display()
            )
        })?;
        let mechanism = taxonomy::classify(&text);
        results.push(taxonomy::FileClassification {
            file: file.to_string_lossy().replace('\\', "/"),
            text_sha256: crate::dossier::sha256::hex(text.as_bytes()),
            text_bytes: text.len(),
            mechanism,
        });
    }
    let report = taxonomy::ClassifyReport::new(results);
    if as_json {
        print!("{}", json::pretty(&report)?);
    } else {
        print!("{}", taxonomy::render_text(&report));
    }
    Ok(ExitCode::SUCCESS)
}

/// `priorart candidates ...`.
///
/// Two modes share one command: with a finding, generate candidates against a
/// corpus; with `--prepare FILE`, resolve and classify the corpus once and
/// write the cache. The finding and its corpus are both required in the first
/// mode; the second takes no finding. A bad combination is a usage error, not
/// a guess.
#[allow(clippy::too_many_arguments)]
fn run_candidates(
    finding: Option<&Path>,
    corpus_path: Option<&Path>,
    prepare_out: Option<&Path>,
    repos: &[PathBuf],
    indexes: &[PathBuf],
    repo_id: Option<&str>,
    commit: &str,
    top_k: usize,
    exclude: &[String],
    as_json: bool,
) -> Result<ExitCode, String> {
    if top_k == 0 || top_k > candidates::MAX_TOP_K {
        return Err(format!(
            "--top-k must be between 1 and {}, got {top_k}",
            candidates::MAX_TOP_K
        ));
    }
    let corpus_path = corpus_path.ok_or_else(|| {
        "a corpus path is required. Pass --corpus <path>, e.g. --corpus eval/benchmark/build/dossier-a.dossier.".to_string()
    })?;
    let index_set = build_candidate_index(repos, indexes, repo_id, commit)?;
    let corpus = candidates::load_corpus(corpus_path)?;

    if let Some(out) = prepare_out {
        if finding.is_some() {
            return Err(
                "--prepare writes the corpus and takes no finding. Omit the finding, or run again without --prepare."
                    .to_string(),
            );
        }
        if !exclude.is_empty() {
            return Err("--exclude applies to a finding run, not --prepare.".to_string());
        }
        let prepared = candidates::prepare_corpus(&corpus, index_set.as_ref());
        let with_anchors = prepared
            .items
            .iter()
            .filter(|item| item.anchors.as_ref().is_some_and(|a| !a.is_empty()))
            .count();
        let with_mechanism = prepared
            .items
            .iter()
            .filter(|item| {
                item.mechanism
                    .as_ref()
                    .is_some_and(|m| !m.root_causes.is_empty())
            })
            .count();
        let body = json::pretty(&prepared)?;
        std::fs::write(out, body.as_bytes())
            .map_err(|e| format!("cannot write prepared corpus {}: {e}", out.display()))?;
        let report = candidates::PrepareReport {
            schema: candidates::CANDIDATES_PREPARE_SCHEMA.to_string(),
            tool_version: env!("CARGO_PKG_VERSION").to_string(),
            corpus: corpus.path.clone(),
            corpus_kind: corpus.kind.as_str().to_string(),
            items: prepared.items.len(),
            anchors: with_anchors,
            mechanisms: with_mechanism,
            repositories: prepared.repositories.clone(),
            out: out.to_string_lossy().replace('\\', "/"),
        };
        if as_json {
            print!("{}", json::pretty(&report)?);
        } else {
            println!(
                "prepared {} prior item(s) from {} ({}) -> {}",
                report.items,
                text::sanitize(&report.corpus),
                report.corpus_kind,
                text::sanitize(&report.out)
            );
            println!(
                "  {} item(s) with anchors, {} with a mechanism, {} index pin(s)",
                report.anchors,
                report.mechanisms,
                report.repositories.len()
            );
        }
        return Ok(ExitCode::SUCCESS);
    }

    let finding = finding.ok_or_else(|| {
        "a finding is required. Pass a markdown file as the first argument, or use --prepare <file> to cache a corpus."
            .to_string()
    })?;
    // A prepared corpus records the repository pins its anchors came from. If
    // none of those pins is in the supplied index, every anchor would compare
    // against a different key and the run would silently return nothing; that
    // is a configuration error, not an abstention.
    if corpus.prepared && !corpus.repositories.is_empty() {
        if let Some(index) = &index_set {
            let mut supplied: Vec<String> = index
                .repos
                .iter()
                .map(|r| format!("{}@{}", r.repo, r.commit))
                .collect();
            supplied.sort();
            supplied.dedup();
            if !corpus.repositories.iter().any(|r| supplied.contains(r)) {
                return Err(format!(
                    "the prepared corpus was built against {}, but the supplied index is {}. Pass the same --repo/--repo-id/--commit, or rebuild the corpus with --prepare.",
                    corpus.repositories.join(", "),
                    supplied.join(", ")
                ));
            }
        }
    }
    let (finding_text, finding_sha) = check::read_finding(finding)?;
    let mut options = candidates::GenerateOptions::new(index_set.as_ref(), finding_sha);
    options.top_k = top_k;
    options.exclude = exclude.iter().cloned().collect();
    let report = candidates::generate(&finding_text, finding, &corpus, &options);
    if as_json {
        print!("{}", json::pretty(&report)?);
    } else {
        print!("{}", candidates::render_text(&report));
    }
    Ok(ExitCode::SUCCESS)
}

/// Resolve `--repo` checkouts and `--index` files into one index set, with the
/// same single-repository rules as `anchors resolve`.
fn build_candidate_index(
    repos: &[PathBuf],
    indexes: &[PathBuf],
    repo_id: Option<&str>,
    commit: &str,
) -> Result<Option<IndexSet>, String> {
    if repos.len() > 1 && repo_id.is_some() {
        return Err("--repo-id names one repository; pass one --repo with it".to_string());
    }
    if repos.len() > 1 && !commit.is_empty() {
        return Err("--commit names one repository; pass one --repo with it".to_string());
    }
    if repos.is_empty() && repo_id.is_some() {
        return Err("--repo-id names one repository; pass one --repo with it".to_string());
    }
    if repos.is_empty() && !commit.is_empty() {
        return Err("--commit names one repository; pass one --repo with it".to_string());
    }
    let mut parsed = Vec::new();
    for (i, repo) in repos.iter().enumerate() {
        let id = if i == 0 {
            repo_id
                .map(str::to_string)
                .unwrap_or_else(|| repo_id_from_dir(repo))
        } else {
            repo_id_from_dir(repo)
        };
        parsed.push(parse_repo(repo, &id, commit, &IndexOptions::default())?);
    }
    for file in indexes {
        parsed.push(candidates::load_index(file)?);
    }
    if parsed.is_empty() {
        Ok(None)
    } else {
        Ok(Some(IndexSet::new(parsed)))
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
            match std::fs::symlink_metadata(p) {
                Err(_) => {
                    return Err(format!(
                        "{} does not exist. Build one with priorart build <program-dir>.",
                        p.display()
                    ))
                }
                Ok(meta) if !meta.file_type().is_file() => {
                    return Err(format!(
                        "{} is not a regular file; priorart reads regular files only. Pass --dossier with a regular file.",
                        p.display()
                    ))
                }
                Ok(_) => {}
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
