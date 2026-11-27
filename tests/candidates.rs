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

#[test]
fn a_repository_index_file_is_equivalent_to_a_checkout() {
    let tmp = TempDir::new("candidates-index");
    let index_file = tmp.join("index.json");
    let indexed = run(&[
        "anchors",
        "index",
        &repo(),
        "--repo-id",
        "example/candidates-fixture",
        "--commit",
        "0000000000000000000000000000000000000000",
        "--json",
    ]);
    assert_eq!(indexed.code, 0, "{}", indexed.stderr);
    std::fs::write(&index_file, indexed.stdout.as_bytes()).expect("write index");

    let direct = run(&[
        "candidates",
        &finding("finding-distance.md"),
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
    let from_file = run(&[
        "candidates",
        &finding("finding-distance.md"),
        "--corpus",
        &corpus("corpus"),
        "--index",
        index_file.to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(direct.code, 0, "{}", direct.stderr);
    assert_eq!(from_file.code, 0, "{}", from_file.stderr);
    let direct: Value = serde_json::from_str(&direct.stdout).expect("direct");
    let from_file: Value = serde_json::from_str(&from_file.stdout).expect("index");
    assert_eq!(
        serde_json::to_vec(&direct["candidates"]).unwrap(),
        serde_json::to_vec(&from_file["candidates"]).unwrap()
    );
    assert_eq!(
        from_file["index"]["repositories"][0],
        "example/candidates-fixture@0000000000000000000000000000000000000000"
    );
}

#[test]
fn default_output_is_unchanged_and_matches_the_pre_ranker_golden() {
    // The golden was captured from the pre-change binary (sha256 1e465f61...)
    // with the same relative arguments, so a default run must still be
    // byte-identical: the ranker score field is omitted when no --ranker was
    // supplied, and the ordering block is the original five-key order.
    let r = run(&[
        "candidates",
        "tests/fixtures/candidates/findings/finding-shared-function.md",
        "--corpus",
        "tests/fixtures/candidates/corpus",
        "--repo",
        "tests/fixtures/candidates-repo",
        "--repo-id",
        "example/candidates-fixture",
        "--commit",
        "0000000000000000000000000000000000000000",
        "--json",
    ]);
    assert_eq!(r.code, 0, "{}", r.stderr);
    let golden = std::fs::read_to_string(
        manifest_dir().join("tests/fixtures/candidates/golden/candidates-default.json"),
    )
    .expect("committed golden is readable");
    // The tool version is metadata, not candidate output; a later release bump
    // must not fail this test. Every other byte must match the golden.
    let without_version = |text: &str| {
        text.lines()
            .filter(|line| !line.contains("\"tool_version\""))
            .collect::<Vec<_>>()
            .join("\n")
    };
    assert_eq!(
        without_version(&r.stdout),
        without_version(&golden),
        "default output differs from the pre-ranker golden"
    );
    assert!(!r.stdout.contains("ranker_score"), "{}", r.stdout);
}

#[test]
fn the_committed_ranker_fixture_reproduces_exact_scores_and_order() {
    let r = run(&[
        "candidates",
        "tests/fixtures/candidates/findings/finding-shared-function.md",
        "--corpus",
        "tests/fixtures/candidates/corpus",
        "--repo",
        "tests/fixtures/candidates-repo",
        "--repo-id",
        "example/candidates-fixture",
        "--commit",
        "0000000000000000000000000000000000000000",
        "--ranker",
        "tests/fixtures/candidates/ranker/fixture-weights.json",
        "--json",
    ]);
    assert_eq!(r.code, 0, "{}", r.stderr);
    let report: Value = serde_json::from_str(&r.stdout).expect("candidates --json prints JSON");
    let candidates = report["candidates"].as_array().expect("candidates");
    let ids: Vec<&str> = candidates
        .iter()
        .map(|c| c["prior_id"].as_str().unwrap())
        .collect();
    // The fixture is built so a broken mean, scale or log1p changes the
    // score. Documented formula, with fn_shared mean 2.0 scale 0.5 coef -0.5,
    // call_graph_distance mean 1.0 scale 0.5 coef -0.25, missingness coef 3.0,
    // rarity log1p coef 0.5:
    //   prior-deposit: -0.5*(1-2)/0.5 + -0.25*(0-1)/0.5 + 0.5*ln(1+6)
    //                  = 1 + 0.5 + 0.5*ln7 = 2.4729550745276567
    //   prior-leaf and prior-total-assets: -0.5*(0-2)/0.5
    //                  + -0.25*(3-1)/0.5 + 3*1 = 2 - 1 + 3 = 4
    // The two tie on every feature; the tie-break is the previous
    // deterministic order, which puts prior-leaf first.
    assert_eq!(
        ids,
        vec!["prior-leaf", "prior-total-assets", "prior-deposit"]
    );
    let expected = [0.9820137900379085, 0.9820137900379085, 0.922223987854721];
    for (candidate, want) in candidates.iter().zip(expected) {
        let got = candidate["ranker_score"]
            .as_f64()
            .expect("a ranker run emits a score");
        assert!(
            (got - want).abs() < 1e-12,
            "{} scored {got}, expected {want}",
            candidate["prior_id"]
        );
    }
    assert_eq!(
        report["ordering"][0],
        "ranker score descending (priorart.ranker-weights/v1)"
    );
    assert_eq!(
        report["ordering"][1],
        "previous candidate order as tie-break"
    );
}

#[test]
fn a_null_call_graph_distance_is_scored_with_the_missingness_indicator() {
    // Prior-mechanism has no call-graph path from the finding, so its distance
    // is null. The documented rule maps it to 3.0 and sets the indicator to
    // 1.0; with the fixture coefficients the logit is
    //   -0.5*(0-2)/0.5 + -0.25*(3-1)/0.5 + 3*1 + 1*1 = 2 - 1 + 3 + 1 = 5,
    // so the score is sigmoid(5).
    let r = run(&[
        "candidates",
        "tests/fixtures/candidates/findings/finding-mechanism.md",
        "--corpus",
        "tests/fixtures/candidates/corpus",
        "--repo",
        "tests/fixtures/candidates-repo",
        "--repo-id",
        "example/candidates-fixture",
        "--commit",
        "0000000000000000000000000000000000000000",
        "--ranker",
        "tests/fixtures/candidates/ranker/fixture-weights.json",
        "--json",
    ]);
    assert_eq!(r.code, 0, "{}", r.stderr);
    let report: Value = serde_json::from_str(&r.stdout).expect("candidates --json prints JSON");
    let candidate = &report["candidates"][0];
    assert_eq!(candidate["prior_id"], "prior-mechanism");
    assert_eq!(candidate["features"]["call_graph_distance"], Value::Null);
    let got = candidate["ranker_score"].as_f64().expect("score");
    assert!(
        (got - 0.9933071490757153).abs() < 1e-12,
        "scored {got}, expected sigmoid(5) = 0.9933071490757153"
    );
}

#[test]
fn a_weights_table_missing_a_feature_is_a_usage_error() {
    let tmp = TempDir::new("ranker-missing-feature");
    let text = std::fs::read_to_string(
        manifest_dir().join("tests/fixtures/candidates/ranker/fixture-weights.json"),
    )
    .expect("fixture weights");
    let mut weights: Value = serde_json::from_str(&text).expect("weights JSON");
    weights["features"]
        .as_array_mut()
        .unwrap()
        .retain(|f| f["name"] != "identifier_score.rarity");
    let path = tmp.write("weights.json", &weights.to_string());
    let r = run(&[
        "candidates",
        "tests/fixtures/candidates/findings/finding-shared-function.md",
        "--corpus",
        "tests/fixtures/candidates/corpus",
        "--repo",
        "tests/fixtures/candidates-repo",
        "--repo-id",
        "example/candidates-fixture",
        "--commit",
        "0000000000000000000000000000000000000000",
        "--ranker",
        path.to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(r.code, 2, "{}", r.stdout);
    assert!(
        r.stderr.contains("identifier_score.rarity") && r.stderr.contains("missing"),
        "{}",
        r.stderr
    );
}

#[test]
fn a_nullable_feature_without_a_missing_value_is_a_usage_error() {
    let tmp = TempDir::new("ranker-missing-value");
    let text = std::fs::read_to_string(
        manifest_dir().join("tests/fixtures/candidates/ranker/fixture-weights.json"),
    )
    .expect("fixture weights");
    let mut weights: Value = serde_json::from_str(&text).expect("weights JSON");
    for feature in weights["features"].as_array_mut().unwrap() {
        if feature["name"] == "call_graph_distance" {
            feature.as_object_mut().unwrap().remove("missing_value");
        }
    }
    let path = tmp.write("weights.json", &weights.to_string());
    let r = run(&[
        "candidates",
        "tests/fixtures/candidates/findings/finding-shared-function.md",
        "--corpus",
        "tests/fixtures/candidates/corpus",
        "--repo",
        "tests/fixtures/candidates-repo",
        "--repo-id",
        "example/candidates-fixture",
        "--commit",
        "0000000000000000000000000000000000000000",
        "--ranker",
        path.to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(r.code, 2, "{}", r.stdout);
    assert!(r.stderr.contains("missing_value"), "{}", r.stderr);
}

#[test]
fn a_ranker_with_prepare_is_a_usage_error() {
    let tmp = TempDir::new("ranker-prepare");
    let out = tmp.join("prepared.json");
    let r = run(&[
        "candidates",
        "--corpus",
        "tests/fixtures/candidates/corpus",
        "--repo",
        "tests/fixtures/candidates-repo",
        "--repo-id",
        "example/candidates-fixture",
        "--prepare",
        out.to_str().unwrap(),
        "--ranker",
        "tests/fixtures/candidates/ranker/fixture-weights.json",
    ]);
    assert_eq!(r.code, 2, "{}", r.stdout);
    assert!(r.stderr.contains("--ranker"), "{}", r.stderr);
}

#[test]
fn the_committed_weights_table_loads_and_scores() {
    // The shipped table must be accepted by inference, not just by the
    // trainer: load it through the CLI on the fixture corpus and require a
    // finite score on every candidate.
    let r = run(&[
        "candidates",
        "tests/fixtures/candidates/findings/finding-shared-function.md",
        "--corpus",
        "tests/fixtures/candidates/corpus",
        "--repo",
        "tests/fixtures/candidates-repo",
        "--repo-id",
        "example/candidates-fixture",
        "--commit",
        "0000000000000000000000000000000000000000",
        "--ranker",
        "eval/dupbench/weights/v1.json",
        "--json",
    ]);
    assert_eq!(r.code, 0, "{}", r.stderr);
    let report: Value = serde_json::from_str(&r.stdout).expect("candidates --json prints JSON");
    let candidates = report["candidates"].as_array().expect("candidates");
    assert!(!candidates.is_empty(), "{report}");
    for candidate in candidates {
        let score = candidate["ranker_score"]
            .as_f64()
            .unwrap_or_else(|| panic!("missing score for {candidate}"));
        assert!(score.is_finite() && (0.0..=1.0).contains(&score), "{score}");
    }
}

/// The raw feature values of one candidate, following the documented rule
/// (the pairs in `priorart candidates --json` plus the derived
/// `call_graph_distance_missing` indicator). Used only to recompute an
/// expected score from the committed weights, independently of `Ranker`.
fn raw_feature(name: &str, features: &Value) -> f64 {
    match name {
        "anchor_function_overlap.shared" => features["anchor_function_overlap"]["shared"]
            .as_array()
            .unwrap()
            .len() as f64,
        "anchor_function_overlap.ratio" => features["anchor_function_overlap"]["ratio"]
            .as_f64()
            .unwrap_or(0.0),
        "anchor_state_overlap.shared" => features["anchor_state_overlap"]["shared"]
            .as_array()
            .unwrap()
            .len() as f64,
        "anchor_state_overlap.ratio" => features["anchor_state_overlap"]["ratio"]
            .as_f64()
            .unwrap_or(0.0),
        // The documented missingness rule: MAX_CANDIDATE_DISTANCE (2) + 1.
        "call_graph_distance" => features["call_graph_distance"].as_f64().unwrap_or(3.0),
        "call_graph_distance_missing" => {
            if features["call_graph_distance"].is_null() {
                1.0
            } else {
                0.0
            }
        }
        "mechanism_match.shared_root_causes" => features["mechanism_match"]["shared_root_causes"]
            .as_array()
            .unwrap()
            .len() as f64,
        "mechanism_match.same_contract" => {
            if features["mechanism_match"]["same_contract"]
                .as_bool()
                .unwrap()
            {
                1.0
            } else {
                0.0
            }
        }
        "impact_match.shared_impacts" => features["impact_match"]["shared_impacts"]
            .as_array()
            .unwrap()
            .len() as f64,
        "identifier_score.specific_symbols" => features["identifier_score"]["specific_symbols"]
            .as_u64()
            .unwrap() as f64,
        "identifier_score.shared_symbols" => features["identifier_score"]["shared_symbols"]
            .as_u64()
            .unwrap() as f64,
        "identifier_score.rarity" => {
            features["identifier_score"]["rarity"].as_u64().unwrap() as f64
        }
        "identifier_score.ranked" => {
            if features["identifier_score"]["ranked"].as_bool().unwrap() {
                1.0
            } else {
                0.0
            }
        }
        other => panic!("the test does not know the feature {other:?}"),
    }
}

fn expected_score_from_weights(weights: &Value, features: &Value) -> f64 {
    let mut z = weights["intercept"].as_f64().expect("intercept");
    for spec in weights["features"].as_array().expect("features") {
        let name = spec["name"].as_str().expect("feature name");
        let mut value = raw_feature(name, features);
        if spec["transform"].as_str().expect("transform") == "log1p" {
            value = value.ln_1p();
        }
        z += spec["coefficient"].as_f64().expect("coefficient")
            * (value - spec["mean"].as_f64().expect("mean"))
            / spec["scale"].as_f64().expect("scale");
    }
    1.0 / (1.0 + (-z).exp())
}

#[test]
fn the_committed_weights_table_reproduces_an_exact_score() {
    // The shipped table has non-unit scales and log1p transforms; a scorer
    // that dropped the scale, the mean, or the log1p would pass a
    // finite-score check but not this one. The expected value is computed
    // here from the recorded mean/scale/coefficient and the candidate's own
    // raw features, not read back from the program.
    let r = run(&[
        "candidates",
        "tests/fixtures/candidates/findings/finding-shared-function.md",
        "--corpus",
        "tests/fixtures/candidates/corpus",
        "--repo",
        "tests/fixtures/candidates-repo",
        "--repo-id",
        "example/candidates-fixture",
        "--commit",
        "0000000000000000000000000000000000000000",
        "--ranker",
        "eval/dupbench/weights/v1.json",
        "--json",
    ]);
    assert_eq!(r.code, 0, "{}", r.stderr);
    let report: Value = serde_json::from_str(&r.stdout).expect("candidates --json prints JSON");
    let candidate = report["candidates"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["prior_id"] == "prior-deposit")
        .expect("prior-deposit is a candidate");
    let weights: Value = serde_json::from_str(
        &std::fs::read_to_string(manifest_dir().join("eval/dupbench/weights/v1.json"))
            .expect("committed weights"),
    )
    .expect("committed weights are JSON");
    // The test is only as strong as the table it reads: require a feature
    // with a nonzero coefficient and a scale away from 1, and a log1p feature
    // that is nonzero for this candidate.
    let specs = weights["features"].as_array().expect("features");
    assert!(specs.iter().any(|spec| {
        spec["coefficient"].as_f64().unwrap() != 0.0
            && (spec["scale"].as_f64().unwrap() - 1.0).abs() > 1e-6
    }));
    assert!(specs.iter().any(|spec| {
        spec["transform"] == "log1p"
            && raw_feature(spec["name"].as_str().unwrap(), &candidate["features"]) > 0.0
    }));
    let expected = expected_score_from_weights(&weights, &candidate["features"]);
    let got = candidate["ranker_score"].as_f64().expect("score");
    assert!(
        (got - expected).abs() < 1e-12,
        "prior-deposit scored {got}, independently computed {expected}"
    );
}

// --- PA-B5 calibrated alarm ----------------------------------------------------

fn alarm_run(finding_name: &str, corpus_dir: &str, extra: &[&str]) -> support::Run {
    let finding_path = format!("tests/fixtures/candidates/findings/{finding_name}");
    let mut args = vec![
        "candidates",
        finding_path.as_str(),
        "--corpus",
        corpus_dir,
        "--repo",
        "tests/fixtures/candidates-repo",
        "--repo-id",
        "example/candidates-fixture",
        "--commit",
        "0000000000000000000000000000000000000000",
        "--ranker",
        "tests/fixtures/candidates/ranker/fixture-weights.json",
    ];
    args.extend_from_slice(extra);
    run(&args)
}

#[test]
fn the_alarm_fires_on_the_calibrated_value_and_not_on_the_raw_score() {
    // fixture-weights score the top candidate of finding-shared-function at
    // 0.9820137900379085; fixture-alarm maps [0.9, 0.985] to 0.5, below the
    // 0.97 threshold. A raw comparison (0.982 >= 0.97) would fire: this is
    // the skip-calibration break.
    let r = alarm_run(
        "finding-shared-function.md",
        "tests/fixtures/candidates/corpus",
        &[
            "--alarm",
            "tests/fixtures/candidates/ranker/fixture-alarm.json",
            "--json",
        ],
    );
    assert_eq!(r.code, 0, "{}", r.stderr);
    let report: Value = serde_json::from_str(&r.stdout).expect("JSON");
    let alarm = &report["alarm"];
    assert_eq!(alarm["schema"], "priorart.alarm-calibration/v1");
    assert_eq!(alarm["status"], "on");
    assert_eq!(alarm["top_candidate"], "prior-leaf");
    // serde_json's float parse may differ from the literal by one ULP.
    let close = |v: &Value, want: f64| (v.as_f64().expect("number") - want).abs() < 1e-12;
    assert!(close(&alarm["top_score"], 0.9820137900379085), "{alarm}");
    assert!(close(&alarm["calibrated"], 0.5), "{alarm}");
    assert_eq!(alarm["fires"], false);
    assert_eq!(alarm["warning"], Value::Null);

    // finding-mechanism's top candidate scores sigmoid(5) = 0.9933..., which
    // calibrates to exactly the threshold 0.97 and fires.
    let r = alarm_run(
        "finding-mechanism.md",
        "tests/fixtures/candidates/corpus",
        &[
            "--alarm",
            "tests/fixtures/candidates/ranker/fixture-alarm.json",
            "--json",
        ],
    );
    assert_eq!(r.code, 0, "{}", r.stderr);
    let report: Value = serde_json::from_str(&r.stdout).expect("JSON");
    assert!(
        close(&report["alarm"]["calibrated"], 0.97),
        "{}",
        report["alarm"]
    );
    assert_eq!(report["alarm"]["fires"], true);
    assert_eq!(report["alarm"]["warning"], "shares a root cause");

    let text = alarm_run(
        "finding-mechanism.md",
        "tests/fixtures/candidates/corpus",
        &[
            "--alarm",
            "tests/fixtures/candidates/ranker/fixture-alarm.json",
        ],
    );
    assert_eq!(text.code, 0, "{}", text.stderr);
    assert!(
        text.stdout
            .contains("WARNING shares a root cause: prior-mechanism"),
        "{}",
        text.stdout
    );
}

#[test]
fn the_alarm_is_absent_without_the_flag_and_requires_a_ranker() {
    let r = alarm_run(
        "finding-shared-function.md",
        "tests/fixtures/candidates/corpus",
        &["--json"],
    );
    assert_eq!(r.code, 0, "{}", r.stderr);
    assert!(!r.stdout.contains("\"alarm\""), "{}", r.stdout);

    let r = run(&[
        "candidates",
        "tests/fixtures/candidates/findings/finding-shared-function.md",
        "--corpus",
        "tests/fixtures/candidates/corpus",
        "--repo",
        "tests/fixtures/candidates-repo",
        "--repo-id",
        "example/candidates-fixture",
        "--commit",
        "0000000000000000000000000000000000000000",
        "--alarm",
        "tests/fixtures/candidates/ranker/fixture-alarm.json",
        "--json",
    ]);
    assert_eq!(r.code, 2, "{}", r.stdout);
    assert!(r.stderr.contains("--ranker"), "{}", r.stderr);
}

#[test]
fn a_non_monotone_alarm_table_is_a_usage_error() {
    let tmp = TempDir::new("alarm-non-monotone");
    let text = std::fs::read_to_string(
        manifest_dir().join("tests/fixtures/candidates/ranker/fixture-alarm.json"),
    )
    .expect("fixture alarm");
    let mut table: Value = serde_json::from_str(&text).expect("alarm JSON");
    table["blocks"][2]["value"] = Value::from(0.2);
    table["threshold"] = Value::Null;
    let path = tmp.write("alarm.json", &table.to_string());
    let r = alarm_run(
        "finding-shared-function.md",
        "tests/fixtures/candidates/corpus",
        &["--alarm", path.to_str().unwrap(), "--json"],
    );
    assert_eq!(r.code, 2, "{}", r.stdout);
    assert!(r.stderr.contains("not monotone"), "{}", r.stderr);
}

#[test]
fn the_committed_alarm_table_loads_and_runs() {
    let r = alarm_run(
        "finding-shared-function.md",
        "tests/fixtures/candidates/corpus",
        &["--alarm", "eval/dupbench/weights/alarm-v1.json", "--json"],
    );
    assert_eq!(r.code, 0, "{}", r.stderr);
    let report: Value = serde_json::from_str(&r.stdout).expect("JSON");
    assert_eq!(report["alarm"]["status"], "on");
    assert!(report["alarm"]["calibrated"].as_f64().is_some());
}

#[test]
fn adding_unrelated_prior_items_never_adds_an_alarm() {
    // Property over the fixture findings: copying the corpus and adding prior
    // items that share no anchor, neighbour or mechanism leaves the alarm
    // block identical, so an unrelated item can never create a warning.
    let tmp = TempDir::new("alarm-unrelated");
    tmp.copy_dir(
        &manifest_dir().join("tests/fixtures/candidates/corpus"),
        "corpus",
    );
    for i in 0..5 {
        tmp.write(
            &format!("corpus/zz-unrelated-{i}.md"),
            &format!(
                "# Unrelated note {i}\n\nThe governance forum thread {i} discusses a documentation typo in the website footer; no contract is involved.\n"
            ),
        );
    }
    let bigger = tmp.join("corpus");
    let bigger = bigger.to_str().unwrap();
    for name in [
        "finding-shared-function.md",
        "finding-mechanism.md",
        "finding-distance.md",
        "finding-shared-state.md",
        "finding-no-code.md",
    ] {
        for table in [
            "tests/fixtures/candidates/ranker/fixture-alarm.json",
            "eval/dupbench/weights/alarm-v1.json",
        ] {
            let base = alarm_run(
                name,
                "tests/fixtures/candidates/corpus",
                &["--alarm", table, "--json"],
            );
            let more = alarm_run(name, bigger, &["--alarm", table, "--json"]);
            assert_eq!(base.code, 0, "{}", base.stderr);
            assert_eq!(more.code, 0, "{}", more.stderr);
            let base: Value = serde_json::from_str(&base.stdout).unwrap();
            let more: Value = serde_json::from_str(&more.stdout).unwrap();
            assert_eq!(base["alarm"], more["alarm"], "{name} with {table}");
        }
    }
}

#[test]
fn the_alarm_run_is_byte_identical_twice() {
    let args = [
        "--alarm",
        "tests/fixtures/candidates/ranker/fixture-alarm.json",
        "--json",
    ];
    let a = alarm_run(
        "finding-mechanism.md",
        "tests/fixtures/candidates/corpus",
        &args,
    );
    let b = alarm_run(
        "finding-mechanism.md",
        "tests/fixtures/candidates/corpus",
        &args,
    );
    assert_eq!(a.code, 0, "{}", a.stderr);
    assert_eq!(a.stdout, b.stdout);
}

// --- F5 prose anchors ------------------------------------------------------------

fn prose_run(finding_name: &str, extra: &[&str]) -> support::Run {
    let finding_path = format!("tests/fixtures/candidates/findings/{finding_name}");
    let mut args = vec![
        "candidates",
        finding_path.as_str(),
        "--corpus",
        "tests/fixtures/candidates/corpus",
        "--repo",
        "tests/fixtures/candidates-repo",
        "--repo-id",
        "example/candidates-fixture",
        "--commit",
        "0000000000000000000000000000000000000000",
        "--json",
    ];
    args.extend_from_slice(extra);
    run(&args)
}

#[test]
fn a_prose_only_finding_abstains_by_default_and_resolves_with_prose_anchors() {
    // finding-prose.md names `deposit()` without backticks: PA-B1 sees no
    // citation, so the default run abstains on no-anchor; the prose pass reads
    // the bare call, resolves it to Ledger.deposit and admits prior-deposit.
    let off = prose_run("finding-prose.md", &[]);
    assert_eq!(off.code, 0, "{}", off.stderr);
    let off: Value = serde_json::from_str(&off.stdout).unwrap();
    assert_eq!(off["abstention"]["reason"], "no-anchor");

    let on = prose_run("finding-prose.md", &["--prose-anchors"]);
    assert_eq!(on.code, 0, "{}", on.stderr);
    let on: Value = serde_json::from_str(&on.stdout).unwrap();
    assert_eq!(on["abstention"], Value::Null, "{on}");
    assert_eq!(on["anchors"]["functions"], 1);
    let ids: Vec<&str> = on["candidates"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["prior_id"].as_str().unwrap())
        .collect();
    assert!(ids.contains(&"prior-deposit"), "{ids:?}");
}

#[test]
fn prose_anchors_leave_every_anchored_or_code_free_finding_byte_identical() {
    // The prose pass runs only before a no-anchor abstention: a finding that
    // resolves with PA-B1, or that has no prose anchor either, is unchanged.
    for name in [
        "finding-shared-function.md",
        "finding-shared-state.md",
        "finding-distance.md",
        "finding-mechanism.md",
        "finding-no-code.md",
        "finding-no-mechanism.md",
        "finding-vocabulary.md",
    ] {
        let off = prose_run(name, &[]);
        let on = prose_run(name, &["--prose-anchors"]);
        assert_eq!(off.code, 0, "{}", off.stderr);
        assert_eq!(off.stdout, on.stdout, "{name}");
    }
}

#[test]
fn the_default_output_does_not_depend_on_the_prose_pass() {
    // Without --prose-anchors the golden captured before F5 still matches.
    let r = prose_run("finding-shared-function.md", &[]);
    assert_eq!(r.code, 0, "{}", r.stderr);
    let golden = std::fs::read_to_string(
        manifest_dir().join("tests/fixtures/candidates/golden/candidates-default.json"),
    )
    .expect("golden");
    let strip = |text: &str| {
        text.lines()
            .filter(|line| !line.contains("\"tool_version\""))
            .collect::<Vec<_>>()
            .join("\n")
    };
    assert_eq!(strip(&r.stdout), strip(&golden));
}

#[test]
fn prose_anchors_apply_to_prepared_prior_items() {
    // A prior item written only in prose gets its anchor at --prepare time
    // when the flag is passed, and the finding then shares its function.
    let tmp = TempDir::new("prose-prepare");
    tmp.write(
        "corpus/prior-prose.md",
        "# Prior in prose\n\nA second deposit() in the same block is counted twice.\n",
    );
    let corpus_dir = tmp.join("corpus");
    let prepared = tmp.join("prepared.json");
    for (flag, expect_hit) in [(false, false), (true, true)] {
        let mut args = vec![
            "candidates",
            "--corpus",
            corpus_dir.to_str().unwrap(),
            "--repo",
            "tests/fixtures/candidates-repo",
            "--repo-id",
            "example/candidates-fixture",
            "--commit",
            "0000000000000000000000000000000000000000",
            "--prepare",
            prepared.to_str().unwrap(),
            "--json",
        ];
        if flag {
            args.push("--prose-anchors");
        }
        let r = run(&args);
        assert_eq!(r.code, 0, "{}", r.stderr);
        let report: Value = serde_json::from_str(&r.stdout).unwrap();
        assert_eq!(report["anchors"], if flag { 1 } else { 0 }, "{report}");
        let q = run(&[
            "candidates",
            "tests/fixtures/candidates/findings/finding-shared-function.md",
            "--corpus",
            prepared.to_str().unwrap(),
            "--repo",
            "tests/fixtures/candidates-repo",
            "--repo-id",
            "example/candidates-fixture",
            "--commit",
            "0000000000000000000000000000000000000000",
            "--json",
        ]);
        assert_eq!(q.code, 0, "{}", q.stderr);
        let q: Value = serde_json::from_str(&q.stdout).unwrap();
        let hit = q["candidates"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["prior_id"] == "prior-prose");
        assert_eq!(hit, expect_hit, "flag={flag}: {q}");
    }
}

#[test]
fn the_prose_run_is_byte_identical_twice() {
    let a = prose_run("finding-prose.md", &["--prose-anchors"]);
    let b = prose_run("finding-prose.md", &["--prose-anchors"]);
    assert_eq!(a.code, 0, "{}", a.stderr);
    assert_eq!(a.stdout, b.stdout);
}
