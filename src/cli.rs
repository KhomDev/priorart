//! vg — score a draft audit finding against 22 documented rejection anti-patterns
//! and 7 pre-hunt gates plus the D1 known-issues gate.
//!
//! Offline, deterministic, no API key. Ships rules, not data: no program names, no
//! chains, no reward tables. Every target-specific input arrives at runtime.

use crate::{bench, engine, init, model, render};

use clap::{CommandFactory, FromArgMatches, Parser, Subcommand};
use engine::{CompiledRule, GateInputs, Severity};
use model::*;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// Rules are embedded so the installed binary is self-contained; --rules points
/// at a directory to override them.
const EMBEDDED_ANTI_PATTERNS: &str = include_str!("../rules/anti-patterns.yaml");
const EMBEDDED_GATES: &str = include_str!("../rules/gates.yaml");

const VERSION: &str = env!("CARGO_PKG_VERSION");

const DISCLAIMER: &str = "Advisory, never authoritative. This tool produces an outcome \
with reasons; it does not decide, and it cannot read your target's live rules. \
Over-filtering is its own failure mode - the catalog is a prioritiser, not a \
kill-switch. A triage rejection is not a final verdict. CLEAR means only that no \
known blocker was found by this tool.";

/// Default path for the local own-history log. Gitignored by design.
pub const DEFAULT_LOG: &str = ".vg-history.yaml";

/// Total bytes `--tests` reads from a directory.
const TEST_BUDGET: usize = 4_000_000;

/// Directories `--tests` never descends into: VCS metadata and build output.
const SKIP_DIRS: [&str; 9] = [
    ".git",
    ".hg",
    ".svn",
    "node_modules",
    "target",
    "out",
    "cache",
    "dist",
    "build",
];

#[derive(Parser)]
#[command(
    name = "vg",
    version,
    about = "Score a draft audit finding against documented rejection anti-patterns",
    long_about = "Before you spend a day writing a PoC for something that cannot be paid, \
                  run the finding through the catalog.\n\n\
                  Ships rules, not data: no program names, no chains, no reward tables. \
                  Target-specific input is supplied by you at runtime and never leaves \
                  your machine.\n\n\
                  NOTE ON THE NAME: vg is also the name of an unrelated bioinformatics \
                  tool. This crate installs BOTH a vg and a viability-gate binary so you \
                  can pick whichever does not collide on your PATH."
)]
struct Cli {
    /// Directory holding anti-patterns.yaml and gates.yaml (default: embedded)
    #[arg(long, global = true, value_name = "DIR")]
    rules: Option<PathBuf>,

    #[command(subcommand)]
    command: Command,
}

/// Target and program inputs shared by `check` and `gates`.
#[derive(clap::Args, Clone, Default)]
struct TargetArgs {
    /// Your private program ledger (enables G1, G2 and part of G4)
    #[arg(long)]
    ledger: Option<PathBuf>,

    /// The ledger entry to use when the ledger holds more than one program
    #[arg(long, value_name = "ID")]
    program: Option<String>,

    /// The severity of your thesis: critical, high, medium or low. Defaults to a
    /// "Severity:" line in the finding. G1 compares it with the payout floor.
    #[arg(long)]
    severity: Option<String>,

    /// In-scope file list, one path, directory/ or glob per line
    #[arg(long)]
    scope: Option<PathBuf>,

    /// Prior-audit references, one per line
    #[arg(long)]
    prior_audits: Option<PathBuf>,

    /// Path to the target's test suite (a directory or file)
    #[arg(long)]
    tests: Option<PathBuf>,

    /// Upstream project this target forks, or "none"
    #[arg(long)]
    fork_of: Option<String>,

    /// Known-issues corpus for this program (YAML). Drives gate D1. This is a
    /// KNOWN-ISSUE MATCHER: it can only see what you transcribed, so it cannot
    /// catch a concurrent duplicate.
    #[arg(long)]
    known_issues: Option<PathBuf>,

    /// Your own submission history (YAML). Enables gate G7, which flags
    /// probable repeats of a family you already submitted.
    #[arg(long)]
    log: Option<PathBuf>,
}

#[derive(Subcommand)]
enum Command {
    /// Score one or more draft findings against the anti-pattern catalog
    Check {
        /// Path(s) to the finding (markdown). Use - for stdin.
        #[arg(required = true, num_args = 1..)]
        findings: Vec<String>,

        /// Machine-readable output (an object for one finding, an array for several)
        #[arg(long)]
        json: bool,

        /// Score threshold below which the outcome is HOLD (default 50).
        /// The score is an uncalibrated heuristic, not a probability.
        #[arg(long, default_value_t = 50, value_parser = clap::value_parser!(u32).range(0..=100))]
        min_score: u32,

        /// One row per rule, table format
        #[arg(long)]
        table: bool,

        /// Exit non-zero when any outcome is KILL
        #[arg(long)]
        fail_on_kill: bool,

        #[command(flatten)]
        target: TargetArgs,
    },

    /// Run only the pre-hunt gates
    Gates {
        /// A draft finding to test the gates against. Without one, G1-G5 check
        /// only that their input is complete and well-formed.
        #[arg(long)]
        finding: Option<PathBuf>,
        #[arg(long)]
        json: bool,
        /// Show each gate's individual checks
        #[arg(long)]
        verbose: bool,
        #[command(flatten)]
        target: TargetArgs,
    },

    /// Explain one anti-pattern, with its detection patterns
    Explain { id: String },

    /// List the catalog
    Rules,

    /// Scaffold the input files a new target needs
    Init {
        /// Directory to write into (created if missing)
        #[arg(default_value = "target")]
        dir: PathBuf,
        /// Overwrite existing files
        #[arg(long)]
        force: bool,
    },

    /// Manage your own submission history (gate G7)
    Log {
        #[command(subcommand)]
        action: LogAction,
    },

    /// Measure the catalog against a labelled set of findings
    Bench {
        /// Directory of findings that were rejected
        #[arg(long)]
        rejected: PathBuf,
        /// Directory of findings that were accepted or paid
        #[arg(long)]
        accepted: Option<PathBuf>,
        /// CSV of labels, `filename,label` with label `rejected` or `accepted`.
        /// A label overrides the directory a file sits in.
        #[arg(long)]
        labels: Option<PathBuf>,
        /// Emit machine-readable output
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum LogAction {
    /// Record one submitted finding
    Add {
        /// The bug family label, as you write it
        #[arg(long)]
        family: String,
        /// The outcome: paid | rejected | duplicate | oos | informational | pending
        #[arg(long)]
        outcome: String,
        /// A free-text note
        #[arg(long)]
        note: Option<String>,
        /// ISO date (YYYY-MM-DD). Defaults to today.
        #[arg(long)]
        date: Option<String>,
        /// The log file (default: .vg-history.yaml)
        #[arg(long)]
        file: Option<PathBuf>,
    },
    /// Check a draft finding against your own history
    Check {
        /// The draft finding to check
        finding: PathBuf,
        /// The log file (default: .vg-history.yaml)
        #[arg(long)]
        file: Option<PathBuf>,
    },
}

pub fn main_entry() -> ExitCode {
    main_entry_named("vg")
}

/// Entry point for a binary installed under `name`, so `--version`, `--help`
/// and error prefixes say `viability-gate` when that is what the user ran.
pub fn main_entry_named(name: &'static str) -> ExitCode {
    let matches = Cli::command().name(name).bin_name(name).get_matches();
    let cli = Cli::from_arg_matches(&matches).unwrap_or_else(|e| e.exit());
    match run(cli) {
        Ok(code) => code,
        Err(msg) => {
            eprintln!("{name}: {}", render::sanitize(&msg));
            ExitCode::from(2)
        }
    }
}

/// Print text that may carry user-supplied strings, without control characters.
fn emit(s: &str) {
    print!("{}", render::sanitize(s));
}

fn load_rules(dir: Option<&Path>) -> Result<(Vec<CompiledRule>, Vec<Gate>), String> {
    let (ap_text, gate_text) = match dir {
        Some(d) => {
            let ap = std::fs::read_to_string(d.join("anti-patterns.yaml"))
                .map_err(|e| format!("cannot read {}/anti-patterns.yaml: {e}", d.display()))?;
            let g = std::fs::read_to_string(d.join("gates.yaml"))
                .map_err(|e| format!("cannot read {}/gates.yaml: {e}", d.display()))?;
            (ap, g)
        }
        None => (
            EMBEDDED_ANTI_PATTERNS.to_string(),
            EMBEDDED_GATES.to_string(),
        ),
    };

    let parsed: RuleFile =
        serde_saphyr::from_str(&ap_text).map_err(|e| format!("anti-patterns.yaml: {e}"))?;
    let gates: GateFile =
        serde_saphyr::from_str(&gate_text).map_err(|e| format!("gates.yaml: {e}"))?;
    for (file, version) in [
        ("anti-patterns.yaml", parsed.version),
        ("gates.yaml", gates.version),
    ] {
        if version != CATALOG_VERSION {
            return Err(format!(
                "{file}: version {version} is not supported (this build reads version \
                 {CATALOG_VERSION})"
            ));
        }
    }

    let compiled = parsed
        .rules
        .into_iter()
        .map(CompiledRule::new)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok((compiled, gates.gates))
}

fn read_text(path: &str) -> Result<String, String> {
    if path == "-" {
        use std::io::Read;
        let mut buf = String::new();
        std::io::stdin()
            .read_to_string(&mut buf)
            .map_err(|e| format!("stdin: {e}"))?;
        return Ok(strip_bom(buf));
    }
    let text = std::fs::read_to_string(path).map_err(|e| format!("cannot read {path}: {e}"))?;
    Ok(strip_bom(text))
}

/// Strip a UTF-8 BOM. Windows editors add one, and it corrupts the first
/// evidence line in the report.
fn strip_bom(s: String) -> String {
    match s.strip_prefix('\u{feff}') {
        Some(rest) => rest.to_string(),
        None => s,
    }
}

fn read_file(p: &Path) -> Result<String, String> {
    std::fs::read_to_string(p)
        .map(strip_bom)
        .map_err(|e| format!("cannot read {}: {e}", p.display()))
}

fn read_lines(path: &Option<PathBuf>) -> Result<Option<Vec<String>>, String> {
    let Some(p) = path else { return Ok(None) };
    let text = read_file(p)?;
    Ok(Some(
        text.lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .map(str::to_string)
            .collect(),
    ))
}

fn read_ledger(path: &Option<PathBuf>) -> Result<Option<Vec<LedgerEntry>>, String> {
    let Some(p) = path else { return Ok(None) };
    let text = read_file(p)?;
    let entries: Vec<LedgerEntry> =
        serde_saphyr::from_str(&text).map_err(|e| format!("{}: {e}", p.display()))?;
    Ok(Some(entries))
}

/// Load a known-issues corpus: a bare list, or a mapping with a known_issues:
/// key.
fn read_known_issues(path: &Option<PathBuf>) -> Result<Option<Vec<KnownIssue>>, String> {
    let Some(p) = path else { return Ok(None) };
    let text = read_file(p)?;
    if let Ok(list) = serde_saphyr::from_str::<Vec<KnownIssue>>(&text) {
        return Ok(Some(list));
    }
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Wrapper {
        known_issues: Vec<KnownIssue>,
    }
    let w: Wrapper = serde_saphyr::from_str(&text).map_err(|e| format!("{}: {e}", p.display()))?;
    Ok(Some(w.known_issues))
}

/// Read the researcher's own submission history: a bare list, or a mapping
/// with a submissions: key. A path you named that does not exist is an error,
/// not a silent "unknown".
fn read_log(path: &Option<PathBuf>) -> Result<Option<Vec<LogEntry>>, String> {
    let Some(p) = path else { return Ok(None) };
    if !p.exists() {
        return Err(format!(
            "no history log at {}. Run 'vg log add ...' to create one",
            p.display()
        ));
    }
    let text = read_file(p)?;
    init::parse_log(&text)
        .map(Some)
        .map_err(|e| format!("{}: {e}", p.display()))
}

/// Read a test suite: a single file, or every UTF-8 file under a directory.
///
/// Deterministic: files are read in sorted path order, VCS and build
/// directories are skipped, symbolic links are not followed, and the total is
/// capped at TEST_BUDGET bytes, cut on a character boundary.
fn read_tests(path: &Option<PathBuf>) -> Result<Option<Vec<(String, String)>>, String> {
    let Some(root) = path else { return Ok(None) };
    if root.is_file() {
        return Ok(Some(vec![(root.display().to_string(), read_file(root)?)]));
    }
    if !root.is_dir() {
        return Err(format!(
            "--tests {}: not a file or directory",
            root.display()
        ));
    }
    let mut files: Vec<PathBuf> = Vec::new();
    let mut stack = vec![root.clone()];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in rd.flatten() {
            let Ok(meta) = std::fs::symlink_metadata(entry.path()) else {
                continue;
            };
            let name = entry.file_name().to_string_lossy().to_string();
            if meta.file_type().is_symlink() {
                continue;
            } else if meta.is_dir() {
                if !SKIP_DIRS.contains(&name.as_str()) {
                    stack.push(entry.path());
                }
            } else if meta.is_file() {
                files.push(entry.path());
            }
        }
    }
    files.sort();
    let mut out = Vec::new();
    let mut budget = TEST_BUDGET;
    for f in files {
        if budget == 0 {
            break;
        }
        let Ok(text) = std::fs::read_to_string(&f) else {
            continue;
        };
        let mut cut = text.len().min(budget);
        while !text.is_char_boundary(cut) {
            cut -= 1;
        }
        budget -= cut;
        let rel = f
            .strip_prefix(root)
            .unwrap_or(&f)
            .to_string_lossy()
            .replace('\\', "/");
        out.push((rel, text[..cut].to_string()));
    }
    Ok(Some(out))
}

/// Everything read from the target arguments, owned.
struct Target {
    ledger: Option<Vec<LedgerEntry>>,
    program: Option<String>,
    severity: Option<Severity>,
    scope: Option<Vec<String>>,
    prior: Option<Vec<String>>,
    tests: Option<Vec<(String, String)>>,
    fork_of: Option<String>,
    known: Option<Vec<KnownIssue>>,
    log: Option<Vec<LogEntry>>,
}

impl Target {
    fn read(a: &TargetArgs) -> Result<Self, String> {
        let ledger = read_ledger(&a.ledger)?;
        if let Some(id) = &a.program {
            match &ledger {
                None => return Err("--program needs --ledger".into()),
                Some(entries) if !entries.iter().any(|e| &e.id == id) => {
                    return Err(format!("no program {id:?} in the ledger"));
                }
                Some(_) => {}
            }
        }
        let severity =
            match &a.severity {
                None => None,
                Some(s) => Some(Severity::parse(s).ok_or_else(|| {
                    format!("--severity {s:?}: use critical, high, medium or low")
                })?),
            };
        Ok(Self {
            ledger,
            program: a.program.clone(),
            severity,
            scope: read_lines(&a.scope)?,
            prior: read_lines(&a.prior_audits)?,
            tests: read_tests(&a.tests)?,
            fork_of: a.fork_of.clone(),
            known: read_known_issues(&a.known_issues)?,
            log: read_log(&a.log)?,
        })
    }

    fn inputs<'a>(&'a self, finding: Option<&'a str>) -> GateInputs<'a> {
        GateInputs {
            ledger: self.ledger.as_deref(),
            program: self.program.as_deref(),
            severity: self.severity,
            scope: self.scope.as_deref(),
            prior_audits: self.prior.as_deref(),
            tests: self.tests.as_deref(),
            fork_of: self.fork_of.as_deref(),
            own_log: self.log.as_deref(),
            known_issues: self.known.as_deref(),
            finding,
        }
    }
}

fn build_report(
    subject: String,
    text: &str,
    rules: &[CompiledRule],
    gates: &[Gate],
    threshold: u32,
    target: &Target,
) -> Report {
    let (mut findings, passed) = engine::evaluate(rules, text);

    // Known-issue matches are merged in before the outcome is computed, because
    // a known issue is a hard fail: it makes the finding Not Applicable rather
    // than merely unpaid.
    let mut ki_warnings: Vec<String> = Vec::new();
    if let Some(issues) = &target.known {
        let (ki_findings, warnings) = engine::match_known_issues(issues, text);
        ki_warnings = warnings;
        findings.extend(ki_findings);
    }
    // Hard fails first, then by weight, so the worst problem reads first.
    findings.sort_by(|a, b| {
        let rank = |v: Verdict| match v {
            Verdict::Hard => 0,
            Verdict::Soft => 1,
            Verdict::Warn => 2,
        };
        rank(a.verdict)
            .cmp(&rank(b.verdict))
            .then(b.weight.cmp(&a.weight))
    });

    let gate_outcomes = engine::evaluate_gates(gates, &target.inputs(Some(text)));
    let (outcome, viability_score) = engine::outcome(&findings, &gate_outcomes, threshold);
    Report {
        tool: "viability-gate".into(),
        version: VERSION.into(),
        subject,
        outcome,
        viability_score,
        threshold,
        findings,
        passed,
        gates: gate_outcomes,
        disclaimer: DISCLAIMER.into(),
        warnings: ki_warnings,
    }
}

fn run(cli: Cli) -> Result<ExitCode, String> {
    // init and log add write local files and need no rules, so they run before
    // the catalog is loaded - a user who has not yet read the docs should not hit
    // a load error on their first command.
    match &cli.command {
        Command::Init { dir, force } => return init::run(dir, *force),
        Command::Log { action } => return run_log(action, cli.rules.as_deref()),
        _ => {}
    }

    let (rules, gates) = load_rules(cli.rules.as_deref())?;

    match cli.command {
        Command::Init { .. } | Command::Log { .. } => unreachable!("handled above"),

        Command::Rules => {
            let plain: Vec<Rule> = rules.into_iter().map(|c| c.rule).collect();
            emit(&render::rules_list(&plain));
            println!(
                "\n  {} rules. Run 'vg explain <ID>' for detection patterns.",
                plain.len()
            );
            Ok(ExitCode::SUCCESS)
        }

        Command::Explain { id } => {
            let want = id.to_uppercase();
            match rules.iter().find(|c| c.rule.id.to_uppercase() == want) {
                Some(c) => {
                    emit(&render::explain(&c.rule));
                    Ok(ExitCode::SUCCESS)
                }
                None => Err(format!("no rule {id:?}. Run 'vg rules' for the catalog.")),
            }
        }

        Command::Bench {
            rejected,
            accepted,
            labels,
            json,
        } => {
            let rejected_findings = read_findings_dir(&rejected)?;
            let accepted_findings = match &accepted {
                Some(d) => read_findings_dir(d)?,
                None => Vec::new(),
            };
            let report = bench::run(
                &rules,
                &rejected_findings,
                &accepted_findings,
                labels.as_deref(),
            )?;
            if json {
                println!("{}", bench::to_json(&report));
            } else {
                emit(&bench::to_text(&report));
            }
            Ok(ExitCode::SUCCESS)
        }

        Command::Gates {
            finding,
            json,
            verbose,
            target,
        } => {
            let target = Target::read(&target)?;
            let finding_text = match &finding {
                Some(path) => Some(read_text(&path.display().to_string())?),
                None => None,
            };
            let outcomes = engine::evaluate_gates(&gates, &target.inputs(finding_text.as_deref()));
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&outcomes)
                        .map_err(|e| format!("cannot serialise the gates: {e}"))?
                );
            } else {
                println!("pre-hunt gates\n");
                emit(&render::gates_text(&outcomes, verbose));
                let unknown = outcomes
                    .iter()
                    .filter(|o| o.status == GateStatus::Unknown)
                    .count();
                if unknown > 0 {
                    println!(
                        "\n  {unknown} gate(s) could not run. Unknown is not pass: \
                         a gate you could not run is a gate you have not run."
                    );
                }
            }
            let failed = outcomes.iter().any(|o| o.status == GateStatus::Fail);
            Ok(if failed {
                ExitCode::from(1)
            } else {
                ExitCode::SUCCESS
            })
        }

        Command::Check {
            findings,
            json,
            min_score,
            table,
            fail_on_kill,
            target,
        } => {
            if findings.iter().filter(|f| f.as_str() == "-").count() > 1 {
                return Err("stdin (-) can be read only once".into());
            }
            let target = Target::read(&target)?;
            let mut reports = Vec::new();
            for finding in &findings {
                let text = read_text(finding)?;
                if text.trim().is_empty() {
                    return Err(format!("{finding} is empty"));
                }
                reports.push(build_report(
                    finding.clone(),
                    &text,
                    &rules,
                    &gates,
                    min_score,
                    &target,
                ));
            }

            if json {
                let out = if reports.len() == 1 {
                    serde_json::to_string_pretty(&reports[0])
                } else {
                    serde_json::to_string_pretty(&reports)
                };
                println!(
                    "{}",
                    out.map_err(|e| format!("cannot serialise the report: {e}"))?
                );
            } else {
                for (i, report) in reports.iter().enumerate() {
                    if i > 0 {
                        println!();
                    }
                    if table {
                        emit(&render::table(report));
                    } else {
                        emit(&render::text(report));
                    }
                }
            }

            let any_kill = reports.iter().any(|r| r.outcome == Outcome::Kill);
            Ok(if fail_on_kill && any_kill {
                ExitCode::from(1)
            } else {
                ExitCode::SUCCESS
            })
        }
    }
}

/// Read every markdown file under a directory, keyed by its path relative to
/// that directory and sorted, so the benchmark is deterministic.
fn read_findings_dir(dir: &Path) -> Result<Vec<(String, String)>, String> {
    let mut out: Vec<(String, String)> = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let rd = std::fs::read_dir(&d).map_err(|e| format!("cannot read {}: {e}", d.display()))?;
        for entry in rd.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().and_then(|e| e.to_str()) == Some("md") {
                let name = path
                    .strip_prefix(dir)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .replace('\\', "/");
                out.push((name, read_file(&path)?));
            }
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

fn log_path(file: &Option<PathBuf>) -> PathBuf {
    file.clone().unwrap_or_else(|| PathBuf::from(DEFAULT_LOG))
}

fn run_log(action: &LogAction, rules_dir: Option<&Path>) -> Result<ExitCode, String> {
    match action {
        LogAction::Add {
            family,
            outcome,
            note,
            date,
            file,
        } => {
            let path = log_path(file);
            init::log_add(&path, family, outcome, note.as_deref(), date.as_deref())?;
            emit(&format!("recorded {family:?} in {}\n", path.display()));
            println!(
                "\n  This file is LOCAL and gitignored. It contains your own bug \
                 families and must never be published."
            );
            Ok(ExitCode::SUCCESS)
        }
        LogAction::Check { finding, file } => {
            let path = log_path(file);
            let (_rules, gates) = load_rules(rules_dir)?;
            let text = read_text(&finding.display().to_string())?;
            let history = read_log(&Some(path))?;
            let inputs = GateInputs {
                own_log: history.as_deref(),
                finding: Some(&text),
                ..Default::default()
            };
            let outcomes = engine::evaluate_gates(&gates, &inputs);
            match outcomes.iter().find(|o| o.gate == "G7") {
                Some(o) if o.status == GateStatus::Fail => {
                    emit(&format!("REPEAT  {}\n", o.detail));
                    Ok(ExitCode::from(1))
                }
                Some(o) => {
                    emit(&format!("OK      {}\n", o.detail));
                    Ok(ExitCode::SUCCESS)
                }
                None => Err("no G7 gate is defined in the catalog".into()),
            }
        }
    }
}
