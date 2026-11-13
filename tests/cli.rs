//! Black-box tests for every command, flag and exit code.
//!
//! These run the real binary. Fixtures are either committed under examples/ and
//! tests/fixtures/, or written into a temp directory by the test.

mod support;

use serde_json::Value;
use std::path::Path;
use support::{manifest_dir, run, run_in, schema, validate, TempDir};

const DEMO: &str = "examples/dossier-demo";
const FINDING: &str = "examples/finding-demo.md";

/// Build the demo dossier inside a temp copy, returning (dir, dossier path).
fn demo_dossier(tmp: &TempDir) -> String {
    tmp.copy_dir(&manifest_dir().join(DEMO), "program");
    let out = tmp.join("demo.dossier");
    let r = run(&[
        "build",
        tmp.join("program").to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
    ]);
    assert_eq!(r.code, 0, "build failed: {} {}", r.stdout, r.stderr);
    out.to_str().unwrap().to_string()
}

fn demo_finding(tmp: &TempDir) -> String {
    std::fs::copy(manifest_dir().join(FINDING), tmp.join("finding.md")).unwrap();
    tmp.join("finding.md").to_str().unwrap().to_string()
}

// ---------------------------------------------------------------- surface

#[test]
fn version_and_help_are_final() {
    let v = run(&["--version"]);
    assert_eq!(v.code, 0);
    assert!(v.stdout.contains("priorart 0.1.1"), "{}", v.stdout);

    let h = run(&["--help"]);
    assert_eq!(h.code, 0);
    for command in [
        "build",
        "check",
        "ledger",
        "vocab",
        "init",
        "explain",
        "rejections",
        "rules",
    ] {
        assert!(h.stdout.contains(command), "help is missing {command}");
    }
    for sub in [
        vec!["build", "--help"],
        vec!["check", "--help"],
        vec!["ledger", "--help"],
        vec!["ledger", "validate", "--help"],
        vec!["vocab", "--help"],
        vec!["vocab", "list", "--help"],
        vec!["vocab", "show", "--help"],
        vec!["init", "--help"],
        vec!["explain", "--help"],
        vec!["rejections", "--help"],
        vec!["rejections", "summary", "--help"],
        vec!["rules", "--help"],
    ] {
        let r = run(&sub);
        assert_eq!(r.code, 0, "{sub:?} --help failed: {}", r.stderr);
        assert!(
            !r.stdout.trim().is_empty(),
            "{sub:?} --help printed nothing"
        );
    }
}

#[test]
fn unknown_command_is_a_usage_error() {
    let r = run(&["frobnicate"]);
    assert_eq!(r.code, 2, "{}", r.stderr);
}

#[test]
fn rules_prints_the_whole_surface() {
    let r = run(&["rules"]);
    assert_eq!(r.code, 0);
    for gate in ["G1", "G2", "G3", "G4"] {
        assert!(r.stdout.contains(gate), "{}", r.stdout);
    }
    assert!(r.stdout.contains("Exit codes"));
    assert!(r.stdout.contains("does NOT do"));
}

// ---------------------------------------------------------------- build

#[test]
fn build_writes_one_deterministic_dossier() {
    let tmp = TempDir::new("determinism");
    tmp.copy_dir(&manifest_dir().join(DEMO), "program");
    let dir = tmp.join("program");
    let first = tmp.join("one.dossier");
    let second = tmp.join("two.dossier");
    for out in [&first, &second] {
        let r = run(&[
            "build",
            dir.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
        ]);
        assert_eq!(r.code, 0, "{}", r.stderr);
    }
    let a = std::fs::read(&first).unwrap();
    let b = std::fs::read(&second).unwrap();
    assert_eq!(a, b, "two builds over one folder must be byte-identical");
    let text = String::from_utf8(a).unwrap();
    assert!(text.contains("priorart.dossier/v1"));
    assert!(
        !text.contains("timestamp"),
        "no wall-clock field in a dossier"
    );
}

#[test]
fn build_json_is_valid_against_the_shipped_schema() {
    let tmp = TempDir::new("build-json");
    tmp.copy_dir(&manifest_dir().join(DEMO), "program");
    let r = run(&["build", tmp.join("program").to_str().unwrap(), "--json"]);
    assert_eq!(r.code, 0, "{}", r.stderr);
    let dossier: Value = serde_json::from_str(&r.stdout).expect("build --json prints JSON");
    let errors = validate(&schema("dossier.schema.json"), &dossier);
    assert!(errors.is_empty(), "{errors:#?}");
    assert_eq!(dossier["schema"], "priorart.dossier/v1");
    assert!(dossier["items"].as_array().unwrap().len() > 3);
    // Every item quote must be a verbatim-ish excerpt: non-empty and inside the
    // text it came from after whitespace normalisation.
    for item in dossier["items"].as_array().unwrap() {
        let quote = item["quote"].as_str().unwrap();
        let text = item["text"].as_str().unwrap();
        let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
        assert!(
            flat.contains(
                quote
                    .trim_end_matches('\u{2026}')
                    .trim_start_matches('\u{2026}')
            ),
            "quote not found in text: {quote:?}"
        );
    }
}

#[test]
fn build_honours_out_and_program_override() {
    let tmp = TempDir::new("build-out");
    tmp.copy_dir(&manifest_dir().join(DEMO), "program");
    let out = tmp.join("named.dossier");
    let r = run(&[
        "build",
        tmp.join("program").to_str().unwrap(),
        "--program",
        "My Program!",
        "--out",
        out.to_str().unwrap(),
    ]);
    assert_eq!(r.code, 0, "{}", r.stderr);
    let dossier: Value = serde_json::from_str(&std::fs::read_to_string(&out).unwrap()).unwrap();
    assert_eq!(dossier["program"]["id"], "my-program");
}

#[test]
fn build_missing_and_empty_dirs_are_usage_errors() {
    let tmp = TempDir::new("build-empty");
    let missing = run(&["build", tmp.join("nope").to_str().unwrap()]);
    assert_eq!(missing.code, 2);
    assert!(
        missing.stderr.contains("not a directory"),
        "{}",
        missing.stderr
    );

    std::fs::create_dir_all(tmp.join("empty")).unwrap();
    let empty = run(&["build", tmp.join("empty").to_str().unwrap()]);
    assert_eq!(empty.code, 2);
    assert!(empty.stderr.contains("no inputs found"), "{}", empty.stderr);
}

#[test]
fn build_skips_the_audits_readme() {
    let tmp = TempDir::new("audits-readme");
    let dossier = demo_dossier(&tmp);
    let text = std::fs::read_to_string(&dossier).unwrap();
    assert!(
        !text.contains("audits/README.md"),
        "scaffolding must not be indexed as a prior audit"
    );
}

#[test]
fn a_pdf_without_extractable_text_is_a_warning_not_a_panic() {
    let tmp = TempDir::new("pdf");
    tmp.write(
        "program/scope.md",
        "# Rewards\n\n- High: $10,000\n\n## In scope\n\n- src/Vault.sol\n",
    );
    // A PDF whose text is in a compressed stream: the literal scrape finds
    // nothing, and pdftotext may be missing (as it is on the target machine).
    let mut pdf = b"%PDF-1.4\n1 0 obj\n<< /Length 8 /Filter /FlateDecode >>\nstream\n".to_vec();
    pdf.extend_from_slice(&[0x78, 0x9c, 0x01, 0x02, 0x00, 0xff, 0xfe, 0x01, 0x02]);
    pdf.extend_from_slice(b"\nendstream\nendobj\ntrailer\n<<>>\n%%EOF\n");
    tmp.write_bytes("program/audits/prior.pdf", &pdf);
    let r = run(&["build", tmp.join("program").to_str().unwrap()]);
    assert_eq!(r.code, 0, "a bad PDF must not fail the build: {}", r.stderr);
    let combined = format!("{}{}", r.stdout, r.stderr);
    assert!(
        combined.contains("no extractable text")
            || combined.contains("pdftotext returned no text")
            || combined.contains("pdftotext failed"),
        "expected a clear PDF warning, got: {combined}"
    );
    assert!(!combined.to_lowercase().contains("panicked"));
}

// ---------------------------------------------------------------- check

#[test]
fn check_prints_three_labelled_sections_with_a_whole_sentence_quote() {
    let tmp = TempDir::new("check-text");
    let dossier = demo_dossier(&tmp);
    let finding = demo_finding(&tmp);
    let r = run(&["check", &finding, "--dossier", &dossier]);
    assert_eq!(r.code, 0, "{}", r.stderr);
    for section in ["HAS THIS BEEN FOUND?", "IS IT EXCLUDED?", "CAN IT BE PAID?"] {
        assert!(
            r.stdout.contains(section),
            "missing {section}: {}",
            r.stdout
        );
    }
    assert!(
        r.stdout
            .contains("reading list only. priorart does not decide duplicates"),
        "retrieval-only framing is missing: {}",
        r.stdout
    );
    assert!(
        r.stdout.contains("to read (share") && r.stdout.contains(">=2 symbol identifiers)"),
        "reading-list headline is missing: {}",
        r.stdout
    );
    assert!(
        !r.stdout.contains("0/64") && !r.stdout.contains("43.3%"),
        "benchmark numbers must not be printed by the tool: {}",
        r.stdout
    );
    // D1: the quote is the whole sentence, not a fixed-length cut.
    assert!(
        r.stdout
            .contains("when the fee index rounds up past the collected balance"),
        "quote was truncated: {}",
        r.stdout
    );
    assert!(!r.stdout.contains("FeeLedger\"\n"), "{}", r.stdout);
    // D2: "no role" must not become a privileged precondition.
    assert!(!r.stdout.contains("privileged_role"), "{}", r.stdout);
    // D3/D4: coverage and scope are recognised by contract name.
    assert!(
        r.stdout.contains("cited identifier LoanBook"),
        "{}",
        r.stdout
    );
    assert!(
        r.stdout.contains("LoanBook (in src/LoanBook.sol)"),
        "{}",
        r.stdout
    );
}

#[test]
fn check_json_is_valid_against_the_shipped_schema() {
    let tmp = TempDir::new("check-json");
    let dossier = demo_dossier(&tmp);
    let finding = demo_finding(&tmp);
    let r = run(&["check", &finding, "--dossier", &dossier, "--json"]);
    assert_eq!(r.code, 0, "{}", r.stderr);
    let report: Value = serde_json::from_str(&r.stdout).unwrap();
    let errors = validate(&schema("check.schema.json"), &report);
    assert!(errors.is_empty(), "{errors:#?}");
    assert_eq!(report["schema"], "priorart.check/v1");
    assert_eq!(report["tool_version"], "0.1.1");
    assert_eq!(report["payout"]["payable"], true);
    // D5: "No oracle ... is needed" must not become a mechanism claim. The demo
    // finding has its own mechanism (fees left unsettled on close), so the check
    // is that the class is that one and never the oracle class.
    assert_eq!(
        report["finding"]["mechanism_class"], "accounting_order_of_operations",
        "{}",
        report
    );
    assert_ne!(
        report["finding"]["mechanism_class"], "oracle_bounds_or_staleness",
        "{}",
        report
    );
    for item in report["prior_art"].as_array().unwrap() {
        assert!(item["shared_identifiers"].as_array().unwrap().len() >= 2);
        assert!(item["page"].as_u64().unwrap() >= 1);
    }
}

#[test]
fn check_top_k_is_bounded() {
    let tmp = TempDir::new("topk");
    let dossier = demo_dossier(&tmp);
    let finding = demo_finding(&tmp);
    let ok = run(&["check", &finding, "--dossier", &dossier, "--top-k", "1"]);
    assert_eq!(ok.code, 0, "{}", ok.stderr);
    for bad in ["0", "51"] {
        let r = run(&["check", &finding, "--dossier", &dossier, "--top-k", bad]);
        assert_eq!(r.code, 2, "--top-k {bad} should be a usage error");
        assert!(r.stderr.contains("--top-k"), "{}", r.stderr);
    }
}

#[test]
fn check_discovers_the_only_dossier_beside_the_finding() {
    let tmp = TempDir::new("discover");
    let dossier = demo_dossier(&tmp);
    let _ = dossier;
    let finding = demo_finding(&tmp);
    // Both finding and dossier in the temp dir: discovery must find it.
    let r = run(&["check", &finding]);
    assert_eq!(r.code, 0, "{}", r.stderr);
}

#[test]
fn check_without_a_dossier_is_a_usage_error() {
    let tmp = TempDir::new("no-dossier");
    tmp.write("finding.md", "# A finding\n");
    let r = run(&["check", tmp.join("finding.md").to_str().unwrap()]);
    assert_eq!(r.code, 2);
    assert!(r.stderr.contains("no dossier"), "{}", r.stderr);
}

#[test]
fn check_rejects_a_malformed_or_foreign_dossier() {
    let tmp = TempDir::new("bad-dossier");
    let finding = demo_finding(&tmp);
    let malformed = tmp.write("bad.dossier", "{not json");
    let r = run(&["check", &finding, "--dossier", malformed.to_str().unwrap()]);
    assert_eq!(r.code, 2, "{}", r.stdout);
    assert!(r.stderr.contains("not a dossier JSON"), "{}", r.stderr);

    let foreign = tmp.write("foreign.dossier", "{\"schema\":\"something.else/v9\"}");
    let r = run(&["check", &finding, "--dossier", foreign.to_str().unwrap()]);
    assert_eq!(r.code, 2);
    assert!(
        r.stderr.contains("is not priorart.dossier/v1"),
        "{}",
        r.stderr
    );
}

#[test]
fn check_missing_finding_is_a_usage_error() {
    let tmp = TempDir::new("missing-finding");
    let dossier = demo_dossier(&tmp);
    let r = run(&[
        "check",
        tmp.join("nope.md").to_str().unwrap(),
        "--dossier",
        &dossier,
    ]);
    assert_eq!(r.code, 2);
    assert!(r.stderr.contains("cannot read finding"), "{}", r.stderr);
}

fn write_finding(tmp: &TempDir, name: &str, body: &str) -> String {
    tmp.write(name, body).to_str().unwrap().to_string()
}

#[test]
fn fail_on_excluded_exits_one_only_when_a_clause_matches() {
    let tmp = TempDir::new("fail-excluded");
    let dossier = demo_dossier(&tmp);
    let finding = write_finding(
        &tmp,
        "excluded.md",
        "# Operator can freeze distributions\n\nSeverity: High\n\n## Summary\n\nThe operator can call `LoanBook.close` with a stale reserve so `FeeLedger.distributeFees` reverts for every depositor.\n",
    );
    let plain = run(&["check", &finding, "--dossier", &dossier]);
    assert_eq!(
        plain.code, 0,
        "evidence is not a verdict by default: {}",
        plain.stderr
    );
    assert!(plain.stdout.contains("privileged_role"), "{}", plain.stdout);
    let strict = run(&[
        "check",
        &finding,
        "--dossier",
        &dossier,
        "--fail-on-excluded",
    ]);
    assert_eq!(strict.code, 1, "{}", strict.stdout);
}

#[test]
fn fail_on_unpayable_exits_one_below_the_floor() {
    let tmp = TempDir::new("fail-unpayable");
    let dossier = demo_dossier(&tmp);
    let finding = write_finding(
        &tmp,
        "low.md",
        "# Rounding drift in share accounting\n\nSeverity: Low\n\n## Summary\n\nAny user can shift `FeeLedger.distributeFees` rounding by one wei per call in `src/LoanBook.sol`.\n",
    );
    let plain = run(&["check", &finding, "--dossier", &dossier]);
    assert_eq!(plain.code, 0, "{}", plain.stderr);
    assert!(plain.stdout.contains("below the floor"), "{}", plain.stdout);
    let strict = run(&[
        "check",
        &finding,
        "--dossier",
        &dossier,
        "--fail-on-unpayable",
    ]);
    assert_eq!(strict.code, 1, "{}", strict.stdout);
}

#[test]
fn unicode_and_crlf_findings_are_handled() {
    let tmp = TempDir::new("encodings");
    let dossier = demo_dossier(&tmp);
    let unicode = write_finding(
        &tmp,
        "unicode.md",
        "# Réserve — trésorerie\n\nSeverity: High\n\nLe défaut est dans `FeeLedger.distributeFees` où la réserve « gelée » dépasse le plafond.\n",
    );
    let r = run(&["check", &unicode, "--dossier", &dossier]);
    assert_eq!(r.code, 0, "{}", r.stderr);
    assert!(
        r.stdout.contains("FeeLedger.distributeFees"),
        "the finding's identifiers must survive non-ASCII text: {}",
        r.stdout
    );

    let lf = "# CRLF finding\n\nSeverity: High\n\n`FeeLedger.distributeFees` and `FeeAccountingMismatch` in `src/LoanBook.sol`.\n";
    let crlf = lf.replace('\n', "\r\n");
    let lf_path = write_finding(&tmp, "lf.md", lf);
    let crlf_path = write_finding(&tmp, "crlf.md", &crlf);
    let a = run(&["check", &lf_path, "--dossier", &dossier, "--json"]);
    let b = run(&["check", &crlf_path, "--dossier", &dossier, "--json"]);
    assert_eq!(a.code, 0, "{}", a.stderr);
    assert_eq!(b.code, 0, "{}", b.stderr);
    let ja: Value = serde_json::from_str(&a.stdout).unwrap();
    let jb: Value = serde_json::from_str(&b.stdout).unwrap();
    assert_eq!(ja["finding"]["identifiers"], jb["finding"]["identifiers"]);
    assert_eq!(ja["prior_art"], jb["prior_art"]);
}

#[test]
fn a_finding_with_no_identifiers_ranks_nothing_and_says_so() {
    let tmp = TempDir::new("no-idents");
    let dossier = demo_dossier(&tmp);
    let finding = write_finding(
        &tmp,
        "prose.md",
        "The protocol has a bug in the accounting somewhere that nobody can quite describe.\n",
    );
    let r = run(&["check", &finding, "--dossier", &dossier]);
    assert_eq!(r.code, 0);
    assert!(
        r.stdout.contains("no prior item can be ranked"),
        "{}",
        r.stdout
    );
    assert!(
        r.stdout.contains("fewer than 2 symbol identifiers"),
        "{}",
        r.stdout
    );
}

// ---------------------------------------------------------------- ledger

const LEDGER: &str = "examples/ledger.example.yaml";
const SHAPES: &str = "examples/tier-shapes.example.yaml";

#[test]
fn ledger_validate_accepts_the_shipped_examples() {
    let r = run(&["ledger", "validate", LEDGER]);
    assert_eq!(r.code, 0, "{} {}", r.stdout, r.stderr);
    assert!(r.stdout.contains("1 entry, 0 error(s)"), "{}", r.stdout);
    assert!(r.stdout.contains("placeholder host"), "{}", r.stdout);

    let both = run(&["ledger", "validate", LEDGER, SHAPES]);
    assert_eq!(both.code, 0, "{}", both.stdout);
    assert!(both.stdout.contains("6 entries"), "{}", both.stdout);
}

#[test]
fn ledger_validate_json_has_the_python_validators_keys() {
    let r = run(&["ledger", "validate", "--json", LEDGER, SHAPES]);
    assert_eq!(r.code, 0, "{}", r.stderr);
    let report: Value = serde_json::from_str(&r.stdout).unwrap();
    for key in [
        "schema_version",
        "ok",
        "strict",
        "ledgers",
        "entries",
        "errors",
        "warnings",
    ] {
        assert!(report.get(key).is_some(), "missing key {key}");
    }
    assert_eq!(report["schema_version"], 1);
    assert_eq!(report["ok"], true);
    assert_eq!(report["entries"], 6);
    assert_eq!(report["ledgers"].as_array().unwrap().len(), 2);
    assert!(
        report.get("ledger").is_none(),
        "multi-file runs must not set ledger"
    );
    assert!(report["warnings"].as_array().unwrap().len() >= 10);
}

#[test]
fn ledger_validate_reports_errors_and_exits_one() {
    let tmp = TempDir::new("ledger-errors");
    let bad = tmp.write(
        "bad.yaml",
        "- id: real-program\n  tier_shape: critical_only\n  floor_status: documented\n  floor_severity: critical\n  source_url: https://example.invalid/rewards\n  retrieved: 2026-10-01\n  exclusionz: []\n",
    );
    let r = run(&["ledger", "validate", bad.to_str().unwrap()]);
    assert_eq!(r.code, 1, "{} {}", r.stdout, r.stderr);
    assert!(r.stderr.contains("exclusionz"), "{}", r.stderr);
    assert!(r.stderr.contains("placeholder host"), "{}", r.stderr);

    let json = run(&["ledger", "validate", "--json", bad.to_str().unwrap()]);
    assert_eq!(json.code, 1);
    let report: Value = serde_json::from_str(&json.stdout).unwrap();
    assert_eq!(report["ok"], false);
    assert!(!report["errors"].as_array().unwrap().is_empty());
}

#[test]
fn ledger_validate_is_strict_on_request() {
    let r = run(&["ledger", "validate", "--strict", LEDGER]);
    assert_eq!(r.code, 1, "{}", r.stdout);
    let json = run(&["ledger", "validate", "--json", "--strict", LEDGER]);
    assert_eq!(json.code, 1);
    let report: Value = serde_json::from_str(&json.stdout).unwrap();
    assert_eq!(report["ok"], false);
    assert!(report["errors"].as_array().unwrap().is_empty());
    assert!(!report["warnings"].as_array().unwrap().is_empty());
}

#[test]
fn ledger_validate_rejects_malformed_and_missing_inputs() {
    let tmp = TempDir::new("ledger-bad");
    let broken = tmp.write("broken.yaml", "- id: [unclosed\n");
    let r = run(&["ledger", "validate", broken.to_str().unwrap()]);
    assert_eq!(r.code, 2, "{} {}", r.stdout, r.stderr);

    let two = tmp.write(
        "two.yaml",
        "- id: one\n  tier_shape: unknown\n  floor_status: unstated\n  source_url: https://docs.real-program.org/one\n  retrieved: \"2026-10-01\"\n---\n- id: two\n  tier_shape: unknown\n  floor_status: unstated\n  source_url: https://docs.real-program.org/two\n  retrieved: \"2026-10-01\"\n",
    );
    let r = run(&["ledger", "validate", "--json", two.to_str().unwrap()]);
    assert_eq!(r.code, 2, "{} {}", r.stdout, r.stderr);
    assert!(
        format!("{}{}", r.stdout, r.stderr).contains("single YAML document"),
        "{} {}",
        r.stdout,
        r.stderr
    );

    let missing = run(&[
        "ledger",
        "validate",
        tmp.join("nope.yaml").to_str().unwrap(),
    ]);
    assert_eq!(missing.code, 2);
    assert!(missing.stderr.contains("not found"), "{}", missing.stderr);

    let none = run(&["ledger", "validate"]);
    assert_eq!(none.code, 2);
}

#[test]
fn ledger_taxonomy_json_has_five_and_fourteen() {
    let r = run(&["ledger", "validate", "--taxonomy", "--json"]);
    assert_eq!(r.code, 0, "{}", r.stderr);
    let payload: Value = serde_json::from_str(&r.stdout).unwrap();
    assert_eq!(payload["tier_shapes"]["count"], 5);
    assert_eq!(payload["clause_classes"]["count"], 14);
    assert_eq!(payload["tier_shapes"]["items"].as_array().unwrap().len(), 5);
    assert_eq!(
        payload["clause_classes"]["items"].as_array().unwrap().len(),
        14
    );
}

#[test]
fn the_ledger_schema_is_the_shipped_v1_contract() {
    let s = schema("ledger.schema.json");
    assert_eq!(s["title"], "Private program ledger (schema v1)");
    assert_eq!(s["type"], "array");
    let shapes = s["items"]["properties"]["tier_shape"]["enum"]
        .as_array()
        .unwrap();
    assert_eq!(shapes.len(), 5, "five tier shapes, no 'any'");
    let classes = s["items"]["properties"]["exclusions"]["items"]["properties"]["class"]["enum"]
        .as_array()
        .unwrap();
    assert_eq!(classes.len(), 14, "fourteen clause classes");
    assert_eq!(s["items"]["additionalProperties"], false);
}

// ---------------------------------------------------------------- vocab / explain

#[test]
fn vocab_list_has_five_shapes_and_fourteen_classes() {
    let r = run(&["vocab", "list"]);
    assert_eq!(r.code, 0);
    assert!(r.stdout.contains("tier shapes (5)"), "{}", r.stdout);
    assert!(r.stdout.contains("clause classes (14)"), "{}", r.stdout);
    for id in [
        "privileged_role",
        "dos_no_extraction",
        "scope_boundary",
        "critical_only",
        "low_and_above",
    ] {
        assert!(r.stdout.contains(id), "missing {id}");
    }
}

#[test]
fn vocab_show_and_json_agree() {
    for name in ["tier-shapes", "clause-classes", "gate-crossref"] {
        let text = run(&["vocab", "show", name]);
        assert_eq!(text.code, 0, "{}: {}", name, text.stderr);
        let json = run(&["vocab", "show", name, "--json"]);
        assert_eq!(json.code, 0, "{}: {}", name, json.stderr);
        let value: Value = serde_json::from_str(&json.stdout).unwrap();
        assert!(value.is_object(), "{name} json");
    }
    let classes: Value =
        serde_json::from_str(&run(&["vocab", "show", "clause-classes", "--json"]).stdout).unwrap();
    assert_eq!(classes["classes"].as_array().unwrap().len(), 14);
    let shapes: Value =
        serde_json::from_str(&run(&["vocab", "show", "tier-shapes", "--json"]).stdout).unwrap();
    assert_eq!(shapes["shapes"].as_array().unwrap().len(), 5);

    let list = run(&["vocab", "list", "--json"]);
    assert_eq!(list.code, 0);
    let value: Value = serde_json::from_str(&list.stdout).unwrap();
    assert_eq!(value["tier_shapes"]["count"], 5);
    assert_eq!(value["clause_classes"]["count"], 14);

    let unknown = run(&["vocab", "show", "nope"]);
    assert_eq!(unknown.code, 2);
    assert!(unknown.stderr.contains("tier-shapes"), "{}", unknown.stderr);
}

#[test]
fn explain_covers_a_clause_a_gate_and_a_shape() {
    let clause = run(&["explain", "privileged_role"]);
    assert_eq!(clause.code, 0);
    assert!(clause.stdout.contains("escape:"), "{}", clause.stdout);
    let gate = run(&["explain", "G2"]);
    assert_eq!(gate.code, 0);
    assert!(gate.stdout.contains("Is it excluded?"), "{}", gate.stdout);
    let shape = run(&["explain", "medium_and_above"]);
    assert_eq!(shape.code, 0);
    assert!(shape.stdout.contains("pays:"), "{}", shape.stdout);
    let unknown = run(&["explain", "nope"]);
    assert_eq!(unknown.code, 2);
    assert!(unknown.stderr.contains("vocab list"), "{}", unknown.stderr);
}

// ---------------------------------------------------------------- init / rejections

#[test]
fn init_scaffolds_and_refuses_to_overwrite_without_force() {
    let tmp = TempDir::new("init");
    let r = run_in(
        &manifest_dir(),
        &["init", tmp.join("prog").to_str().unwrap()],
    );
    assert_eq!(r.code, 0, "{}", r.stderr);
    for rel in [
        "program.yaml",
        "scope.md",
        "known-issues.md",
        "audits/README.md",
    ] {
        assert!(tmp.join(&format!("prog/{rel}")).is_file(), "{rel} missing");
    }
    // The scaffold must build and check without edits.
    let build = run(&["build", tmp.join("prog").to_str().unwrap()]);
    assert_eq!(build.code, 0, "{}", build.stderr);

    let again = run(&["init", tmp.join("prog").to_str().unwrap()]);
    assert_eq!(again.code, 2);
    assert!(again.stderr.contains("--force"), "{}", again.stderr);
    let forced = run(&["init", tmp.join("prog").to_str().unwrap(), "--force"]);
    assert_eq!(forced.code, 0, "{}", forced.stderr);
}

#[test]
fn rejections_summary_reports_the_shipped_dataset() {
    let r = run(&["rejections", "summary"]);
    assert_eq!(r.code, 0);
    assert!(r.stdout.contains("113 logged submissions"), "{}", r.stdout);
    assert!(r.stdout.contains("110 published"), "{}", r.stdout);
    assert!(r.stdout.contains("55.8"), "{}", r.stdout);
    assert!(r.stdout.contains("one researcher's log"), "{}", r.stdout);

    let json = run(&["rejections", "summary", "--json"]);
    assert_eq!(json.code, 0);
    let payload: Value = serde_json::from_str(&json.stdout).unwrap();
    assert_eq!(payload["schema"], "priorart.rejections-summary/v1");
    assert_eq!(payload["dataset"]["total"], 113);
    assert_eq!(payload["dataset"]["published_rows"], 110);
    assert_eq!(payload["provenance"]["dataset_version"], "0.6.1");
    assert_eq!(payload["provenance"]["data_identical_to"], "0.6.0");
}

// ---------------------------------------------------------------- offline guarantee

#[test]
fn the_crate_has_no_http_client() {
    let manifest = std::fs::read_to_string(manifest_dir().join("Cargo.toml")).unwrap();
    for banned in [
        "reqwest",
        "ureq",
        "hyper",
        "isahc",
        "curl",
        "surf",
        "attohttpc",
        "minreq",
        "awc",
    ] {
        assert!(
            !manifest.contains(banned),
            "Cargo.toml gained an HTTP client: {banned}"
        );
    }
    let lock = std::fs::read_to_string(manifest_dir().join("Cargo.lock")).unwrap();
    for banned in [
        "reqwest",
        "ureq",
        "hyper",
        "isahc",
        "attohttpc",
        "minreq",
        "tokio",
    ] {
        assert!(
            !lock.contains(&format!("name = \"{banned}\"")),
            "Cargo.lock gained a network crate: {banned}"
        );
    }
    // And no source file opens a socket.
    let mut checked = 0;
    for entry in walk(&manifest_dir().join("src")) {
        let text = std::fs::read_to_string(&entry).unwrap();
        for banned in ["TcpStream", "UdpSocket", "std::net::"] {
            assert!(!text.contains(banned), "{} uses {banned}", entry.display());
        }
        checked += 1;
    }
    assert!(checked > 10, "walked only {checked} source files");
}

fn walk(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            out.extend(walk(&path));
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push(path);
        }
    }
    out
}
