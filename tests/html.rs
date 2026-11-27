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
        "High: 10,000 USD",         // the claimed severity's row, quoted from the page
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

/// A headerless table under a real heading becomes bullets, one row each.
/// A cell that is only a section title ("Rewards") is not itself a tier.
#[test]
fn a_table_under_a_heading_is_read_as_bullets() {
    let tmp = TempDir::new("html-table");
    tmp.write(
        "program/scope.html",
        "<!doctype html><html><body><h1>Program</h1>\
<h2>Rewards</h2>\
<table><tr><td>Critical</td><td>50,000 USD</td></tr></table>\
<h2>Assets in scope</h2>\
<table><tr><th>Target</th><th>Name</th></tr>\
<tr><td><a href=\"https://github.com/acme/vault\"></a></td><td></td></tr></table>\
<h2>Impacts in scope</h2>\
<table><tr><td>High</td><td>Email</td></tr></table>\
<h2>Out of scope</h2>\
<table><tr><td>Attacks requiring a privileged role</td></tr></table>\
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
    let items = dossier["items"].as_array().unwrap();
    let text_of = |kind: &str| -> Vec<String> {
        items
            .iter()
            .filter(|i| i["kind"] == kind)
            .map(|i| i["text"].as_str().unwrap().to_string())
            .collect()
    };
    let tiers = text_of("payout-tier");
    assert!(
        tiers
            .iter()
            .any(|t| t.contains("Critical") && t.contains("50,000 USD")),
        "{tiers:?}"
    );
    let paths = text_of("scope-path");
    assert!(
        paths
            .iter()
            .any(|t| t.contains("https://github.com/acme/vault")),
        "{paths:?}"
    );
    assert!(
        text_of("payout-tier").iter().all(|t| !t.contains("Email")),
        "the impacts row must not be a payout tier"
    );
    let impacts = text_of("impact");
    assert!(
        impacts
            .iter()
            .any(|t| t.contains("High") && t.contains("Email")),
        "{impacts:?}"
    );
    assert!(text_of("scope-clause")
        .iter()
        .any(|t| t.contains("privileged role")));
}

/// A cell whose text is itself a Markdown bullet is still one payout tier:
/// the row bullet wraps the cell text, and the inner "- " is part of that text.
#[test]
fn a_table_cell_that_is_itself_a_markdown_bullet_is_still_read() {
    let tmp = TempDir::new("html-table-bullet");
    tmp.write(
        "program/scope.html",
        "<!doctype html><html><body><h2>Rewards</h2>\
<table><tr><td>- Critical: 50,000 USD</td></tr></table></body></html>\n",
    );
    let build = run(&["build", tmp.join("program").to_str().unwrap(), "--json"]);
    assert_eq!(build.code, 0, "{}", build.stderr);
    let dossier: Value = serde_json::from_str(&build.stdout).expect("build --json prints JSON");
    let kinds: Vec<&str> = dossier["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["kind"].as_str().unwrap())
        .collect();
    assert!(
        kinds.contains(&"payout-tier"),
        "a cell that starts with '- ' is a bullet, so it is read: {kinds:?}"
    );
}

/// A page whose text exists only inside `<script>` yields no items and says so.
#[test]
fn a_javascript_only_page_yields_nothing_and_says_so() {
    let tmp = TempDir::new("html-js");
    tmp.write(
        "program/scope.html",
        "<html><body><script>document.write('<h2>Rewards</h2><li>Critical: 50,000 USD</li>')</script></body></html>\n",
    );
    let build = run(&["build", tmp.join("program").to_str().unwrap()]);
    assert_eq!(build.code, 0, "{}", build.stderr);
    let combined = format!("{}{}", build.stdout, build.stderr);
    assert!(combined.contains("no readable text"), "{combined}");
    assert!(combined.contains("JavaScript-rendered"), "{combined}");
    assert!(!combined.contains("50,000"), "{combined}");
    assert!(!combined.contains("payout-tier"), "{combined}");
}
