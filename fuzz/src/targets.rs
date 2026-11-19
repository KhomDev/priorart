//! The fuzz targets. Each takes arbitrary bytes and must not panic.
//!
//! The cargo-fuzz binaries in `fuzz_targets/` call these functions, and so
//! does `tests/fuzz_replay.rs`, which replays the committed seeds on every
//! `cargo test`. Keep this file free of libfuzzer and of anything that needs
//! nightly.
//!
//! The public API reads files, so the file-based targets write their input
//! into a scratch directory first. No target runs `pdftotext`: the PDF target
//! calls the in-process scrape only, so a fuzz run never spawns a subprocess.

use priorart::dossier::{import, load};
use priorart::model::Dossier;
use priorart::{check, ledger, matching, render};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::LazyLock;

const FINDING_FALLBACK: &str = "# Finding\n\nLoanBook.close lets any user close a loan.\n";

/// The committed demo dossier, parsed once. The finding target matches
/// against its real items, so the matcher and the gates see real input.
static DEMO: LazyLock<Dossier> = LazyLock::new(|| {
    serde_json::from_str(include_str!(
        "../../examples/dossier-demo/dossier-demo.dossier"
    ))
    .expect("the committed demo dossier parses")
});

/// A scratch directory under the system temp dir, removed on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        static N: AtomicUsize = AtomicUsize::new(0);
        let n = N.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("priorart-fuzz-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create scratch dir");
        Scratch(dir)
    }

    /// Write `bytes` at `rel` under the scratch directory and return the path.
    fn write(&self, rel: &str, bytes: &[u8]) -> PathBuf {
        let path = self.0.join(rel);
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(&path, bytes);
        path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Markdown and plain-text import: the program page, known issues and audits.
pub fn markdown_import(data: &[u8]) {
    let s = Scratch::new();
    s.write("scope.md", data);
    s.write("audits/review.md", data);
    let _ = import::read_program_dir(&s.0, None);
}

/// Uncompressed PDF: the in-process literal-string scrape.
pub fn pdf_scrape(data: &[u8]) {
    let _ = import::scrape_pdf_literals(data);
}

/// `program.yaml`, read by the import the way `priorart build` reads it.
pub fn program_yaml(data: &[u8]) {
    let s = Scratch::new();
    s.write("scope.md", b"## In scope\n\n- src/LoanBook.sol\n");
    s.write("program.yaml", data);
    let _ = import::read_program_dir(&s.0, None);
}

/// Known issues as YAML, read by the import.
pub fn known_issues_yaml(data: &[u8]) {
    let s = Scratch::new();
    s.write("scope.md", b"## In scope\n\n- src/LoanBook.sol\n");
    s.write("known-issues.yaml", data);
    let _ = import::read_program_dir(&s.0, None);
}

/// A private ledger, validated in strict mode.
pub fn ledger_yaml(data: &[u8]) {
    let s = Scratch::new();
    let path = s.write("ledger.yaml", data);
    let _ = ledger::validate(&[path], true, false);
}

/// A dossier file. If it loads, the gates and both renderers run over it.
pub fn dossier_json(data: &[u8]) {
    let s = Scratch::new();
    let path = s.write("x.dossier", data);
    if let Ok(dossier) = load::load(&path) {
        let report = check::build(
            Path::new("finding.md"),
            FINDING_FALLBACK,
            "0",
            &path,
            &dossier,
            matching::DEFAULT_TOP_K,
        );
        let _ = render::json::pretty(&report);
        let _ = render::text::check(&report, &dossier, FINDING_FALLBACK);
    }
}

/// A finding: the profile, the preconditions, and a full check against the
/// committed demo dossier. The text is read the way `check` reads a finding.
pub fn finding_profile(data: &[u8]) {
    let raw = String::from_utf8_lossy(data);
    let text = import::strip_bom(&raw).replace("\r\n", "\n");
    let _ = matching::Profile::from_text(&text).all_identifiers();
    let _ = matching::claimed_severity(&text);
    let _ = matching::mechanism_of(&text);
    let _ = priorart::gates::preconditions(&text).summary();
    let report = check::build(
        Path::new("finding.md"),
        &text,
        "0",
        Path::new("demo.dossier"),
        &DEMO,
        matching::DEFAULT_TOP_K,
    );
    let _ = render::text::check(&report, &DEMO, &text);
}
