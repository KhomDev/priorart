//! Adversarial inputs: a finding, a program folder or a dossier that is trying
//! to fool priorart, or to break it.
//!
//! Each test states what the tool SHOULD do. Every case here passes as of
//! 0.1.3; the cases that were once `#[ignore]`d bugs (the double negation, the
//! hand-edited quote, the out-of-range page) are now regressions if they fail.
//!
//! Every fixture is fictional. The tests run the real binary, as tests/cli.rs
//! does.

#[allow(dead_code)]
mod support;

use serde_json::{json, Value};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use support::{manifest_dir, run, Run, TempDir};

const CHECK_TOP_LEVEL_KEYS: [&str; 8] = [
    "schema",
    "tool_version",
    "finding",
    "dossier",
    "prior_art",
    "exclusions",
    "payout",
    "notes",
];

/// Write program files under `<tmp>/program` and build a dossier from them.
fn build_program(tmp: &TempDir, files: &[(&str, &str)]) -> (Run, PathBuf) {
    for (rel, body) in files {
        tmp.write(&format!("program/{rel}"), body);
    }
    let out = tmp.join("out.dossier");
    let r = run(&[
        "build",
        tmp.join("program").to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
    ]);
    (r, out)
}

/// Copy the committed demo folder into `<tmp>/program` and build it.
fn build_demo(tmp: &TempDir) -> PathBuf {
    tmp.copy_dir(&manifest_dir().join("examples/dossier-demo"), "program");
    let out = tmp.join("demo.dossier");
    let r = run(&[
        "build",
        tmp.join("program").to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
    ]);
    assert_eq!(r.code, 0, "demo build failed: {} {}", r.stdout, r.stderr);
    out
}

/// Write a finding and run `check --json` against a dossier.
fn check_json(tmp: &TempDir, finding_body: &str, dossier: &Path) -> (Run, Value) {
    let finding = tmp.write("finding.md", finding_body);
    let r = run(&[
        "check",
        finding.to_str().unwrap(),
        "--dossier",
        dossier.to_str().unwrap(),
        "--json",
    ]);
    let value = serde_json::from_str(&r.stdout).unwrap_or(Value::Null);
    (r, value)
}

fn read_dossier(path: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn write_dossier(path: &Path, value: &Value) {
    std::fs::write(path, serde_json::to_string_pretty(value).unwrap()).unwrap();
}

/// Run the binary and kill it if it runs past `limit`. Returns the run and the
/// elapsed time. A run past the limit fails the test.
fn run_with_deadline(args: &[&str], limit: Duration) -> (Run, Duration) {
    let start = Instant::now();
    let mut child = Command::new(env!("CARGO_BIN_EXE_priorart"))
        .args(args)
        .current_dir(manifest_dir())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn priorart");
    loop {
        match child.try_wait().expect("poll child") {
            Some(status) => {
                let mut stdout = String::new();
                let mut stderr = String::new();
                if let Some(mut s) = child.stdout.take() {
                    let _ = s.read_to_string(&mut stdout);
                }
                if let Some(mut s) = child.stderr.take() {
                    let _ = s.read_to_string(&mut stderr);
                }
                let run = Run {
                    code: status.code().unwrap_or(-1),
                    stdout,
                    stderr,
                };
                return (run, start.elapsed());
            }
            None => {
                if start.elapsed() > limit {
                    let _ = child.kill();
                    let _ = child.wait();
                    panic!("priorart {args:?} still running after {limit:?}");
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    }
}

/// True when the run ended with a Rust panic rather than a handled error.
fn panicked(r: &Run) -> bool {
    r.stderr.contains("panicked") || r.code == 101
}

// ------------------------------------------------------------ finding stuffing

#[test]
fn a_finding_stuffed_with_every_dossier_identifier_is_ranked_but_still_a_reading_list() {
    let tmp = TempDir::new("adv-stuff");
    let dossier_path = build_demo(&tmp);
    let dossier = read_dossier(&dossier_path);

    // Every identifier any item holds, backticked, in one finding.
    let mut all: Vec<String> = Vec::new();
    for item in dossier["items"].as_array().unwrap() {
        for id in item["identifiers"].as_array().unwrap() {
            let s = id.as_str().unwrap().to_string();
            if !all.contains(&s) {
                all.push(s);
            }
        }
    }
    assert!(
        all.len() >= 3,
        "demo dossier should hold several identifiers"
    );
    let stuffed = format!(
        "# Stuffed finding\n\n**Severity:** High\n\n## Summary\n\n{}\n",
        all.iter()
            .map(|s| format!("`{s}`"))
            .collect::<Vec<_>>()
            .join(" ")
    );

    let (r, report) = check_json(&tmp, &stuffed, &dossier_path);
    assert_eq!(r.code, 0, "check failed: {}", r.stderr);
    assert!(!panicked(&r));

    // It can rank items, but never more than the default top-k bound.
    let ranked = report["prior_art"]["ranked"]
        .as_array()
        .or_else(|| report["prior_art"].as_array())
        .expect("prior_art lists ranked items");
    assert!(
        !ranked.is_empty(),
        "the stuffed finding should have forced a ranked match, or the test proves nothing"
    );
    assert!(
        ranked.len() <= 5,
        "ranked {} items, top-k is 5",
        ranked.len()
    );

    // The report has no verdict field: it is the same eight keys as always.
    let mut keys: Vec<&str> = report
        .as_object()
        .unwrap()
        .keys()
        .map(|k| k.as_str())
        .collect();
    keys.sort();
    let mut expected = CHECK_TOP_LEVEL_KEYS.to_vec();
    expected.sort();
    assert_eq!(keys, expected, "check/v1 gained or lost a top-level key");

    // The text output still says what it is.
    let text = run(&[
        "check",
        tmp.join("finding.md").to_str().unwrap(),
        "--dossier",
        dossier_path.to_str().unwrap(),
    ]);
    assert!(
        text.stdout.contains("reading list only"),
        "text output lost the reading-list line:\n{}",
        text.stdout
    );
}

// --------------------------------------------------------- negation tricks

#[test]
fn an_exclusion_clause_is_quoted_whole_including_its_negations() {
    let tmp = TempDir::new("adv-neg-clause");
    let (r, path) = build_program(
        &tmp,
        &[(
            "scope.md",
            "# Example scope\n\n## Out of scope\n\n- Issues that need a privileged role are not excluded unless the owner key is compromised.\n",
        )],
    );
    assert_eq!(r.code, 0, "{}", r.stderr);
    let (r, report) = check_json(
        &tmp,
        "# Finding\n\nThe attacker is the owner and uses the privileged role.\n",
        &path,
    );
    assert_eq!(r.code, 0);
    let ex = report["exclusions"].as_array().unwrap();
    assert!(
        ex.iter().any(|e| e["clause_class"] == "privileged_role"),
        "the role clause should be shown: {report}"
    );
    let clause = ex[0]["clause"].as_str().unwrap();
    assert!(
        clause.contains("not excluded unless the owner key is compromised"),
        "the clause was shortened or rewritten: {clause}"
    );
}

#[test]
fn a_finding_that_denies_a_role_does_not_trip_the_role_exclusion() {
    let tmp = TempDir::new("adv-neg-role");
    let dossier = build_demo(&tmp);
    let (r, report) = check_json(
        &tmp,
        "# Finding\n\nAttacker is any user. No privileged role, owner or governance call is needed.\n",
        &dossier,
    );
    assert_eq!(r.code, 0);
    let ex = report["exclusions"].as_array().unwrap();
    assert!(
        !ex.iter().any(|e| e["clause_class"] == "privileged_role"),
        "a finding that denies a role must not be read as needing one: {report}"
    );
}

#[test]
fn a_double_negation_in_the_finding_is_read_as_the_positive_it_states() {
    let tmp = TempDir::new("adv-double-neg");
    let dossier = build_demo(&tmp);
    // "not without a role" means "has a role". The role clause should be tripped.
    let (r, report) = check_json(
        &tmp,
        "# Finding\n\nThe attacker is not without a privileged role in this design.\n",
        &dossier,
    );
    assert_eq!(r.code, 0);
    let ex = report["exclusions"].as_array().unwrap();
    assert!(
        ex.iter().any(|e| e["clause_class"] == "privileged_role"),
        "double negation should still show the role clause: {report}"
    );
}

// ------------------------------------------------------ malformed inputs

#[test]
fn a_program_yaml_with_wrong_types_is_an_error_not_a_panic() {
    let cases = [
        "payout: high\n",
        "payout:\n  tier_shape: 7\n  floor_severity: [1, 2]\n",
        "prior_audits: none\n",
        "name:\n  - a\n  - b\nurl: {x: 1}\n",
        "::: not yaml :::\n\t- [\n",
    ];
    for (i, body) in cases.iter().enumerate() {
        let tmp = TempDir::new(&format!("adv-yaml-types-{i}"));
        let (r, _) = build_program(
            &tmp,
            &[
                ("scope.md", "## In scope\n\n- src/LoanBook.sol\n"),
                ("program.yaml", body),
            ],
        );
        assert!(!panicked(&r), "case {i} panicked: {}", r.stderr);
        assert!(
            r.code == 0 || r.code == 2,
            "case {i}: exit {} is neither a success nor a usage error",
            r.code
        );
    }
}

#[test]
fn unknown_program_yaml_keys_are_ignored_and_the_build_succeeds() {
    let tmp = TempDir::new("adv-yaml-unknown");
    let (r, path) = build_program(
        &tmp,
        &[
            ("scope.md", "## In scope\n\n- src/LoanBook.sol\n"),
            (
                "program.yaml",
                "name: Example\nunexpected_block:\n  nested: [1, 2, 3]\nmore: true\n",
            ),
        ],
    );
    assert_eq!(r.code, 0, "{}", r.stderr);
    let dossier = read_dossier(&path);
    assert_eq!(dossier["program"]["name"], "Example");
}

#[test]
fn a_yaml_alias_bomb_finishes_in_bounded_time_without_a_panic() {
    // Each level refers to the one above nine times. Fully expanded, the top
    // level is 9^8 leaves: a parser without an alias budget never returns.
    let mut body =
        String::from("l0: &l0 [\"x\", \"x\", \"x\", \"x\", \"x\", \"x\", \"x\", \"x\", \"x\"]\n");
    for level in 1..=7 {
        let prev = level - 1;
        let refs = (0..9)
            .map(|_| format!("*l{prev}"))
            .collect::<Vec<_>>()
            .join(", ");
        body.push_str(&format!("l{level}: &l{level} [{refs}]\n"));
    }
    body.push_str("payout: *l7\n");

    let tmp = TempDir::new("adv-alias-bomb");
    tmp.write("program/scope.md", "## In scope\n\n- src/LoanBook.sol\n");
    tmp.write("program/program.yaml", &body);
    let out = tmp.join("bomb.dossier");
    let (r, elapsed) = run_with_deadline(
        &[
            "build",
            tmp.join("program").to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
        ],
        Duration::from_secs(30),
    );
    assert!(!panicked(&r), "alias bomb panicked: {}", r.stderr);
    assert!(
        r.code == 0 || r.code == 2,
        "alias bomb: exit {} (stderr {})",
        r.code,
        r.stderr
    );
    eprintln!("alias bomb: exit {} in {elapsed:?}", r.code);
}

// -------------------------------------------------------- hand-edited dossiers

#[test]
fn a_dossier_with_a_foreign_schema_id_is_rejected() {
    let tmp = TempDir::new("adv-schema-id");
    let path = build_demo(&tmp);
    let mut d = read_dossier(&path);
    d["schema"] = json!("priorart.dossier/v2");
    write_dossier(&path, &d);
    let (r, _) = check_json(&tmp, "# Finding\n\nSomething.\n", &path);
    assert_eq!(r.code, 2, "stdout {} stderr {}", r.stdout, r.stderr);
    assert!(r.stderr.contains("schema"), "{}", r.stderr);
}

#[test]
fn a_hand_edited_quote_that_is_in_no_source_is_flagged() {
    let tmp = TempDir::new("adv-fake-quote");
    let path = build_demo(&tmp);
    let mut d = read_dossier(&path);
    d["items"][0]["quote"] = json!("The owner may drain every vault at any time.");
    write_dossier(&path, &d);
    let (r, report) = check_json(
        &tmp,
        "# Finding\n\nLoanBook.close lets a user close.\n",
        &path,
    );
    assert_eq!(r.code, 0);
    let notes = report["notes"].to_string();
    assert!(
        notes.contains("source")
            && (notes.contains("not found") || notes.contains("does not match")),
        "the report should say the quote is not in any source: {notes}"
    );
}

#[test]
fn a_page_number_past_the_source_page_count_is_flagged() {
    let tmp = TempDir::new("adv-page");
    let path = build_demo(&tmp);
    let mut d = read_dossier(&path);
    d["items"][0]["page"] = json!(999);
    write_dossier(&path, &d);
    let (r, _) = check_json(
        &tmp,
        "# Finding\n\nLoanBook.close lets a user close.\n",
        &path,
    );
    // Refusing the dossier (exit 2) or flagging the page in the report are both
    // acceptable. Silently reading page 999 of a one-page source is the failure.
    assert_ne!(
        r.code, 0,
        "a page 999 in a one-page source was read as current"
    );
}

// -------------------------------------------------------- source handling

#[test]
fn two_sources_with_identical_content_keep_distinct_ids_and_one_sha() {
    let tmp = TempDir::new("adv-same-sha");
    let body = "# Internal review\n\n## Finding 1\n\n`LoanBook.close` lets any user close a loan without the fee settlement.\n";
    let (r, path) = build_program(
        &tmp,
        &[
            ("scope.md", "## In scope\n\n- src/LoanBook.sol\n"),
            ("audits/first.md", body),
            ("audits/second.md", body),
        ],
    );
    assert_eq!(r.code, 0, "{}", r.stderr);
    let d = read_dossier(&path);
    let audit_shas: Vec<&str> = d["sources"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| s["kind"] == "audit")
        .map(|s| s["sha256"].as_str().unwrap())
        .collect();
    assert_eq!(audit_shas.len(), 2, "both audits should be recorded");
    assert_eq!(audit_shas[0], audit_shas[1], "identical bytes, one digest");

    let mut ids: Vec<&str> = d["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["id"].as_str().unwrap())
        .collect();
    let before = ids.len();
    ids.sort();
    ids.dedup();
    assert_eq!(
        ids.len(),
        before,
        "item ids must stay unique across sources"
    );
}

#[test]
fn a_pdf_whose_text_is_drawn_as_images_is_listed_unread_and_the_build_succeeds() {
    // A page that paints one image and no text operators: a scanned page.
    let pdf: &[u8] = b"%PDF-1.4
1 0 obj << /Type /Catalog /Pages 2 0 R >> endobj
2 0 obj << /Type /Pages /Kids [3 0 R] /Count 1 >> endobj
3 0 obj << /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Contents 4 0 R /Resources << /XObject << /Im0 5 0 R >> >> >> endobj
4 0 obj << /Length 30 >> stream
q 200 0 0 200 0 0 cm /Im0 Do Q
endstream endobj
5 0 obj << /Type /XObject /Subtype /Image /Width 1 /Height 1 /ColorSpace /DeviceGray /BitsPerComponent 8 /Length 1 >> stream
x
endstream endobj
trailer << /Root 1 0 R >>
%%EOF
";
    let tmp = TempDir::new("adv-image-pdf");
    tmp.write_bytes("program/audits/scan.pdf", pdf);
    tmp.write("program/scope.md", "## In scope\n\n- src/LoanBook.sol\n");
    let out = tmp.join("pdf.dossier");
    let r = run(&[
        "build",
        tmp.join("program").to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
    ]);
    assert!(!panicked(&r), "image PDF panicked: {}", r.stderr);
    assert_eq!(r.code, 0, "{}", r.stderr);
    let combined = format!("{}{}", r.stdout, r.stderr);
    assert!(
        combined.contains("scan.pdf"),
        "an image-only PDF must be named as unread, never skipped silently:\n{combined}"
    );
}

// ------------------------------------------------------ look-alikes

#[test]
fn a_unicode_look_alike_identifier_does_not_match_the_ascii_one() {
    let tmp = TempDir::new("adv-homoglyph");
    let dossier = build_demo(&tmp);
    // Cyrillic 'о' (U+043E) in place of the Latin 'o' in LoanBook.
    let fake = "# Finding\n\nThe `LoanB\u{043E}ok.close` path lets any user close a loan.\n";
    let (r, report) = check_json(&tmp, fake, &dossier);
    assert_eq!(r.code, 0);
    let ids: Vec<&str> = report["finding"]["identifiers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert!(
        !ids.contains(&"LoanBook.close"),
        "a homoglyph was read as the ASCII identifier: {ids:?}"
    );
}

// ------------------------------------------------------ terminal safety

#[test]
fn terminal_control_characters_in_a_quote_do_not_reach_stdout() {
    let tmp = TempDir::new("adv-control");
    let quote = "`LoanBook.close` and `FeeLedger.distributeFees` let \u{1b}[2J\u{1b}]0;owned\u{7} any user \u{9b}31m close a loan.";
    let (r, path) = build_program(
        &tmp,
        &[
            ("scope.md", "## In scope\n\n- src/LoanBook.sol\n"),
            ("audits/review.md", &format!("# Review\n\n{quote}\n")),
        ],
    );
    assert_eq!(r.code, 0, "{}", r.stderr);
    assert!(!r.stdout.contains('\u{1b}'), "build summary carries ESC");

    let finding_path = tmp.write(
        "finding-text.md",
        "# Finding\n\n`LoanBook.close` and `FeeLedger.distributeFees` let any user close a loan.\n",
    );
    let text = run(&[
        "check",
        finding_path.to_str().unwrap(),
        "--dossier",
        path.to_str().unwrap(),
    ]);
    assert_eq!(text.code, 0);
    // The quote must actually be printed, or the check below proves nothing.
    assert!(
        text.stdout.contains("close a loan"),
        "the hostile quote was not printed, so the control check is vacuous:\n{}",
        text.stdout
    );
    for bad in ['\u{1b}', '\u{7}', '\u{9b}'] {
        assert!(
            !text.stdout.contains(bad),
            "control character U+{:04X} reached check's stdout",
            bad as u32
        );
    }
}
