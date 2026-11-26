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
