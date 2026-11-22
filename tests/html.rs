//! HTML program pages: the committed fixture is converted to text, and the
//! three gates answer from the converted page.
//!
//! The fixture is `tests/fixtures/html-program`: a saved page whose rewards,
//! assets and exclusions are HTML lists, plus script and style bodies that must
//! never reach an item. The finding beside it is fictional.

#[allow(dead_code)]
mod support;

use priorart::dossier::import::read_program_dir;
use serde_json::Value;
use support::{manifest_dir, run, TempDir};

fn fixture_dir() -> std::path::PathBuf {
    manifest_dir().join("tests/fixtures/html-program")
}

fn fixture_finding() -> std::path::PathBuf {
    manifest_dir().join("tests/fixtures/html-finding.md")
}

#[test]
fn html_program_page_yields_payout_scope_and_exclusion_items() {
    let imported = read_program_dir(&fixture_dir(), None).expect("the HTML fixture builds");
    let source = imported
        .sources
        .iter()
        .find(|s| s.kind == "program-page")
        .expect("a program page source");
    assert_eq!(source.path, "scope.html", "the saved page is the source");
    assert_eq!(source.title.as_deref(), Some("scope.html"));

    let kinds: Vec<&str> = imported.items.iter().map(|i| i.kind.as_str()).collect();
    assert_eq!(
        kinds.iter().filter(|k| **k == "payout-tier").count(),
        3,
        "three reward bullets: {kinds:?}"
    );
    assert_eq!(
        kinds.iter().filter(|k| **k == "scope-path").count(),
        2,
        "two assets bullets: {kinds:?}"
    );
    assert_eq!(
        kinds.iter().filter(|k| **k == "scope-clause").count(),
        2,
        "two exclusion bullets: {kinds:?}"
    );

    let all_text = imported
        .items
        .iter()
        .map(|i| i.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        !all_text.contains('<') && !all_text.contains('>'),
        "markup reached an item: {all_text}"
    );
    assert!(
        !all_text.contains("analytics") && !all_text.contains("font-family"),
        "script or style content reached an item: {all_text}"
    );
    assert!(
        all_text.contains("privileged roles"),
        "the &nbsp; entity was not decoded into the clause: {all_text}"
    );

    let payout = imported.program.payout.expect("program.yaml payout block");
    assert_eq!(
        payout.quote.as_deref(),
        Some("Medium: 2,000 USD"),
        "the reward quote must be read from the converted page"
    );
    assert_eq!(payout.section.as_deref(), Some("Rewards"));
}

#[test]
fn html_program_page_builds_and_answers_all_three_gates() {
    let tmp = TempDir::new("html-gates");
    let out = tmp.join("html-program.dossier");
    let build = run(&[
        "build",
        fixture_dir().to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
    ]);
    assert_eq!(
        build.code, 0,
        "HTML fixture build failed: {} {}",
        build.stdout, build.stderr
    );
    let combined = format!("{}{}", build.stdout, build.stderr);
    assert!(combined.contains("scope.html"), "{combined}");
    assert!(!combined.contains("analytics"), "{combined}");

    let check = run(&[
        "check",
        fixture_finding().to_str().unwrap(),
        "--dossier",
        out.to_str().unwrap(),
    ]);
    assert_eq!(check.code, 0, "HTML fixture check failed: {}", check.stderr);
    for expected in [
        "High is payable",          // G1 read the rewards table
        "privileged_role",          // G2 read the exclusion clause
        "Vault (in src/Vault.sol)", // G3 read the assets list
        "Medium: 2,000 USD",        // the payout quote came from the page
    ] {
        assert!(
            check.stdout.contains(expected),
            "expected {expected:?} in the check output:\n{}",
            check.stdout
        );
    }
    assert!(!check.stdout.contains("analytics"), "{}", check.stdout);
}

#[test]
fn a_scope_htm_only_folder_builds() {
    let tmp = TempDir::new("html-htm");
    tmp.write(
        "program/scope.htm",
        "<h2>Rewards</h2><ul><li>High: 10,000 USD</li></ul>\
<h2>In scope</h2><ul><li>src/Vault.sol</li></ul>\n",
    );
    let build = run(&["build", tmp.join("program").to_str().unwrap(), "--json"]);
    assert_eq!(build.code, 0, "{}", build.stderr);
    let dossier: Value = serde_json::from_str(&build.stdout).expect("build --json prints JSON");
    let source = dossier["sources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["kind"] == "program-page")
        .expect("a program-page source");
    assert_eq!(source["path"], "scope.htm");
    let kinds: Vec<&str> = dossier["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["kind"].as_str().unwrap())
        .collect();
    assert!(kinds.contains(&"payout-tier"), "{kinds:?}");
    assert!(kinds.contains(&"scope-path"), "{kinds:?}");
}

/// A table-based page is a known 0.2.0 limitation, not a promise. This test
/// pins the limitation so it cannot be mistaken for support later: no
/// payout/scope/exclusion items are extracted from a table, and the gates say
/// what they could not read.
#[test]
fn table_based_pages_are_a_known_limitation() {
    let tmp = TempDir::new("html-table");
    tmp.write(
        "program/scope.html",
        "<!doctype html><html><body><h1>Program</h1>\
<table><tr><td>Rewards</td></tr><tr><td>Critical: 50,000 USD</td></tr></table>\
<table><tr><td>Assets in scope</td></tr><tr><td>src/Vault.sol</td></tr></table>\
<table><tr><td>Out of scope</td></tr><tr><td>Attacks requiring a privileged role</td></tr></table>\
</body></html>\n",
    );
    let out = tmp.join("table.dossier");
    let build = run(&[
        "build",
        tmp.join("program").to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(build.code, 0, "{}", build.stderr);
    let dossier: Value = serde_json::from_str(&build.stdout).expect("build --json prints JSON");
    let kinds: Vec<&str> = dossier["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["kind"].as_str().unwrap())
        .collect();
    for unsupported in ["payout-tier", "scope-path", "scope-clause"] {
        assert!(
            !kinds.contains(&unsupported),
            "table mapping is a P1 plan item, not 0.2.0; {unsupported} appeared: {kinds:?}"
        );
    }
    assert!(kinds.contains(&"page-section"), "{kinds:?}");

    let finding = tmp.write(
        "finding.md",
        "# Finding\n\n`Vault.close` needs an operator.\nSeverity: High\n",
    );
    let check = run(&[
        "check",
        finding.to_str().unwrap(),
        "--dossier",
        out.to_str().unwrap(),
    ]);
    assert_eq!(check.code, 0, "{}", check.stderr);
    assert!(
        check.stdout.contains("no in-scope file list was indexed"),
        "G3 must report that the table scope was not read:\n{}",
        check.stdout
    );
    assert!(
        check.stdout.contains("no payout block"),
        "G1 must report that no payout table was read:\n{}",
        check.stdout
    );
}
