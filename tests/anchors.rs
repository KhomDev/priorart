//! Black-box tests for the `anchors` command family.
//!
//! The fixture repository is `tests/fixtures/anchors-repo/`: four Solidity
//! files, one of them a deliberate syntax error, with inheritance, internal
//! calls and state writes.

#[allow(dead_code)]
mod support;

use serde_json::Value;
use support::{manifest_dir, run};

fn path(rel: &str) -> String {
    manifest_dir().join(rel).to_string_lossy().to_string()
}

fn repo() -> String {
    path("tests/fixtures/anchors-repo")
}

#[test]
fn index_json_names_the_repository_and_its_symbols() {
    let r = run(&[
        "anchors",
        "index",
        &repo(),
        "--repo-id",
        "example/anchor-fixture",
        "--commit",
        "0000000000000000000000000000000000000000",
        "--json",
    ]);
    assert_eq!(r.code, 0, "{}", r.stderr);
    let index: Value = serde_json::from_str(&r.stdout).expect("index JSON");
    assert_eq!(index["repo"], "example/anchor-fixture");
    assert_eq!(index["commit"], "0000000000000000000000000000000000000000");
    assert_eq!(index["files"].as_object().expect("files").len(), 4);
    assert_eq!(
        index["parse_failures"].as_array().expect("failures").len(),
        1
    );
    let functions = index["functions"].as_array().expect("functions");
    assert!(
        functions
            .iter()
            .any(|f| f["name"] == "deposit" && f["contract"] == "Vault"),
        "deposit is missing"
    );
    let graph = index["call_graph"].as_object().expect("call graph");
    assert_eq!(graph["Vault::compound"][0], "Base::baseIncrement");
    assert_eq!(graph["Vault::useHelper"][0], "Vault::_helper");
    let rw = &index["read_write"]["Vault::deposit"];
    assert_eq!(rw["reads"][0], "Vault.balances");
    assert_eq!(rw["writes"][1], "Vault.totalAssets");
}

#[test]
fn resolve_json_has_anchors_and_reasoned_abstentions() {
    let r = run(&[
        "anchors",
        "resolve",
        "--repo",
        &repo(),
        "--repo-id",
        "example/anchor-fixture",
        "--commit",
        "0000000000000000000000000000000000000000",
        "--text-file",
        &path("tests/fixtures/anchors-finding.md"),
        "--json",
    ]);
    assert_eq!(r.code, 0, "{}", r.stderr);
    let set: Value = serde_json::from_str(&r.stdout).expect("anchor JSON");
    assert_eq!(set["schema"], "priorart.anchors/v1");
    assert_eq!(set["cites_code"], true);
    let anchors = set["anchors"].as_array().expect("anchors");
    assert!(!anchors.is_empty(), "{set}");
    assert!(
        anchors.iter().any(|a| a["symbol"] == "Vault.deposit"),
        "deposit anchor missing: {set}"
    );
    assert!(
        anchors
            .iter()
            .any(|a| a["kind"] == "state-var" && a["symbol"] == "Vault.totalAssets"),
        "state-variable anchor missing: {set}"
    );
    assert!(
        anchors
            .iter()
            .all(|a| a["confidence"].as_u64().unwrap_or(0) > 0),
        "an anchor has no confidence: {set}"
    );
    let abstentions = set["abstentions"].as_array().expect("abstentions");
    assert!(
        abstentions.iter().any(|a| a["reason"] == "not-found"),
        "the missing symbol did not abstain: {set}"
    );
}

#[test]
fn prose_without_code_abstains_with_no_citation() {
    let r = run(&[
        "anchors",
        "resolve",
        "--repo",
        &repo(),
        "--text-file",
        &path("tests/fixtures/anchors-finding-no-code.md"),
        "--json",
    ]);
    assert_eq!(r.code, 0, "{}", r.stderr);
    let set: Value = serde_json::from_str(&r.stdout).expect("anchor JSON");
    assert_eq!(set["cites_code"], false);
    assert_eq!(set["anchors"].as_array().expect("anchors").len(), 0);
    assert_eq!(set["abstentions"][0]["reason"], "no-citation");
}

#[test]
fn resolution_is_byte_identical_on_a_second_run() {
    let args = [
        "anchors",
        "resolve",
        "--repo",
        &repo(),
        "--text-file",
        &path("tests/fixtures/anchors-finding.md"),
        "--json",
    ];
    let first = run(&args);
    let second = run(&args);
    assert_eq!(first.code, 0);
    assert_eq!(first.stdout.as_bytes(), second.stdout.as_bytes());
}

#[test]
fn bench_json_measures_the_tiny_corpus_and_blocks_a_missing_source() {
    let root = path("tests/fixtures/anchors-bench");
    let measured = run(&[
        "anchors",
        "bench",
        "--benchmark-root",
        &format!("{root}/benchmark"),
        "--cache-root",
        &format!("{root}/cache"),
        "--sources-root",
        &format!("{root}/sources"),
        "--corpus",
        "tiny",
        "--allow-unverified-commit",
        "--json",
    ]);
    assert_eq!(measured.code, 0, "{}", measured.stderr);
    let report: Value = serde_json::from_str(&measured.stdout).expect("bench JSON");
    assert_eq!(report["schema"], "priorart.anchors-bench/v1");
    let row = &report["corpora"][0];
    assert_eq!(row["status"], "measured");
    assert_eq!(row["findings_total"], 2);
    assert_eq!(row["findings_citing_code"], 1);
    assert_eq!(row["findings_resolved"], 1);
    assert_eq!(report["pooled"]["findings_resolved"], 1);

    let blocked = run(&[
        "anchors",
        "bench",
        "--benchmark-root",
        &format!("{root}/benchmark"),
        "--cache-root",
        &format!("{root}/cache"),
        "--sources-root",
        &format!("{root}/absent"),
        "--corpus",
        "tiny",
        "--allow-unverified-commit",
        "--json",
    ]);
    assert_eq!(blocked.code, 0, "{}", blocked.stderr);
    let report: Value = serde_json::from_str(&blocked.stdout).expect("blocked JSON");
    let row = &report["corpora"][0];
    assert_eq!(row["status"], "blocked");
    assert_eq!(row["findings_total"], 2);
    assert_eq!(row["findings_citing_code"], 1);
    assert!(row["findings_resolved"].is_null());
    assert!(report["pooled"].is_null());
    assert!(row["blocked_reason"]
        .as_str()
        .unwrap_or("")
        .starts_with("source-missing"));
}

#[test]
fn bench_text_output_is_a_table_with_the_denominator() {
    let root = path("tests/fixtures/anchors-bench");
    let r = run(&[
        "anchors",
        "bench",
        "--benchmark-root",
        &format!("{root}/benchmark"),
        "--cache-root",
        &format!("{root}/cache"),
        "--sources-root",
        &format!("{root}/sources"),
        "--corpus",
        "tiny",
        "--allow-unverified-commit",
    ]);
    assert_eq!(r.code, 0, "{}", r.stderr);
    assert!(r.stdout.contains("corpus"), "{}", r.stdout);
    assert!(r.stdout.contains("cited"), "{}", r.stdout);
    assert!(r.stdout.contains("tiny"), "{}", r.stdout);
    assert!(r.stdout.contains("pooled: 1/1"), "{}", r.stdout);
}

#[test]
fn anchors_resolve_requires_a_repository() {
    let r = run(&[
        "anchors",
        "resolve",
        "--text-file",
        &path("tests/fixtures/anchors-finding.md"),
    ]);
    assert_eq!(r.code, 2, "{}", r.stderr);
}
