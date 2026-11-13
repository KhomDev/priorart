//! The README example is generated, never hand-copied.
//!
//! This test runs the committed fixture through the real binary, builds the
//! exact block, and asserts that README.md carries it byte for byte. If the
//! block is stale the test prints the block it would write; regenerate with
//! scripts/regen-readme-examples.sh (or copy the printed block).

use std::path::PathBuf;
use std::process::Command;

const BEGIN: &str = "<!-- BEGIN GENERATED EXAMPLE (scripts/regen-readme-examples.sh) -->";
const END: &str = "<!-- END GENERATED EXAMPLE -->";

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

/// The generated block, without the marker lines.
pub fn example_block() -> String {
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

#[test]
fn readme_example_is_generated_from_the_fixture() {
    let readme = std::fs::read_to_string(manifest_dir().join("README.md")).expect("README.md");
    let begin = readme
        .find(BEGIN)
        .expect("README is missing the begin marker");
    let after_begin = begin + BEGIN.len();
    let end = readme[after_begin..]
        .find(END)
        .expect("README is missing the end marker")
        + after_begin;
    let actual = readme[after_begin..end].trim_matches('\n');
    let expected = example_block();
    assert_eq!(
        actual.trim_end(),
        expected.trim_end(),
        "README example is stale. Regenerate it with scripts/regen-readme-examples.sh.\n\n--- expected ---\n{expected}\n--- end ---\n"
    );
}

#[test]
fn the_generated_block_is_deterministic() {
    assert_eq!(example_block(), example_block());
}
