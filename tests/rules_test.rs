//! Integration test: every anti-pattern has a positive, a negative and (where a
//! defect was reproduced) a regression fixture.
//!
//! This test drives the REAL engine, not a re-implementation of it. An earlier
//! version mirrored the matching semantics in the test, which meant the test
//! could not catch a change in the engine it was supposed to be guarding.
//!
//! Run with: cargo test --release

use serde::Deserialize;
use std::collections::BTreeSet;
use viability_gate::engine::CompiledRule;
use viability_gate::model::{GateFile, RuleFile, Verdict};

const ANTI_PATTERNS: &str = include_str!("../rules/anti-patterns.yaml");
const GATES: &str = include_str!("../rules/gates.yaml");
const FIXTURES: &str = include_str!("rules.fixture.yaml");

#[derive(Deserialize)]
struct FixtureFile {
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    rule: String,
    fires: String,
    suppressed: String,
    #[serde(default)]
    regression: Option<String>,
    /// "fires" or "silent".
    #[serde(default)]
    regression_expect: Option<String>,
    /// Optional expected verdict for the regression case.
    #[serde(default)]
    regression_verdict: Option<String>,
}

fn compiled() -> Vec<CompiledRule> {
    let parsed: RuleFile =
        serde_saphyr::from_str(ANTI_PATTERNS).expect("anti-patterns.yaml parses");
    parsed
        .rules
        .into_iter()
        .map(|r| CompiledRule::new(r).expect("every rule compiles"))
        .collect()
}

fn load() -> (Vec<CompiledRule>, Vec<Case>) {
    let fixtures: FixtureFile = serde_saphyr::from_str(FIXTURES).expect("fixtures parse");
    (compiled(), fixtures.cases)
}

fn find<'a>(rules: &'a [CompiledRule], id: &str) -> Option<&'a CompiledRule> {
    rules.iter().find(|c| c.rule.id == id)
}

#[test]
fn catalog_has_22_rules() {
    let (rules, _) = load();
    assert_eq!(rules.len(), 22, "expected P1..P22");
    let ids: BTreeSet<&str> = rules.iter().map(|r| r.rule.id.as_str()).collect();
    for n in 1..=22 {
        let id = format!("P{n}");
        assert!(ids.contains(id.as_str()), "missing rule {id}");
    }
}

#[test]
fn every_rule_has_a_fixture() {
    let (rules, cases) = load();
    let covered: BTreeSet<&str> = cases.iter().map(|c| c.rule.as_str()).collect();
    let missing: Vec<&str> = rules
        .iter()
        .map(|r| r.rule.id.as_str())
        .filter(|id| !covered.contains(id))
        .collect();
    assert!(missing.is_empty(), "rules with no fixture: {missing:?}");
}

#[test]
fn every_rule_fires_on_its_positive_fixture() {
    let (rules, cases) = load();
    let mut failures = Vec::new();
    for case in &cases {
        let Some(rule) = find(&rules, &case.rule) else {
            failures.push(format!("{}: no such rule", case.rule));
            continue;
        };
        if rule.matches(&case.fires).is_none() {
            failures.push(format!("{}: did not fire on its positive case", case.rule));
        }
    }
    assert!(
        failures.is_empty(),
        "positive fixtures failed:\n{}",
        failures.join("\n")
    );
}

#[test]
fn every_rule_is_silent_on_its_negative_fixture() {
    let (rules, cases) = load();
    let mut failures = Vec::new();
    for case in &cases {
        let Some(rule) = find(&rules, &case.rule) else {
            continue;
        };
        if rule.matches(&case.suppressed).is_some() {
            failures.push(format!(
                "{}: fired on its NEGATIVE case (over-firing)",
                case.rule
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "negative fixtures failed:\n{}",
        failures.join("\n")
    );
}

/// The regression cases are the point of this file: each one is the exact text
/// that reproduced a reported defect. If a fix is ever reverted, this fails.
#[test]
fn regression_cases_hold() {
    let (rules, cases) = load();
    let mut failures = Vec::new();
    let mut checked = 0usize;
    for case in &cases {
        let Some(text) = case.regression.as_deref() else {
            continue;
        };
        checked += 1;
        let expect = case.regression_expect.as_deref().unwrap_or("silent");
        let Some(rule) = find(&rules, &case.rule) else {
            failures.push(format!("{}: no such rule", case.rule));
            continue;
        };
        let hit = rule.matches(text);
        match expect {
            "silent" => {
                if hit.is_some() {
                    failures.push(format!(
                        "{}: regression case must be SILENT but fired",
                        case.rule
                    ));
                }
            }
            "fires" => match hit {
                None => failures.push(format!(
                    "{}: regression case must FIRE but was silent",
                    case.rule
                )),
                Some(m) => {
                    if let Some(want) = case.regression_verdict.as_deref() {
                        let got = match m.verdict {
                            Verdict::Hard => "hard",
                            Verdict::Soft => "soft",
                            Verdict::Warn => "warn",
                        };
                        if got != want {
                            failures.push(format!(
                                "{}: regression verdict {got}, expected {want}",
                                case.rule
                            ));
                        }
                    }
                }
            },
            other => failures.push(format!("{}: bad regression_expect {other:?}", case.rule)),
        }
    }
    assert!(
        checked >= 8,
        "expected at least 8 regression cases, got {checked}"
    );
    assert!(
        failures.is_empty(),
        "regression fixtures failed:\n{}",
        failures.join("\n")
    );
}

#[test]
fn gates_are_eight_and_well_formed() {
    // G1..G7 plus D1. D1's input is the --known-issues corpus; it is declared
    // so the gate vocabulary here and the earliest_gate vocabulary in
    // rejection-taxonomy are the same id set.
    let g: GateFile = serde_saphyr::from_str(GATES).expect("gates.yaml parses");
    assert_eq!(g.version, viability_gate::model::CATALOG_VERSION);
    assert_eq!(g.gates.len(), 8, "expected G1..G7 plus D1");
    let ids: BTreeSet<&str> = g.gates.iter().map(|x| x.id.as_str()).collect();
    for n in 1..=7 {
        let id = format!("G{n}");
        assert!(ids.contains(id.as_str()), "missing gate {id}");
    }
    assert!(
        ids.contains("D1"),
        "missing gate D1 - the renamed duplicate gate"
    );
    let d1 = g.gates.iter().find(|x| x.id == "D1").expect("D1 exists");
    assert_eq!(d1.implemented_by.as_deref(), Some("--known-issues"));
    for gate in &g.gates {
        assert!(
            !gate.checks.is_empty(),
            "gate {} declares no checks",
            gate.id
        );
        for c in &gate.checks {
            assert!(!c.name.is_empty(), "gate {} has an unnamed check", gate.id);
            assert!(
                !c.test.is_empty(),
                "gate {} check {} has no test",
                gate.id,
                c.name
            );
            assert!(
                !c.fail.is_empty(),
                "gate {} check {} has no fail text",
                gate.id,
                c.name
            );
        }
    }
}

/// G6 is the only gate with no supporting rows. If that changes, the docs must
/// change with it, so this asserts the current honest state.
#[test]
fn gate_evidence_is_declared() {
    let g: GateFile = serde_saphyr::from_str(GATES).expect("gates.yaml parses");
    for gate in &g.gates {
        if gate.origin_rows == 0 {
            assert_eq!(
                gate.evidence.label(),
                "none",
                "gate {} claims rows but declares evidence none",
                gate.id
            );
        }
    }
    let g6 = g.gates.iter().find(|x| x.id == "G6").expect("G6 exists");
    assert_eq!(
        g6.evidence.label(),
        "none",
        "G6 has no rows in the author's log"
    );
}

/// The catalog must not have grown a private path or a target token. Ecosystem
/// tokens belong in rules/packs/, never inlined here.
#[test]
fn no_rule_names_a_target_or_a_private_path() {
    let banned = [
        "hackenproof",
        "immunefi",
        "cantina",
        "sherlock",
        "code4rena",
        "github.com/",
        "gitlab.com/",
        // Assembled from fragments for the same reason
        // scripts/check_tripwires.py does it: this test must not itself contain
        // the literals it forbids, or it is its own first violation.
        concat!("know", "ledge/"),
        concat!("_private", "-tools"),
        concat!("res", "ults/"),
        concat!("submission", "_guides/"),
    ];
    for (name, body) in [("anti-patterns.yaml", ANTI_PATTERNS), ("gates.yaml", GATES)] {
        let low = body.to_lowercase();
        for token in banned {
            assert!(
                !low.contains(token),
                "{name} contains a banned token: {token}"
            );
        }
    }
}

/// Every rule must carry provenance metadata, and a rule that claims rows must
/// declare them as in-sample. Nothing may claim a false-positive rate, because
/// no accepted/paid control set exists yet.
#[test]
fn rule_provenance_is_declared_and_fp_rate_is_unmeasured() {
    let (rules, _) = load();
    for c in &rules {
        let r = &c.rule;
        assert!(
            !r.doc.is_empty(),
            "{} has no doc page, and the old doctrine path is gone",
            r.id
        );
        assert!(
            r.doc.starts_with("docs/rules/"),
            "{} doc must point at a public page, got {:?}",
            r.id,
            r.doc
        );
        assert!(
            r.fp_rate.is_none(),
            "{} claims a false-positive rate, but no control set exists",
            r.id
        );
        if r.origin_rows == 0 {
            assert_eq!(
                r.evidence.label(),
                "none",
                "{} claims 0 rows but declares in_sample evidence",
                r.id
            );
        }
    }
}
