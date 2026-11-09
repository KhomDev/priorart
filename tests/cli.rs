//! End-to-end tests: drive the real `vg` and `viability-gate` binaries the way
//! the README tells a user to, and check what they print and how they exit.
//!
//! The unit and fixture tests exercise the engine. These exist because every
//! defect in the 2026-11-06 audit that the engine tests could not see was in
//! the command line: a documented command that did not parse, a flag that did
//! nothing, a crash on real input, an outcome no input could produce.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

const VG: &str = env!("CARGO_BIN_EXE_vg");
const VIABILITY_GATE: &str = env!("CARGO_BIN_EXE_viability-gate");

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// A fresh, empty directory for one test.
fn tmp() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("vg-cli-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn write(dir: &Path, name: &str, body: &str) -> PathBuf {
    let p = dir.join(name);
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(&p, body).unwrap();
    p
}

fn vg(args: &[&str], cwd: &Path) -> Output {
    Command::new(VG)
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("vg runs")
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).to_string()
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).to_string()
}

fn json(o: &Output) -> serde_json::Value {
    serde_json::from_slice(&o.stdout)
        .unwrap_or_else(|e| panic!("not JSON ({e}):\n{}\n{}", stdout(o), stderr(o)))
}

fn outcome_of(dir: &Path, name: &str, body: &str) -> (String, Vec<String>) {
    let f = write(dir, name, body);
    let o = vg(&["check", f.to_str().unwrap(), "--json"], dir);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    let v = json(&o);
    let rules = v["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["rule"].as_str().unwrap().to_string())
        .collect();
    (v["outcome"].as_str().unwrap().to_string(), rules)
}

// ------------------------------------------------------------ audit counterexamples

#[test]
fn valid_findings_from_the_audit_are_not_killed() {
    let dir = tmp();
    let cases = [
        (
            "missing-ac.md",
            "# Missing access control on setFee lets anyone drain fees\n\n## Description\nThe `setFee` function is missing the onlyOwner modifier, so any caller can set the fee to 100% and redirect all protocol fees to themselves.\n\n## Impact\nAn unprivileged attacker steals 12,500 USDC of accrued fees per day.\n",
        ),
        (
            "sandwich.md",
            "# Missing slippage check in swap allows sandwich attacks\n\n## Description\n`swapExactIn` passes amountOutMin = 0 to the router. A searcher can sandwich every user swap.\n\n## Impact\nUsers lose up to 30% of every swap; the attacker profits 4,000 USDC in our fork test.\n",
        ),
        (
            "tvl.md",
            "# Rounding bug lets attacker steal part of the total value locked\n\n## Summary\nThe attacker drains 1,200 USDC per call from the vault.\n\n## Proof of concept\nRun `forge test`. The attacker's balance increases by 1,200 USDC.\n\n## Recommendation\nThe function should revert when shares round to zero.\n",
        ),
        (
            "intended.md",
            "# Fee accrual breaks the intended behaviour of withdraw\n\n## Description\nContrary to the intended behaviour described in the whitepaper, withdraw charges the fee twice. The attacker profits 500 USDC per call at the users' expense.\n",
        ),
        (
            "curly.md",
            "# Report\n\n## Description\nThis path doesn\u{2019}t front-run anything; the attack is a single call.\n",
        ),
    ];
    for (name, body) in cases {
        let (outcome, rules) = outcome_of(&dir, name, body);
        assert_ne!(outcome, "KILL", "{name} was killed by {rules:?}");
        // P23-P26 are zero-weight notes about report structure. They are not
        // the false kills this regression guards against.
        let bad: Vec<_> = rules
            .iter()
            .filter(|r| !matches!(r.as_str(), "P23" | "P24" | "P25" | "P26"))
            .collect();
        assert!(bad.is_empty(), "{name} fired {bad:?}");
    }
}

#[test]
fn p21_no_longer_holds_on_its_own_advice() {
    let dir = tmp();
    let (_, rules) = outcome_of(
        &dir,
        "config.md",
        "# Oracle staleness\n\nThe attack requires no misconfiguration and works under the deployed configuration. An unprivileged attacker borrows 90,000 USDC against collateral worth 10,000 USDC.\n",
    );
    assert!(!rules.contains(&"P21".to_string()), "{rules:?}");
}

#[test]
fn p13_fires_on_an_unquantified_inconsistent_state() {
    let dir = tmp();
    let (outcome, rules) = outcome_of(
        &dir,
        "inconsistent.md",
        "# Inconsistent state after partial liquidation\n\n## Description\nAfter a partial liquidation the protocol enters an inconsistent state where the debt index and collateral index diverge. In 2 of the tested paths this happens.\n",
    );
    assert!(rules.contains(&"P13".to_string()), "{rules:?}");
    assert_eq!(outcome, "HOLD");
}

#[test]
fn an_epoch_heading_is_not_a_proof_section() {
    let dir = tmp();
    let (_, rules) = outcome_of(
        &dir,
        "epoch.md",
        "# Permanent loss of staking rewards\n\n## Epoch accounting\nThe epoch counter reverts to zero after rollover, so rewards for the last epoch are never distributed.\n",
    );
    assert!(!rules.contains(&"P19".to_string()), "{rules:?}");
}

// ------------------------------------------------------------ outcomes

fn full_inputs(dir: &Path, exclusion_pattern: &str) -> Vec<String> {
    let ledger = write(
        dir,
        "ledger.yaml",
        &format!(
            "- id: prog\n  tier_shape: low_and_above\n  floor_status: documented\n  floor_severity: low\n  source_url: \"https://example.org/rewards\"\n  retrieved: \"2026-11-01\"\n  exclusions:\n    - class: privileged_role\n      clause: \"Attacks requiring privileged access\"\n      pattern: \"{exclusion_pattern}\"\n  prior_audits:\n    - firm: X\n      url: \"https://example.org/a.pdf\"\n      covers: [\"src/Vault.sol\"]\n"
        ),
    );
    let scope = write(dir, "scope.txt", "src/Vault.sol\n");
    let prior = write(dir, "prior.txt", "audit-1\n");
    write(dir, "suite/Vault.t.sol", "// withdraw works\n");
    let hist = write(
        dir,
        "hist.yaml",
        "submissions:\n  - family: \"price oracle stale bound\"\n    outcome: rejected\n",
    );
    let ki = write(
        dir,
        "ki.yaml",
        "known_issues:\n  - id: KI-1\n    summary: dust\n    patterns: [\"dust.{0,20}stuck\"]\n",
    );
    vec![
        "--ledger".into(),
        ledger.to_string_lossy().into(),
        "--scope".into(),
        scope.to_string_lossy().into(),
        "--prior-audits".into(),
        prior.to_string_lossy().into(),
        "--tests".into(),
        dir.join("suite").to_string_lossy().into(),
        "--fork-of".into(),
        "none".into(),
        "--log".into(),
        hist.to_string_lossy().into(),
        "--known-issues".into(),
        ki.to_string_lossy().into(),
    ]
}

const CLEAN: &str = "# Reentrancy in withdraw lets an attacker drain the vault\n\n**Severity:** High\n\n## Description\nPreconditions: the vault holds deposits. `withdraw` in src/Vault.sol sends ETH before zeroing the balance. An unprivileged attacker re-enters. The victim depositor is forced to absorb the loss. Impact: 50 ETH.\n\nSeverity rule: \"High: direct theft of user funds\".\n";

#[test]
fn clear_is_reachable_when_every_gate_runs_and_passes() {
    let dir = tmp();
    let f = write(&dir, "clean.md", CLEAN);
    let mut args: Vec<String> = vec!["check".into(), f.to_string_lossy().into(), "--json".into()];
    args.extend(full_inputs(&dir, "(owner|admin) (can|must)"));
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let o = vg(&refs, &dir);
    let v = json(&o);
    let gates: Vec<String> = v["gates"]
        .as_array()
        .unwrap()
        .iter()
        .map(|g| format!("{} {} {}", g["gate"], g["status"], g["detail"]))
        .collect();
    assert_eq!(v["outcome"], "CLEAR", "{gates:#?}");
}

#[test]
fn a_failed_gate_holds_the_finding() {
    let dir = tmp();
    let f = write(&dir, "clean.md", CLEAN);
    let mut args: Vec<String> = vec!["check".into(), f.to_string_lossy().into(), "--json".into()];
    // This exclusion pattern matches the finding, so G2 fails.
    args.extend(full_inputs(&dir, "re-?enter"));
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let v = json(&vg(&refs, &dir));
    assert_eq!(v["outcome"], "HOLD");
    let g2 = v["gates"]
        .as_array()
        .unwrap()
        .iter()
        .find(|g| g["gate"] == "G2")
        .unwrap();
    assert_eq!(g2["status"], "fail", "{g2}");
}

#[test]
fn d1_runs_under_vg_check_with_a_corpus() {
    let dir = tmp();
    let f = write(
        &dir,
        "f.md",
        "Dust is left stuck in the router after every swap.\n",
    );
    let ki = write(
        &dir,
        "ki.yaml",
        "- id: KI-1\n  summary: dust\n  patterns: [\"dust.{0,20}stuck\"]\n",
    );
    let v = json(&vg(
        &[
            "check",
            f.to_str().unwrap(),
            "--known-issues",
            ki.to_str().unwrap(),
            "--json",
        ],
        &dir,
    ));
    assert_eq!(v["outcome"], "KILL");
    let d1 = v["gates"]
        .as_array()
        .unwrap()
        .iter()
        .find(|g| g["gate"] == "D1")
        .unwrap();
    assert_eq!(d1["status"], "fail");
}

// ------------------------------------------------------------ the README commands

#[test]
fn check_accepts_several_findings_and_fails_on_any_kill() {
    let dir = tmp();
    let a = write(
        &dir,
        "a.md",
        "An admin can set the pause flag and brick the vault.\n",
    );
    let b = write(&dir, "b.md", CLEAN);
    let o = vg(
        &[
            "check",
            a.to_str().unwrap(),
            b.to_str().unwrap(),
            "--json",
            "--min-score",
            "50",
            "--fail-on-kill",
        ],
        &dir,
    );
    assert_eq!(o.status.code(), Some(1), "{}", stderr(&o));
    let v = json(&o);
    assert_eq!(
        v.as_array().map(Vec::len),
        Some(2),
        "an array for several findings"
    );
    assert_eq!(v[0]["outcome"], "KILL");
}

#[test]
fn the_command_vg_init_prints_runs() {
    let dir = tmp();
    let o = vg(&["init", "t1"], &dir);
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    assert!(stdout(&o).contains("--known-issues"));
    let o = vg(
        &[
            "gates",
            "--ledger",
            "t1/ledger.yaml",
            "--scope",
            "t1/scope.txt",
            "--known-issues",
            "t1/known-issues.yaml",
            "--log",
            "t1/history.yaml",
        ],
        &dir,
    );
    assert_ne!(
        o.status.code(),
        Some(2),
        "a parse or usage error: {}",
        stderr(&o)
    );
    // The unfilled scaffold is meant to fail: the floor is uncited.
    assert_eq!(o.status.code(), Some(1));
}

#[test]
fn every_rule_explains_and_the_catalog_lists() {
    let dir = tmp();
    let o = vg(&["rules"], &dir);
    assert!(stdout(&o).contains("26 rules"));
    for n in 1..=26 {
        let id = format!("P{n}");
        let o = vg(&["explain", &id], &dir);
        assert_eq!(o.status.code(), Some(0), "{id}: {}", stderr(&o));
    }
}

#[test]
fn bench_uses_the_labels() {
    let dir = tmp();
    write(&dir, "rej/a.md", "An admin can set the pause flag.\n");
    write(&dir, "rej/b.md", "The attacker drains 5 ETH.\n");
    write(&dir, "acc/c.md", "The attacker drains 5 ETH atomically.\n");
    let labels = write(
        &dir,
        "labels.csv",
        "filename,label\na.md,rejected\nb.md,accepted\n",
    );
    let v = json(&vg(
        &[
            "bench",
            "--rejected",
            "rej",
            "--accepted",
            "acc",
            "--labels",
            labels.to_str().unwrap(),
            "--json",
        ],
        &dir,
    ));
    assert_eq!(
        v["rejected_total"], 1,
        "b.md moved to accepted by its label"
    );
    assert_eq!(v["accepted_total"], 2);
    let o = vg(&["bench", "--rejected", "rej"], &dir);
    assert!(
        stdout(&o).contains("n/a"),
        "no accepted set means no rate:\n{}",
        stdout(&o)
    );
}

#[test]
fn the_second_binary_names_itself() {
    let o = Command::new(VIABILITY_GATE)
        .arg("--version")
        .output()
        .unwrap();
    assert!(stdout(&o).starts_with("viability-gate "), "{}", stdout(&o));
}

// ------------------------------------------------------------ the own-history log

#[test]
fn log_add_survives_hostile_labels_and_log_check_finds_a_real_repeat() {
    let dir = tmp();
    for family in ["null", "true", "stale cached exchange rate"] {
        let o = vg(
            &[
                "log",
                "add",
                "--family",
                family,
                "--outcome",
                "rejected",
                "--file",
                "h.yaml",
            ],
            &dir,
        );
        assert_eq!(o.status.code(), Some(0), "{family}: {}", stderr(&o));
    }
    let draft = write(
        &dir,
        "draft.md",
        "# Stale cached exchange rate lets an attacker mint underpriced shares\n\nThe vault caches the exchange rate and refreshes it only when a deposit crosses the rebalance threshold, so the cached exchange rate goes stale and the attacker deposits at the stale rate.\n",
    );
    let o = vg(
        &["log", "check", draft.to_str().unwrap(), "--file", "h.yaml"],
        &dir,
    );
    assert_eq!(o.status.code(), Some(1), "{}{}", stdout(&o), stderr(&o));
    assert!(stdout(&o).starts_with("REPEAT"));
}

#[test]
fn log_add_rejects_a_bad_outcome() {
    let dir = tmp();
    let o = vg(
        &[
            "log",
            "add",
            "--family",
            "f",
            "--outcome",
            "won",
            "--file",
            "h.yaml",
        ],
        &dir,
    );
    assert_eq!(o.status.code(), Some(2));
    assert!(!dir.join("h.yaml").exists());
}

// ------------------------------------------------------------ input errors

#[test]
fn a_named_input_that_does_not_exist_is_an_error() {
    let dir = tmp();
    let f = write(&dir, "f.md", "text\n");
    for args in [
        vec!["check", f.to_str().unwrap(), "--log", "nope.yaml"],
        vec!["check", f.to_str().unwrap(), "--program", "p"],
        vec!["check", f.to_str().unwrap(), "--severity", "huge"],
        vec!["check", f.to_str().unwrap(), "--min-score", "101"],
    ] {
        let o = vg(&args, &dir);
        assert_eq!(o.status.code(), Some(2), "{args:?}: {}", stdout(&o));
    }
}

#[test]
fn a_rule_file_with_a_typo_is_rejected() {
    let dir = tmp();
    let rules = dir.join("rules");
    std::fs::create_dir_all(&rules).unwrap();
    let original = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("rules/anti-patterns.yaml"),
    )
    .unwrap()
    .replace("\r\n", "\n");
    let ap = original.replacen("    none_of:\n", "    none_off:\n", 1);
    assert_ne!(ap, original, "the fixture must actually introduce the typo");
    std::fs::write(rules.join("anti-patterns.yaml"), ap).unwrap();
    std::fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("rules/gates.yaml"),
        rules.join("gates.yaml"),
    )
    .unwrap();
    let o = vg(&["--rules", rules.to_str().unwrap(), "rules"], &dir);
    assert_eq!(o.status.code(), Some(2));
    assert!(stderr(&o).contains("none_off"), "{}", stderr(&o));
}

#[test]
fn a_large_multibyte_test_suite_does_not_crash() {
    let dir = tmp();
    let mut body = "a".repeat(3_999_999);
    body.push_str(&"é".repeat(10));
    write(&dir, "suite/a.txt", &body);
    let o = vg(&["gates", "--tests", "suite"], &dir);
    assert!(
        o.status.code() == Some(0) || o.status.code() == Some(1),
        "{}",
        stderr(&o)
    );
    assert!(!stderr(&o).contains("panicked"));
}

#[test]
fn control_characters_in_a_finding_never_reach_the_terminal() {
    let dir = tmp();
    let f = write(
        &dir,
        "f.md",
        "An admin can set the pause flag \u{1b}]0;pwned\u{7} and brick the vault.\n",
    );
    let o = vg(&["check", f.to_str().unwrap()], &dir);
    assert!(!stdout(&o).contains('\u{1b}'), "{}", stdout(&o));
    assert!(!stdout(&o).contains('\u{7}'));
}

#[test]
fn import_audit_writes_the_gate_inputs_and_match_ranks_identifiers() {
    let dir = tmp();
    let audit = write(
        &dir,
        "audit.yaml",
        "version: 1\nid: example-engagement\nseverity_floor: high\nexclusion_clauses:\n  - class: privileged_role\n    text: \"Attacks that require a privileged role\"\nknown_issues:\n  - id: M-1\n    summary: \"`reduce_outflow` credits only the current segment and `add_outflow` aborts\"\n    identifiers: [\"reduce_outflow\", \"add_outflow\"]\nscope:\n  - src/Vault.sol\n",
    );
    let out = dir.join("out");
    let o = vg(
        &[
            "import",
            "audit",
            audit.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
        ],
        &dir,
    );
    assert_eq!(o.status.code(), Some(0), "{}", stderr(&o));
    assert!(out.join("ledger.yaml").is_file());
    assert!(out.join("scope.txt").is_file());
    assert!(out.join("index.yaml").is_file());
    let ledger = std::fs::read_to_string(out.join("ledger.yaml")).unwrap();
    assert!(ledger.contains("high_and_above"), "{ledger}");
    assert!(
        !ledger.contains("pattern:"),
        "import must not invent a regex"
    );

    let finding = write(
        &dir,
        "draft.md",
        "An unprivileged attacker calls `reduce_outflow` and `add_outflow` in the same transaction. Impact: 3 tokens.\n",
    );
    let o = vg(
        &[
            "match",
            finding.to_str().unwrap(),
            "--index",
            out.join("index.yaml").to_str().unwrap(),
        ],
        &dir,
    );
    let text = stdout(&o);
    assert!(text.contains("reduce_outflow"), "{text}");
    assert!(
        o.status.code() == Some(0) || o.status.code() == Some(1),
        "{}",
        stderr(&o)
    );
}
