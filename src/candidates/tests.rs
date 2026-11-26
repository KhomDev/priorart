//! Unit tests for candidate generation and features.
//!
//! The feature-value tests build anchors and a call graph by hand, so the
//! expected numbers are exact and do not depend on `solar`. The rule tests run
//! over `tests/fixtures/candidates-repo`, a six-function Solidity fixture whose
//! call graph is `entry -> middle -> leaf`.

use super::*;
use crate::anchors::SymbolInfo;
use crate::taxonomy::{Match as TaxonomyMatch, Span as TaxonomySpan, TAXONOMY_SCHEMA};
use std::path::{Path, PathBuf};

fn manifest(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel)
}

fn fixture_index() -> IndexSet {
    let root = manifest("tests/fixtures/candidates-repo");
    let repo = crate::anchors::parse_repo(
        &root,
        "example/candidates-fixture",
        "0000000000000000000000000000000000000000",
        &crate::anchors::IndexOptions::default(),
    )
    .expect("fixture repository parses");
    IndexSet::single(repo)
}

fn corpus(name: &str) -> Corpus {
    load_corpus(&manifest("tests/fixtures/candidates").join(name)).expect("corpus loads")
}

fn finding(name: &str) -> String {
    std::fs::read_to_string(
        manifest("tests/fixtures/candidates")
            .join("findings")
            .join(name),
    )
    .expect("finding fixture is readable")
}

fn anchor(repo: &str, file: &str, symbol: &str, kind: AnchorKind) -> Anchor {
    Anchor {
        repo: repo.to_string(),
        commit: "0".to_string(),
        file: file.to_string(),
        span: crate::anchors::Span {
            start_byte: 0,
            end_byte: 1,
            start_line: 1,
            end_line: 1,
        },
        kind,
        symbol: symbol.to_string(),
        confidence: 90,
    }
}

fn function_anchor(symbol: &str) -> Anchor {
    anchor(
        "example/candidates-fixture",
        "contracts/Ledger.sol",
        symbol,
        AnchorKind::Function,
    )
}

fn state_anchor(symbol: &str) -> Anchor {
    anchor(
        "example/candidates-fixture",
        "contracts/Ledger.sol",
        symbol,
        AnchorKind::StateVar,
    )
}

fn class_matches(classes: &[&str]) -> Vec<TaxonomyMatch> {
    classes
        .iter()
        .enumerate()
        .map(|(i, class)| TaxonomyMatch {
            class: (*class).to_string(),
            span: TaxonomySpan {
                start_byte: i,
                end_byte: i + 1,
            },
            term: (*class).to_string(),
        })
        .collect()
}

fn mechanism(root_causes: &[&str], impacts: &[&str]) -> MechanismSet {
    MechanismSet {
        schema: TAXONOMY_SCHEMA.to_string(),
        root_causes: class_matches(root_causes),
        impacts: class_matches(impacts),
        assets: Vec::new(),
        abstain_reason: None,
    }
}

fn function(id: &str, name: &str, contract: &str) -> SymbolInfo {
    SymbolInfo {
        id: id.to_string(),
        name: name.to_string(),
        kind: AnchorKind::Function,
        file: "contracts/Ledger.sol".to_string(),
        span: crate::anchors::Span {
            start_byte: 0,
            end_byte: 1,
            start_line: 1,
            end_line: 1,
        },
        contract: Some(contract.to_string()),
        bases: Vec::new(),
        function_kind: Some("function".to_string()),
        var_type: None,
    }
}

fn hand_built_repo() -> RepoIndex {
    RepoIndex {
        repo: "example/candidates-fixture".to_string(),
        commit: "0".to_string(),
        root: String::new(),
        commit_verified: true,
        files: BTreeMap::new(),
        contracts: Vec::new(),
        functions: vec![
            function("Ledger::entry", "entry", "Ledger"),
            function("Ledger::middle", "middle", "Ledger"),
            function("Ledger::leaf", "leaf", "Ledger"),
            function("Ledger::alpha", "alpha", "Ledger"),
            function("Ledger::beta", "beta", "Ledger"),
        ],
        state_variables: Vec::new(),
        call_graph: BTreeMap::from([
            (
                "Ledger::entry".to_string(),
                vec!["Ledger::middle".to_string()],
            ),
            (
                "Ledger::middle".to_string(),
                vec!["Ledger::leaf".to_string()],
            ),
        ]),
        calls: Vec::new(),
        read_write: BTreeMap::new(),
        parse_failures: Vec::new(),
    }
}

#[test]
fn anchor_overlap_is_exact_and_kind_scoped() {
    let finding_anchors = vec![
        function_anchor("Ledger.a"),
        function_anchor("Ledger.b"),
        function_anchor("Ledger.c"),
        state_anchor("Ledger.s1"),
    ];
    let prior_anchors = vec![
        function_anchor("Ledger.b"),
        function_anchor("Ledger.c"),
        function_anchor("Ledger.d"),
        state_anchor("Ledger.s1"),
        state_anchor("Ledger.s2"),
    ];
    let functions = overlap(&finding_anchors, &prior_anchors, AnchorKind::Function);
    assert_eq!(functions.shared, vec!["Ledger.b", "Ledger.c"]);
    assert_eq!(functions.finding, 3);
    assert_eq!(functions.prior, 3);
    assert_eq!(functions.ratio, Some(0.5));

    let states = overlap(&finding_anchors, &prior_anchors, AnchorKind::StateVar);
    assert_eq!(states.shared, vec!["Ledger.s1"]);
    assert_eq!(states.finding, 1);
    assert_eq!(states.prior, 2);
    assert_eq!(states.ratio, Some(0.5));
}

#[test]
fn call_graph_distance_is_exact_at_zero_one_two_and_none() {
    let repos = [hand_built_repo()];
    let index = FunctionIndex::new(&repos);
    let finding = [function_anchor("Ledger.entry")];
    let distances = index.distances(&finding);
    let distance_to = |symbol: &str| {
        let key = index.key_of(&function_anchor(symbol)).expect("key");
        distances.get(&key).copied()
    };
    assert_eq!(distance_to("Ledger.entry"), Some(0));
    assert_eq!(distance_to("Ledger.middle"), Some(1));
    assert_eq!(distance_to("Ledger.leaf"), Some(2));
    assert_eq!(distance_to("Ledger.alpha"), None);
    // A different repository never shares a graph node, even with the same id.
    let other = anchor(
        "other/repo",
        "contracts/Ledger.sol",
        "Ledger.middle",
        AnchorKind::Function,
    );
    assert_eq!(index.key_of(&other), None);
}

#[test]
fn mechanism_match_requires_a_shared_contract_and_impact_is_separate() {
    let repos = [hand_built_repo()];
    let index = FunctionIndex::new(&repos);
    let finding_anchors = [function_anchor("Ledger.alpha")];
    let distances = index.distances(&finding_anchors);
    let rounding = mechanism(&["rounding-precision"], &["fund-loss"]);
    let same_contract = mechanism(&["rounding-precision"], &["fund-loss"]);
    let (features, reasons) = pair_features(
        &finding_anchors,
        &[function_anchor("Ledger.beta")],
        &rounding,
        &same_contract,
        Some(&index),
        Some(&distances),
        IdentifierScore::none(),
    );
    assert_eq!(
        features.mechanism_match.shared_root_causes,
        vec!["rounding-precision"]
    );
    assert_eq!(features.mechanism_match.contracts, vec!["Ledger"]);
    assert!(features.mechanism_match.same_contract);
    assert_eq!(features.impact_match.shared_impacts, vec!["fund-loss"]);
    assert_eq!(reasons.len(), 1);
    assert_eq!(reasons[0].rule, CandidateRule::MechanismContract);

    // The same mechanism in a different contract is not the mechanism rule;
    // the impact match is reported but never admits a pair on its own.
    let (features, reasons) = pair_features(
        &finding_anchors,
        &[anchor(
            "example/candidates-fixture",
            "contracts/Other.sol",
            "Other.gamma",
            AnchorKind::Function,
        )],
        &rounding,
        &mechanism(&["rounding-precision"], &["fund-loss"]),
        Some(&index),
        Some(&distances),
        IdentifierScore::none(),
    );
    assert_eq!(features.impact_match.shared_impacts, vec!["fund-loss"]);
    assert!(!features.mechanism_match.same_contract);
    assert!(reasons.is_empty(), "{reasons:?}");

    // Shared impacts alone never admit a pair.
    let (features, reasons) = pair_features(
        &finding_anchors,
        &[function_anchor("Ledger.beta")],
        &mechanism(&[], &["fund-loss"]),
        &mechanism(&[], &["fund-loss"]),
        Some(&index),
        Some(&distances),
        IdentifierScore::none(),
    );
    assert_eq!(features.impact_match.shared_impacts, vec!["fund-loss"]);
    assert!(reasons.is_empty(), "{reasons:?}");
}

#[test]
fn identifier_score_is_the_existing_matcher_row() {
    let finding_text = "`LedgerBook` and `GhostAccount` are both wrong.";
    let priors = vec![PreparedPrior {
        id: "prior-vocabulary".to_string(),
        title: None,
        anchors: Vec::new(),
        mechanism: mechanism(&[], &[]),
        identifiers: Profile::from_text(
            "`LedgerBook` and `GhostAccount` appear in the prior item.",
        )
        .all_identifiers(),
    }];
    let scores = identifier_scores(&priors, finding_text);
    let row = scores.get(&0).expect("the matcher ranks the pair");
    assert!(row.ranked);
    assert_eq!(row.specific_symbols, 2);
    assert_eq!(row.shared_symbols, 2);
    assert!(row
        .shared
        .iter()
        .any(|s| s.eq_ignore_ascii_case("ledgerbook")));
}

#[test]
fn a_shared_function_anchor_is_a_candidate() {
    let index = fixture_index();
    let report = generate(
        &finding("finding-shared-function.md"),
        Path::new("finding-shared-function.md"),
        &corpus("corpus"),
        &GenerateOptions::new(Some(&index), "sha"),
    );
    assert!(report.abstention.is_none(), "{:?}", report.abstention);
    let ids: Vec<&str> = report
        .candidates
        .iter()
        .map(|c| c.prior_id.as_str())
        .collect();
    assert_eq!(
        ids,
        vec!["prior-deposit", "prior-leaf", "prior-total-assets"]
    );
    let first = &report.candidates[0];
    let f = &first.features;
    assert_eq!(f.anchor_function_overlap.shared, vec!["Ledger.deposit"]);
    assert_eq!(f.anchor_function_overlap.finding, 1);
    assert_eq!(f.anchor_function_overlap.prior, 1);
    assert_eq!(f.anchor_function_overlap.ratio, Some(1.0));
    assert_eq!(
        f.anchor_state_overlap.shared,
        vec!["Ledger.balances", "Ledger.totalAssets"]
    );
    assert_eq!(f.anchor_state_overlap.finding, 2);
    assert_eq!(f.anchor_state_overlap.prior, 2);
    assert_eq!(f.anchor_state_overlap.ratio, Some(1.0));
    assert_eq!(f.call_graph_distance, Some(0));
    assert_eq!(f.identifier_score.specific_symbols, 2);
    assert!(f.identifier_score.ranked);
    assert_eq!(first.reasons[0].rule, CandidateRule::SharedFunction);
    assert_eq!(first.reasons[1].rule, CandidateRule::SharedState);
}

#[test]
fn a_shared_state_variable_is_a_candidate() {
    let index = fixture_index();
    let report = generate(
        &finding("finding-shared-state.md"),
        Path::new("finding-shared-state.md"),
        &corpus("corpus"),
        &GenerateOptions::new(Some(&index), "sha"),
    );
    let prior = report
        .candidates
        .iter()
        .find(|c| c.prior_id == "prior-total-assets")
        .expect("prior-total-assets is a candidate");
    assert!(prior
        .reasons
        .iter()
        .any(|r| r.rule == CandidateRule::SharedState));
    assert_eq!(
        prior.features.anchor_state_overlap.shared,
        vec!["Ledger.totalAssets"]
    );
}

#[test]
fn a_call_graph_distance_two_neighbour_is_a_candidate() {
    let index = fixture_index();
    let report = generate(
        &finding("finding-distance.md"),
        Path::new("finding-distance.md"),
        &corpus("corpus"),
        &GenerateOptions::new(Some(&index), "sha"),
    );
    let ids: Vec<&str> = report
        .candidates
        .iter()
        .map(|c| c.prior_id.as_str())
        .collect();
    assert_eq!(ids, vec!["prior-unrelated", "prior-leaf"]);
    let one = &report.candidates[0];
    assert_eq!(one.features.call_graph_distance, Some(1));
    assert_eq!(one.reasons[0].rule, CandidateRule::CallGraph);
    assert_eq!(one.reasons[0].distance, Some(1));
    assert_eq!(one.reasons[0].finding_functions, vec!["Ledger.entry"]);
    assert_eq!(one.reasons[0].prior_functions, vec!["Ledger.middle"]);
    let two = &report.candidates[1];
    assert_eq!(two.features.call_graph_distance, Some(2));
    assert_eq!(two.reasons[0].distance, Some(2));
}

#[test]
fn the_same_mechanism_in_the_same_contract_is_a_candidate() {
    let index = fixture_index();
    let report = generate(
        &finding("finding-mechanism.md"),
        Path::new("finding-mechanism.md"),
        &corpus("corpus"),
        &GenerateOptions::new(Some(&index), "sha"),
    );
    let ids: Vec<&str> = report
        .candidates
        .iter()
        .map(|c| c.prior_id.as_str())
        .collect();
    assert_eq!(ids, vec!["prior-mechanism"]);
    let candidate = &report.candidates[0];
    assert_eq!(candidate.reasons.len(), 1);
    assert_eq!(candidate.reasons[0].rule, CandidateRule::MechanismContract);
    assert_eq!(candidate.reasons[0].root_causes, vec!["rounding-precision"]);
    assert_eq!(candidate.reasons[0].contracts, vec!["Ledger"]);
    assert_eq!(candidate.features.call_graph_distance, None);
    assert_eq!(candidate.features.anchor_function_overlap.shared.len(), 0);
    assert_eq!(candidate.features.anchor_state_overlap.shared.len(), 0);
}

#[test]
fn shared_vocabulary_alone_is_not_a_candidate() {
    let index = fixture_index();
    let finding_text = finding("finding-vocabulary.md");
    let prior_text = std::fs::read_to_string(manifest(
        "tests/fixtures/candidates/corpus-vocabulary/prior-vocabulary.md",
    ))
    .expect("prior text");
    let finding_anchors = crate::anchors::resolve_text(&finding_text, &index).anchors;
    let prior_anchors = crate::anchors::resolve_text(&prior_text, &index).anchors;
    let function_index = FunctionIndex::new(index.repos.as_slice());
    let distances = function_index.distances(&finding_anchors);
    let score = identifier_scores(
        &[PreparedPrior {
            id: "prior-vocabulary".to_string(),
            title: None,
            anchors: prior_anchors.clone(),
            mechanism: mechanism(&[], &[]),
            identifiers: Profile::from_text(&prior_text).all_identifiers(),
        }],
        &finding_text,
    )
    .remove(&0)
    .expect("the identifier matcher ranks the pair");
    assert!(
        score.ranked,
        "the matcher should score the shared vocabulary"
    );
    assert_eq!(score.specific_symbols, 2);
    let (features, reasons) = pair_features(
        &finding_anchors,
        &prior_anchors,
        &mechanism(&[], &[]),
        &mechanism(&[], &[]),
        Some(&function_index),
        Some(&distances),
        score,
    );
    assert!(reasons.is_empty(), "{reasons:?}");
    assert!(features.identifier_score.ranked);
    assert_eq!(features.anchor_function_overlap.shared.len(), 0);
    assert_eq!(features.anchor_state_overlap.shared.len(), 0);
    assert_eq!(features.call_graph_distance, None);

    // The whole report abstains: the finding has an anchor and no mechanism,
    // and no prior item qualified.
    let report = generate(
        &finding_text,
        Path::new("finding-vocabulary.md"),
        &corpus("corpus-vocabulary"),
        &GenerateOptions::new(Some(&index), "sha"),
    );
    assert!(report.candidates.is_empty());
    assert_eq!(
        report.abstention.expect("abstention").reason,
        AbstainReason::NoMechanism
    );
}

#[test]
fn abstention_corpus_missing() {
    let index = fixture_index();
    let report = generate(
        &finding("finding-shared-function.md"),
        Path::new("finding-shared-function.md"),
        &corpus("empty-corpus.json"),
        &GenerateOptions::new(Some(&index), "sha"),
    );
    assert!(report.candidates.is_empty());
    let abstention = report.abstention.expect("abstention");
    assert_eq!(abstention.reason, AbstainReason::CorpusMissing);
}

#[test]
fn abstention_repo_missing() {
    let report = generate(
        &finding("finding-shared-function.md"),
        Path::new("finding-shared-function.md"),
        &corpus("corpus"),
        &GenerateOptions::new(None, "sha"),
    );
    assert!(report.candidates.is_empty());
    let abstention = report.abstention.expect("abstention");
    assert_eq!(abstention.reason, AbstainReason::RepoMissing);
}

#[test]
fn abstention_no_anchor() {
    let index = fixture_index();
    let report = generate(
        &finding("finding-no-code.md"),
        Path::new("finding-no-code.md"),
        &corpus("corpus"),
        &GenerateOptions::new(Some(&index), "sha"),
    );
    assert!(report.candidates.is_empty());
    assert_eq!(
        report.abstention.expect("abstention").reason,
        AbstainReason::NoAnchor
    );
    assert!(!report.anchors.cites_code);
}

#[test]
fn abstention_no_mechanism() {
    let index = fixture_index();
    let report = generate(
        &finding("finding-no-mechanism.md"),
        Path::new("finding-no-mechanism.md"),
        &corpus("corpus-vocabulary"),
        &GenerateOptions::new(Some(&index), "sha"),
    );
    assert!(report.candidates.is_empty());
    assert_eq!(
        report.abstention.expect("abstention").reason,
        AbstainReason::NoMechanism
    );
    assert!(report.mechanism.root_causes.is_empty());
}

#[test]
fn abstention_no_candidates() {
    let index = fixture_index();
    let report = generate(
        &finding("finding-mechanism.md"),
        Path::new("finding-mechanism.md"),
        &corpus("corpus-deposit"),
        &GenerateOptions::new(Some(&index), "sha"),
    );
    assert!(report.candidates.is_empty());
    assert_eq!(
        report.abstention.expect("abstention").reason,
        AbstainReason::NoCandidates
    );
    assert!(!report.mechanism.root_causes.is_empty());
}

#[test]
fn generation_is_byte_identical_twice() {
    let index = fixture_index();
    let text = finding("finding-shared-function.md");
    let options = GenerateOptions::new(Some(&index), "sha");
    let a = generate(
        &text,
        Path::new("finding-shared-function.md"),
        &corpus("corpus"),
        &options,
    );
    let b = generate(
        &text,
        Path::new("finding-shared-function.md"),
        &corpus("corpus"),
        &options,
    );
    assert_eq!(
        serde_json::to_vec(&a).expect("a"),
        serde_json::to_vec(&b).expect("b")
    );
}

#[test]
fn excluded_items_are_not_candidates() {
    let index = fixture_index();
    let mut options = GenerateOptions::new(Some(&index), "sha");
    options.exclude.insert("prior-deposit".to_string());
    let report = generate(
        &finding("finding-shared-function.md"),
        Path::new("finding-shared-function.md"),
        &corpus("corpus"),
        &options,
    );
    assert!(report
        .candidates
        .iter()
        .all(|c| c.prior_id != "prior-deposit"));
    assert_eq!(report.limits.excluded, 1);
    assert_eq!(report.corpus.items, 6);
}

#[test]
fn a_prepared_corpus_produces_the_same_candidates() {
    let index = fixture_index();
    let original = corpus("corpus");
    let prepared = prepare_corpus(&original, Some(&index));
    assert!(prepared.items.iter().all(|i| i.anchors.is_some()));
    assert!(prepared.items.iter().all(|i| i.mechanism.is_some()));
    let reused = Corpus {
        kind: CorpusKind::Prepared,
        path: "prepared.json".to_string(),
        items: prepared.items,
        repositories: prepared.repositories,
        prepared: true,
    };
    let text = finding("finding-shared-function.md");
    let options = GenerateOptions::new(Some(&index), "sha");
    let direct = generate(
        &text,
        Path::new("finding-shared-function.md"),
        &original,
        &options,
    );
    let cached = generate(
        &text,
        Path::new("finding-shared-function.md"),
        &reused,
        &options,
    );
    assert_eq!(
        serde_json::to_vec(&direct.candidates).expect("direct"),
        serde_json::to_vec(&cached.candidates).expect("cached")
    );
    assert!(cached.corpus.prepared);
}

#[test]
fn the_directory_loader_sorts_items_by_id() {
    let loaded = corpus("corpus");
    let ids: Vec<&str> = loaded.items.iter().map(|i| i.id.as_str()).collect();
    let mut sorted = ids.clone();
    sorted.sort();
    assert_eq!(ids, sorted);
    assert_eq!(loaded.kind, CorpusKind::Directory);
    assert!(!loaded.prepared);
}

#[test]
fn load_index_rejects_an_empty_repository_id() {
    let path = std::env::temp_dir().join(format!(
        "priorart-candidates-test-{}-empty-index.json",
        std::process::id()
    ));
    let mut repo = hand_built_repo();
    repo.repo = String::new();
    let body = serde_json::to_vec(&repo).expect("serialise index");
    std::fs::write(&path, body).expect("write fixture");
    let result = load_index(&path);
    let _ = std::fs::remove_file(&path);
    assert!(result.is_err(), "{result:?}");
    assert!(
        result.unwrap_err().contains("empty repository id"),
        "the error names the empty id"
    );
}

#[test]
fn the_ordering_array_is_the_documented_key() {
    assert_eq!(ORDERING.len(), 5);
    assert!(ORDERING[0].starts_with("anchor overlap total"));
    assert!(ORDERING[4].starts_with("prior item id"));
}

// --- PA-B4 ranker inference -------------------------------------------------

use super::ranker::{Ranker, RankerFeature, RankerWeights, RANKER_FEATURE_NAMES};

fn ranker_candidate(distance: Option<u32>) -> Candidate {
    Candidate {
        prior_id: "prior-x".to_string(),
        prior_title: None,
        reasons: Vec::new(),
        features: Features {
            anchor_function_overlap: Overlap {
                shared: vec!["Ledger.entry".to_string()],
                finding: 1,
                prior: 1,
                ratio: Some(1.0),
            },
            anchor_state_overlap: Overlap {
                shared: vec!["Ledger.s1".to_string(), "Ledger.s2".to_string()],
                finding: 2,
                prior: 2,
                ratio: Some(1.0),
            },
            call_graph_distance: distance,
            mechanism_match: MechanismMatch {
                shared_root_causes: vec!["rounding-precision".to_string()],
                same_contract: true,
                contracts: vec!["Ledger".to_string()],
            },
            impact_match: ImpactMatch {
                shared_impacts: Vec::new(),
            },
            identifier_score: IdentifierScore {
                ranked: false,
                specific_symbols: 0,
                shared_symbols: 0,
                rarity: 0,
                shared: Vec::new(),
            },
        },
        ranker_score: None,
    }
}

fn fixture_weights() -> RankerWeights {
    // The same hand-checkable arithmetic as the committed JSON fixture: a
    // non-unit mean and scale (function overlap: mean 2.0, scale 0.5; distance:
    // mean 1.0, scale 0.5), so a scorer that drops the mean or the scale
    // cannot pass. This hand-built candidate's rarity is zero, so its log1p
    // transform is a no-op here; the JSON fixture exercises it with a nonzero
    // value.
    let coefficients: BTreeMap<&str, f64> = BTreeMap::from([
        ("anchor_function_overlap.shared", -0.5),
        ("call_graph_distance", -0.25),
        ("call_graph_distance_missing", 3.0),
        ("mechanism_match.shared_root_causes", 1.0),
        ("identifier_score.rarity", 0.5),
    ]);
    let features = RANKER_FEATURE_NAMES
        .iter()
        .map(|name| {
            let (mean, scale) = match *name {
                "anchor_function_overlap.shared" => (2.0, 0.5),
                "call_graph_distance" => (1.0, 0.5),
                _ => (0.0, 1.0),
            };
            let transform = if *name == "identifier_score.rarity" {
                "log1p"
            } else {
                "identity"
            };
            RankerFeature {
                name: (*name).to_string(),
                kind: None,
                transform: transform.to_string(),
                missing_value: if name.ends_with(".ratio") {
                    Some(0.0)
                } else if *name == "call_graph_distance" {
                    Some(3.0)
                } else {
                    None
                },
                mean,
                scale,
                coefficient: coefficients.get(name).copied().unwrap_or(0.0),
            }
        })
        .collect();
    RankerWeights {
        schema: RANKER_WEIGHTS_SCHEMA.to_string(),
        version: 1,
        seed: 1,
        code_commit: None,
        features,
        intercept: 0.0,
    }
}

#[test]
fn ranker_scores_a_hand_built_candidate_exactly() {
    let ranker = Ranker::from_weights(fixture_weights()).expect("fixture weights validate");
    // z = -0.5*(1-2)/0.5 (function shared, mean 2 scale 0.5)
    //     + -0.25*(0-1)/0.5 (distance 0, mean 1 scale 0.5)
    //     + 3*0 (not missing) + 1*1 (shared root cause) = 2.5.
    let scored = ranker.score(&ranker_candidate(Some(0)));
    assert!(
        (scored - 0.9241418199787566).abs() < 1e-12,
        "scored {scored}"
    );
    // A null distance uses missing_value 3.0 and sets the indicator:
    // z = 1 - 0.25*(3-1)/0.5 + 3 + 1 = 1 - 1 + 3 + 1 = 4.0.
    let null_distance = ranker.score(&ranker_candidate(None));
    assert!(
        (null_distance - 0.9820137900379085).abs() < 1e-12,
        "scored {null_distance}"
    );
}

#[test]
fn ranker_validate_rejects_a_missing_feature() {
    let mut weights = fixture_weights();
    weights
        .features
        .retain(|feature| feature.name != "identifier_score.rarity");
    let error = weights.validate().expect_err("missing feature is rejected");
    assert!(
        error.contains("identifier_score.rarity") && error.contains("missing"),
        "{error}"
    );
}

#[test]
fn ranker_validate_rejects_unknown_and_duplicate_features() {
    let mut unknown = fixture_weights();
    unknown.features[0].name = "anchor_function_overlap.bogus".to_string();
    let error = unknown.validate().expect_err("unknown feature is rejected");
    assert!(error.contains("unknown feature"), "{error}");

    let mut duplicate = fixture_weights();
    duplicate.features[1].name = duplicate.features[0].name.clone();
    let error = duplicate
        .validate()
        .expect_err("duplicate feature is rejected");
    assert!(error.contains("more than once"), "{error}");
}

#[test]
fn ranker_validate_requires_the_missingness_rule() {
    let mut weights = fixture_weights();
    for feature in &mut weights.features {
        if feature.name == "call_graph_distance" {
            feature.missing_value = None;
        }
    }
    let error = weights
        .validate()
        .expect_err("null without a rule is rejected");
    assert!(error.contains("missing_value"), "{error}");
}

// --- PA-B5 calibrated alarm ---------------------------------------------------

use super::alarm::{Alarm, AlarmBlock, AlarmCalibration};

fn committed_alarm() -> Alarm {
    Alarm::load(&manifest("eval/dupbench/weights/alarm-v1.json"))
        .expect("the committed calibration table loads and validates")
}

fn scored_candidate(id: &str, score: f64) -> Candidate {
    let mut candidate = ranker_candidate(Some(0));
    candidate.prior_id = id.to_string();
    candidate.ranker_score = Some(score);
    candidate
}

fn block(score_lo: f64, score_hi: f64, value: f64) -> AlarmBlock {
    AlarmBlock {
        score_lo,
        score_hi,
        n: 10,
        k: (value * 10.0).round() as u64,
        value,
    }
}

#[test]
fn the_committed_calibration_is_monotone() {
    let alarm = committed_alarm();
    let table = alarm.calibration();
    for pair in table.blocks.windows(2) {
        assert!(pair[0].score_hi < pair[1].score_lo, "blocks overlap");
        assert!(pair[0].value <= pair[1].value, "block values decrease");
    }
    // The step map itself is non-decreasing over the whole score range.
    let mut previous = f64::NEG_INFINITY;
    for step in 0..=10_000 {
        let raw = f64::from(step) / 10_000.0;
        let value = table.calibrate(raw);
        assert!(value >= previous, "calibrate({raw}) = {value} < {previous}");
        previous = value;
    }
}

#[test]
fn a_non_monotone_calibration_is_rejected() {
    let table = AlarmCalibration {
        schema: ALARM_CALIBRATION_SCHEMA.to_string(),
        version: 1,
        blocks: vec![block(0.0, 0.4, 0.6), block(0.5, 1.0, 0.3)],
        threshold: None,
        validation_contests: Vec::new(),
    };
    let error = table.validate().expect_err("a decreasing map is rejected");
    assert!(error.contains("not monotone"), "{error}");

    let overlapping = AlarmCalibration {
        blocks: vec![block(0.0, 0.6, 0.2), block(0.5, 1.0, 0.3)],
        ..table.clone()
    };
    assert!(overlapping.validate().is_err());

    let foreign_threshold = AlarmCalibration {
        blocks: vec![block(0.0, 0.4, 0.2), block(0.5, 1.0, 0.9)],
        threshold: Some(0.85),
        ..table
    };
    let error = foreign_threshold
        .validate()
        .expect_err("a threshold off the block values is rejected");
    assert!(
        error.contains("not one of the calibration block values"),
        "{error}"
    );
}

#[test]
fn the_shipped_threshold_is_exact_at_its_block_edge() {
    // Committed by eval/dupbench/calibrate_alarm.py on the validation split:
    // bin 16 holds 122 queries with 110 true alarms, so its isotonic value is
    // exactly 110/122; its tail (bins 16-18) is 149/162 with Wilson lower
    // bound 0.8676 >= 0.80, and the tail from bin 15 (262/318) has lower
    // bound 0.7782 < 0.80. The lowest raw score in bin 16 is 0.8548965723626742.
    //
    // serde_json's default float parser may land one ULP from the decimal in
    // the file, so the comparisons with the hand-computed fractions allow
    // 1e-15; every comparison between table values is exact.
    let alarm = committed_alarm();
    let table = alarm.calibration();
    let threshold = table.threshold.expect("the committed alarm ships on");
    assert!((threshold - 110.0 / 122.0).abs() < 1e-15, "{threshold}");
    let shipped: Vec<&AlarmBlock> = table
        .blocks
        .iter()
        .filter(|b| b.value == threshold)
        .collect();
    assert_eq!(shipped.len(), 1);
    assert_eq!((shipped[0].n, shipped[0].k), (122, 110));
    let edge = shipped[0].score_lo;
    assert!((edge - 0.854_896_572_362_674_2).abs() < 1e-15, "{edge}");
    // At the edge the calibrated value is exactly the threshold and fires.
    assert_eq!(table.calibrate(edge), threshold);
    assert!(table.fires(table.calibrate(edge)));
    // One ULP below the edge is the bin-15 value (113/156) and does not fire.
    let below = f64::from_bits(edge.to_bits() - 1);
    let below_value = table.calibrate(below);
    assert!((below_value - 113.0 / 156.0).abs() < 1e-15, "{below_value}");
    assert!(below_value < threshold);
    assert!(!table.fires(below_value));
}

#[test]
fn the_alarm_compares_the_calibrated_value_not_the_raw_score() {
    // A raw top score of 0.86 is below the calibrated threshold (0.9016...) but
    // falls in bin 16, whose calibrated value is the threshold: the alarm must
    // fire. Comparing the raw score with the threshold (skipping calibration)
    // would not fire, and this test would fail.
    let alarm = committed_alarm();
    let report = alarm.evaluate(&[scored_candidate("prior-a", 0.86)]);
    assert!(report.fires, "{report:?}");
    assert_eq!(report.warning.as_deref(), Some(ALARM_WARNING));
    assert_eq!(report.top_candidate.as_deref(), Some("prior-a"));
    assert_eq!(report.top_score, Some(0.86));
    assert_eq!(report.calibrated, alarm.calibration().threshold);
    // And the other direction: 0.85 is raw-below too and calibrates to the
    // bin-15 value, so nothing fires.
    let quiet = alarm.evaluate(&[scored_candidate("prior-a", 0.85)]);
    assert!(!quiet.fires, "{quiet:?}");
    assert_eq!(quiet.warning, None);
}

#[test]
fn an_alarm_that_ships_off_never_fires() {
    let table = AlarmCalibration {
        schema: ALARM_CALIBRATION_SCHEMA.to_string(),
        version: 1,
        blocks: vec![block(0.0, 0.4, 0.2), block(0.5, 1.0, 0.7)],
        threshold: None,
        validation_contests: Vec::new(),
    };
    let alarm = Alarm::from_calibration(table).expect("an off table validates");
    for score in [0.0, 0.5, 0.99, 1.0] {
        let report = alarm.evaluate(&[scored_candidate("prior-a", score)]);
        assert!(!report.fires);
        assert_eq!(report.status, "off");
    }
    let empty = alarm.evaluate(&[]);
    assert!(!empty.fires && empty.top_candidate.is_none());
}

#[test]
fn the_committed_calibration_names_no_final_contest() {
    // FINAL refusal, checked on the committed artifact: every contest the
    // calibration was fitted on is a validation contest in the Dup-Bench
    // manifest, and none is FINAL.
    let alarm = committed_alarm();
    let manifest: serde_json::Value = serde_json::from_slice(
        &std::fs::read(manifest("eval/dupbench/manifest.json")).expect("manifest"),
    )
    .expect("manifest JSON");
    let split_of = |slug: &str| -> Option<String> {
        manifest["contests"]
            .as_array()
            .expect("contests")
            .iter()
            .find(|c| c["slug"] == slug)
            .and_then(|c| c["split"].as_str().map(str::to_string))
    };
    let contests = &alarm.calibration().validation_contests;
    assert!(!contests.is_empty());
    for slug in contests {
        assert_eq!(
            split_of(slug).as_deref(),
            Some("validation"),
            "{slug} is not a validation contest"
        );
    }
}

// --- PA-B5 calibration table validation, field by field ----------------------

fn table_of(blocks: Vec<AlarmBlock>, threshold: Option<f64>) -> AlarmCalibration {
    AlarmCalibration {
        schema: ALARM_CALIBRATION_SCHEMA.to_string(),
        version: 1,
        blocks,
        threshold,
        validation_contests: Vec::new(),
    }
}

fn raw_block(score_lo: f64, score_hi: f64, n: u64, k: u64, value: f64) -> AlarmBlock {
    AlarmBlock {
        score_lo,
        score_hi,
        n,
        k,
        value,
    }
}

#[test]
fn each_non_finite_block_field_is_rejected_on_its_own() {
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        for (what, block) in [
            ("score_lo", raw_block(bad, 0.5, 10, 5, 0.5)),
            ("score_hi", raw_block(0.1, bad, 10, 5, 0.5)),
            ("value", raw_block(0.1, 0.5, 10, 5, bad)),
        ] {
            let error = table_of(vec![block], None).validate().expect_err(what);
            assert!(
                error.contains("non-finite or out-of-range"),
                "{what}: {error}"
            );
        }
    }
}

#[test]
fn a_finite_value_outside_zero_one_is_rejected() {
    for value in [1.5, -0.25] {
        let error = table_of(vec![raw_block(0.1, 0.5, 10, 5, value)], None)
            .validate()
            .expect_err("out of range");
        assert!(error.contains("out-of-range"), "{error}");
    }
    // The closed ends are allowed.
    for value in [0.0, 1.0] {
        table_of(vec![raw_block(0.1, 0.5, 10, 5, value)], None)
            .validate()
            .expect("0 and 1 are valid rates");
    }
}

#[test]
fn a_single_point_block_is_valid_and_an_inverted_one_is_not() {
    table_of(vec![raw_block(0.4, 0.4, 3, 1, 1.0 / 3.0)], None)
        .validate()
        .expect("score_lo == score_hi is one observed score");
    let error = table_of(vec![raw_block(0.5, 0.4, 3, 1, 1.0 / 3.0)], None)
        .validate()
        .expect_err("score_lo above score_hi");
    assert!(error.contains("score_lo above score_hi"), "{error}");
}

#[test]
fn block_counts_need_n_positive_and_k_at_most_n() {
    let error = table_of(vec![raw_block(0.1, 0.5, 0, 0, 0.0)], None)
        .validate()
        .expect_err("n == 0");
    assert!(error.contains("n 0 and k 0"), "{error}");
    let error = table_of(vec![raw_block(0.1, 0.5, 4, 5, 1.0)], None)
        .validate()
        .expect_err("k > n");
    assert!(error.contains("n 4 and k 5"), "{error}");
    table_of(vec![raw_block(0.1, 0.5, 5, 5, 1.0)], None)
        .validate()
        .expect("k == n is valid");
}

#[test]
fn equal_adjacent_block_values_are_accepted_as_monotone() {
    // The contract is a non-decreasing map. calibrate_alarm.py pools ties, so
    // its own output always has strictly increasing values, but a table with
    // two equal adjacent values is still monotone and maps every score the
    // same way a pooled table would; it is accepted.
    let table = table_of(
        vec![
            raw_block(0.1, 0.2, 10, 5, 0.5),
            raw_block(0.3, 0.4, 10, 5, 0.5),
        ],
        Some(0.5),
    );
    table
        .validate()
        .expect("equal adjacent values are monotone");
    assert_eq!(table.calibrate(0.35), 0.5);
}

#[test]
fn the_size_cap_accepts_exactly_the_limit_and_rejects_one_byte_more() {
    let path = std::path::Path::new("alarm.json");
    super::alarm::check_size(MAX_JSON_BYTES, path).expect("exactly the cap is read");
    super::alarm::check_size(MAX_JSON_BYTES - 1, path).expect("under the cap is read");
    let error = super::alarm::check_size(MAX_JSON_BYTES + 1, path).expect_err("one byte more");
    assert!(error.contains("priorart reads up to"), "{error}");
}
