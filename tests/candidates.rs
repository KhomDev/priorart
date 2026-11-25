//! Black-box tests for the `candidates` command.
//!
//! The fixtures are `tests/fixtures/candidates-repo` (a six-function Solidity
//! contract whose call graph is `entry -> middle -> leaf`) and
//! `tests/fixtures/candidates/` (findings and one-item corpora). The feature
//! numbers here are the contract the JSON consumer sees; the unit tests in
//! `src/candidates/tests.rs` check them against hand-built anchors.

#[allow(dead_code)]
mod support;

use serde_json::Value;
use support::{manifest_dir, run, TempDir};

fn path(rel: &str) -> String {
    manifest_dir().join(rel).to_string_lossy().to_string()
}

fn repo() -> String {
    path("tests/fixtures/candidates-repo")
}

fn corpus(name: &str) -> String {
    path(&format!("tests/fixtures/candidates/{name}"))
}

fn finding(name: &str) -> String {
    path(&format!("tests/fixtures/candidates/findings/{name}"))
}

fn candidate_report(name: &str, corpus_dir: &str) -> Value {
    let r = run(&[
        "candidates",
        &finding(name),
        "--corpus",
        &corpus(corpus_dir),
        "--repo",
        &repo(),
        "--repo-id",
        "example/candidates-fixture",
        "--commit",
        "0000000000000000000000000000000000000000",
        "--json",
    ]);
    assert_eq!(r.code, 0, "{}", r.stderr);
    serde_json::from_str(&r.stdout).expect("candidates --json prints JSON")
}

#[test]
fn candidates_json_names_the_schema_and_every_feature() {
    let report = candidate_report("finding-shared-function.md", "corpus");
    assert_eq!(report["schema"], "priorart.candidates/v1");
    assert_eq!(
        report["finding"]["path"],
        finding("finding-shared-function.md").replace('\\', "/")
    );
    assert_eq!(report["corpus"]["kind"], "directory");
    assert_eq!(report["corpus"]["items"], 6);
    assert_eq!(report["index"]["supplied"], true);
    assert_eq!(
        report["index"]["repositories"][0],
        "example/candidates-fixture@0000000000000000000000000000000000000000"
    );
    assert_eq!(report["abstention"], Value::Null);
    assert_eq!(report["anchors"]["functions"], 1);
    assert_eq!(report["anchors"]["state_variables"], 2);
    assert_eq!(
        report["mechanism"]["root_causes"].as_array().unwrap().len(),
        0
    );
    assert_eq!(report["ordering"].as_array().unwrap().len(), 5);
    assert_eq!(report["limits"]["top_k"], 20);
    assert_eq!(report["limits"]["max_candidate_distance"], 2);

    let candidates = report["candidates"].as_array().expect("candidates");
    let ids: Vec<&str> = candidates
        .iter()
        .map(|c| c["prior_id"].as_str().unwrap())
        .collect();
    assert_eq!(
        ids,
        vec!["prior-deposit", "prior-leaf", "prior-total-assets"]
    );

    let first = &candidates[0];
    let features = &first["features"];
    assert_eq!(
        features["anchor_function_overlap"]["shared"],
        serde_json::json!(["Ledger.deposit"])
    );
    assert_eq!(features["anchor_function_overlap"]["finding"], 1);
    assert_eq!(features["anchor_function_overlap"]["prior"], 1);
    assert_eq!(features["anchor_function_overlap"]["ratio"], 1.0);
    assert_eq!(
        features["anchor_state_overlap"]["shared"],
        serde_json::json!(["Ledger.balances", "Ledger.totalAssets"])
    );
    assert_eq!(features["anchor_state_overlap"]["ratio"], 1.0);
    assert_eq!(features["call_graph_distance"], 0);
    assert_eq!(
        features["mechanism_match"]["shared_root_causes"],
        serde_json::json!([])
    );
    assert_eq!(features["mechanism_match"]["same_contract"], true);
    assert_eq!(
        features["impact_match"]["shared_impacts"],
        serde_json::json!([])
    );
    assert_eq!(features["identifier_score"]["ranked"], true);
    assert_eq!(features["identifier_score"]["specific_symbols"], 2);

    let rules: Vec<&str> = first["reasons"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["rule"].as_str().unwrap())
        .collect();
    assert_eq!(rules, vec!["shared-function", "shared-state"]);
}

#[test]
fn a_distance_two_neighbour_is_reported_with_its_distance() {
    let report = candidate_report("finding-distance.md", "corpus");
    let candidates = report["candidates"].as_array().expect("candidates");
    let ids: Vec<&str> = candidates
        .iter()
        .map(|c| c["prior_id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, vec!["prior-unrelated", "prior-leaf"]);
    assert_eq!(candidates[0]["features"]["call_graph_distance"], 1);
    assert_eq!(candidates[1]["features"]["call_graph_distance"], 2);
    assert_eq!(candidates[1]["reasons"][0]["rule"], "call-graph");
    assert_eq!(candidates[1]["reasons"][0]["distance"], 2);
}

#[test]
fn a_mechanism_match_in_the_same_contract_is_reported() {
    let report = candidate_report("finding-mechanism.md", "corpus");
    let candidates = report["candidates"].as_array().expect("candidates");
    assert_eq!(candidates.len(), 1, "{report}");
    let candidate = &candidates[0];
    assert_eq!(candidate["prior_id"], "prior-mechanism");
    assert_eq!(candidate["reasons"][0]["rule"], "mechanism-contract");
    assert_eq!(
        candidate["reasons"][0]["root_causes"],
        serde_json::json!(["rounding-precision"])
    );
    assert_eq!(
        candidate["reasons"][0]["contracts"],
        serde_json::json!(["Ledger"])
    );
    assert_eq!(
        candidate["features"]["mechanism_match"]["shared_root_causes"],
        serde_json::json!(["rounding-precision"])
    );
}

#[test]
fn shared_vocabulary_alone_leaves_the_report_abstained() {
    let report = candidate_report("finding-vocabulary.md", "corpus-vocabulary");
    assert_eq!(report["candidates"].as_array().unwrap().len(), 0);
    assert_eq!(report["abstention"]["reason"], "no-mechanism");
}

#[test]
fn the_text_output_names_an_abstention() {
    let r = run(&[
        "candidates",
        &finding("finding-no-code.md"),
        "--corpus",
        &corpus("corpus"),
        "--repo",
        &repo(),
    ]);
    assert_eq!(r.code, 0, "{}", r.stderr);
    assert!(r.stdout.contains("ABSTAIN no-anchor"), "{}", r.stdout);
    assert!(r.stdout.contains("candidates: 0"), "{}", r.stdout);
}

#[test]
fn a_json_run_is_byte_identical_twice() {
    let args = [
        "candidates",
        &finding("finding-shared-function.md"),
        "--corpus",
        &corpus("corpus"),
        "--repo",
        &repo(),
        "--repo-id",
        "example/candidates-fixture",
        "--commit",
        "0000000000000000000000000000000000000000",
        "--json",
    ];
    let first = run(&args);
    let second = run(&args);
    assert_eq!(first.code, 0, "{}", first.stderr);
    assert_eq!(second.code, 0, "{}", second.stderr);
    assert_eq!(first.stdout.as_bytes(), second.stdout.as_bytes());
}

#[test]
fn prepare_then_run_produces_the_same_candidates() {
    let tmp = TempDir::new("candidates-prepare");
    let out = tmp.join("prepared.json");
    let prepared = run(&[
        "candidates",
        "--corpus",
        &corpus("corpus"),
        "--repo",
        &repo(),
        "--repo-id",
        "example/candidates-fixture",
        "--commit",
        "0000000000000000000000000000000000000000",
        "--prepare",
        out.to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(prepared.code, 0, "{}", prepared.stderr);
    let summary: Value = serde_json::from_str(&prepared.stdout).expect("prepare JSON");
    assert_eq!(summary["schema"], "priorart.candidates-prepare/v1");
    assert_eq!(summary["items"], 6);
    assert_eq!(summary["anchors"], 6);
    let stored: Value =
        serde_json::from_str(&std::fs::read_to_string(&out).unwrap()).expect("prepared corpus");
    assert_eq!(stored["schema"], "priorart.candidates-corpus/v1");
    assert_eq!(stored["items"].as_array().unwrap().len(), 6);
    assert!(stored["items"][0]["anchors"].is_array());

    let direct = run(&[
        "candidates",
        &finding("finding-shared-function.md"),
        "--corpus",
        &corpus("corpus"),
        "--repo",
        &repo(),
        "--repo-id",
        "example/candidates-fixture",
        "--commit",
        "0000000000000000000000000000000000000000",
        "--json",
    ]);
    let cached = run(&[
        "candidates",
        &finding("finding-shared-function.md"),
        "--corpus",
        out.to_str().unwrap(),
        "--repo",
        &repo(),
        "--repo-id",
        "example/candidates-fixture",
        "--commit",
        "0000000000000000000000000000000000000000",
        "--json",
    ]);
    assert_eq!(direct.code, 0, "{}", direct.stderr);
    assert_eq!(cached.code, 0, "{}", cached.stderr);
    let direct: Value = serde_json::from_str(&direct.stdout).expect("direct");
    let cached: Value = serde_json::from_str(&cached.stdout).expect("cached");
    assert_eq!(
        serde_json::to_vec(&direct["candidates"]).unwrap(),
        serde_json::to_vec(&cached["candidates"]).unwrap()
    );
    assert_eq!(cached["corpus"]["prepared"], true);
    assert_eq!(cached["corpus"]["kind"], "prepared");
}

#[test]
fn usage_errors_are_exit_two_and_name_the_reason() {
    // No corpus.
    let r = run(&["candidates", &finding("finding-shared-function.md")]);
    assert_eq!(r.code, 2, "{}", r.stdout);
    assert!(r.stderr.contains("--corpus"), "{}", r.stderr);

    // --prepare with a finding.
    let tmp = TempDir::new("candidates-usage");
    let out = tmp.join("prepared.json");
    let r = run(&[
        "candidates",
        &finding("finding-shared-function.md"),
        "--corpus",
        &corpus("corpus"),
        "--prepare",
        out.to_str().unwrap(),
    ]);
    assert_eq!(r.code, 2, "{}", r.stdout);
    assert!(r.stderr.contains("--prepare"), "{}", r.stderr);

    // --top-k zero.
    let r = run(&[
        "candidates",
        &finding("finding-shared-function.md"),
        "--corpus",
        &corpus("corpus"),
        "--top-k",
        "0",
    ]);
    assert_eq!(r.code, 2, "{}", r.stdout);
    assert!(r.stderr.contains("--top-k"), "{}", r.stderr);

    // --repo-id with no --repo.
    let r = run(&[
        "candidates",
        &finding("finding-shared-function.md"),
        "--corpus",
        &corpus("corpus"),
        "--repo-id",
        "example/candidates-fixture",
    ]);
    assert_eq!(r.code, 2, "{}", r.stdout);
    assert!(r.stderr.contains("--repo-id"), "{}", r.stderr);
}

#[test]
fn a_prepared_corpus_with_duplicate_ids_is_refused() {
    let tmp = TempDir::new("candidates-duplicate-ids");
    let path = tmp.write(
        "dupes.json",
        r#"{
  "schema": "priorart.candidates-corpus/v1",
  "tool_version": "test",
  "repositories": [],
  "items": [
    {"id": "same", "title": null, "text": "`Ledger.deposit`", "anchors": [], "mechanism": null},
    {"id": "same", "title": null, "text": "`Ledger.leaf`", "anchors": [], "mechanism": null}
  ]
}
"#,
    );
    let r = run(&[
        "candidates",
        &finding("finding-shared-function.md"),
        "--corpus",
        path.to_str().unwrap(),
    ]);
    assert_eq!(r.code, 2, "{}", r.stdout);
    assert!(r.stderr.contains("more than once"), "{}", r.stderr);
}

#[test]
fn a_prepared_corpus_from_another_pin_is_a_usage_error() {
    let tmp = TempDir::new("candidates-mismatch");
    let out = tmp.join("prepared.json");
    let prepared = run(&[
        "candidates",
        "--corpus",
        &corpus("corpus"),
        "--repo",
        &repo(),
        "--repo-id",
        "example/candidates-fixture",
        "--prepare",
        out.to_str().unwrap(),
    ]);
    assert_eq!(prepared.code, 0, "{}", prepared.stderr);

    let r = run(&[
        "candidates",
        &finding("finding-shared-function.md"),
        "--corpus",
        out.to_str().unwrap(),
        "--repo",
        &repo(),
        "--repo-id",
        "someone-elses-id",
    ]);
    assert_eq!(r.code, 2, "{}", r.stdout);
    assert!(
        r.stderr.contains("prepared corpus was built against"),
        "{}",
        r.stderr
    );
}
