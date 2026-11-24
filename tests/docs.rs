//! The README example is generated, never hand-copied.
//!
//! This test runs the committed fixture through the real binary, builds the
//! exact block, and asserts that README.md carries it byte for byte. If the
//! block is stale the test prints the block it would write; regenerate with
//! scripts/regen-readme-examples.sh (or copy the printed block).

use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;

const BEGIN: &str = "<!-- BEGIN GENERATED EXAMPLE (scripts/regen-readme-examples.sh) -->";
const END: &str = "<!-- END GENERATED EXAMPLE -->";
const HTML_BEGIN: &str = "<!-- BEGIN GENERATED HTML EXAMPLE (scripts/regen-readme-examples.sh) -->";
const HTML_END: &str = "<!-- END GENERATED HTML EXAMPLE -->";

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn run(args: &[&str]) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_priorart"))
        .args(args)
        .current_dir(manifest_dir())
        .output()
        .expect("run priorart");
    assert!(
        output.status.success(),
        "priorart {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    // Normalise Windows path separators so the block is identical on every
    // platform a contributor runs this on.
    String::from_utf8_lossy(&output.stdout)
        .replace('\\', "/")
        .replace("\r\n", "\n")
        .trim_end()
        .to_string()
}

/// Serialises the tests that build the committed fixture in place: they write
/// the same dossier and would otherwise race on the atomic rename, failing one
/// of them with "cannot rename into ... (os error 2)".
static FIXTURE_LOCK: Mutex<()> = Mutex::new(());

/// The generated block, without the marker lines.
pub fn example_block() -> String {
    let _guard = FIXTURE_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let build = run(&["build", "examples/dossier-demo"]);
    let check = run(&[
        "check",
        "examples/finding-demo.md",
        "--dossier",
        "examples/dossier-demo/dossier-demo.dossier",
    ]);
    format!(
        "$ priorart build examples/dossier-demo\n{build}\n\n$ priorart check examples/finding-demo.md --dossier examples/dossier-demo/dossier-demo.dossier\n{check}"
    )
}

/// The generated HTML example, from the committed HTML fixture.
pub fn html_example_block() -> String {
    let _guard = FIXTURE_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let build = run(&["build", "tests/fixtures/html-program"]);
    let check = run(&[
        "check",
        "tests/fixtures/html-finding.md",
        "--dossier",
        "tests/fixtures/html-program/html-program.dossier",
    ]);
    format!(
        "$ priorart build tests/fixtures/html-program\n{build}\n\n$ priorart check tests/fixtures/html-finding.md --dossier tests/fixtures/html-program/html-program.dossier\n{check}"
    )
}

fn generated_section(readme: &str, begin: &str, end: &str, what: &str) -> String {
    let start = readme
        .find(begin)
        .unwrap_or_else(|| panic!("README is missing the {what} begin marker"));
    let after_begin = start + begin.len();
    let stop = readme[after_begin..]
        .find(end)
        .unwrap_or_else(|| panic!("README is missing the {what} end marker"))
        + after_begin;
    readme[after_begin..stop].trim_matches('\n').to_string()
}

#[test]
fn readme_example_is_generated_from_the_fixture() {
    let readme = std::fs::read_to_string(manifest_dir().join("README.md")).expect("README.md");
    let actual = generated_section(&readme, BEGIN, END, "example");
    let expected = example_block();
    assert_eq!(
        actual.trim_end(),
        expected.trim_end(),
        "README example is stale. Regenerate it with scripts/regen-readme-examples.sh.\n\n--- expected ---\n{expected}\n--- end ---\n"
    );
}

#[test]
fn html_readme_example_is_generated_from_the_fixture() {
    let readme = std::fs::read_to_string(manifest_dir().join("README.md")).expect("README.md");
    let actual = generated_section(&readme, HTML_BEGIN, HTML_END, "HTML example");
    let expected = html_example_block();
    assert_eq!(
        actual.trim_end(),
        expected.trim_end(),
        "README HTML example is stale. Regenerate it with scripts/regen-readme-examples.sh.\n\n--- expected ---\n{expected}\n--- end ---\n"
    );
}

#[test]
fn the_generated_block_is_deterministic() {
    assert_eq!(example_block(), example_block());
    assert_eq!(html_example_block(), html_example_block());
}

/// The published benchmark numbers live in two results files and are quoted in
/// docs/benchmark.md and README.md. This pins the four rates and the two
/// known-prior counts to the committed results, so a re-measurement cannot
/// update one page without the other.
#[test]
fn benchmark_docs_quote_the_measured_numbers() {
    let root = manifest_dir();
    let read = |path: &str| std::fs::read_to_string(root.join(path)).expect(path);
    let docs = read("docs/benchmark.md");
    let readme = read("README.md");

    let load = |path: &str| -> serde_json::Value {
        serde_json::from_str(&read(path)).expect("results.json is valid JSON")
    };
    let tapioca = load("eval/benchmark/results.json");
    let pooltogether = load("eval/benchmark/pooltogether/results.json");

    let cases = [
        (
            &tapioca,
            "false_alarm_rate",
            "alarms",
            60,
            15,
            "15/60 = 25.0%",
        ),
        (
            &tapioca,
            "duplicate_recall",
            "hits",
            64,
            36,
            "36/64 = 56.3%",
        ),
        (
            &pooltogether,
            "false_alarm_rate",
            "alarms",
            40,
            9,
            "9/40 = 22.5%",
        ),
        (
            &pooltogether,
            "duplicate_recall",
            "hits",
            85,
            32,
            "32/85 = 37.6%",
        ),
    ];
    for (doc, measure, key, n, k, needle) in cases {
        let row = &doc["measurements"][measure];
        assert_eq!(row["n"].as_u64(), Some(n), "{measure}: n drifted");
        assert_eq!(row[key].as_u64(), Some(k), "{measure}: {key} drifted");
        assert!(
            docs.contains(needle),
            "docs/benchmark.md is missing {needle}; update it with the results"
        );
        assert!(
            readme.contains(needle),
            "README.md is missing {needle}; update it with the results"
        );
    }

    for (doc, n) in [(&tapioca, 1), (&pooltogether, 2)] {
        let row = &doc["measurements"]["known_prior_recall"];
        assert_eq!(row["n"].as_u64(), Some(n), "known_prior_recall: n drifted");
        assert_eq!(
            row["hits"].as_u64(),
            Some(0),
            "known_prior_recall: hits drifted"
        );
    }
    for needle in ["n=1, 0 hits", "n=2, 0 hits"] {
        assert!(
            docs.contains(needle) && readme.contains(needle),
            "the known-prior count {needle} is missing from the docs"
        );
    }
}
