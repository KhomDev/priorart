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
