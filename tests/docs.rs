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
