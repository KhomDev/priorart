//! Black-box tests for `priorart taxonomy classify`.
//!
//! The fixtures are synthetic: a composite finding, a clean finding whose
//! classes are all negated or anti-cued, and no third-party text. The tests run
//! the real binary and check the JSON schema, the text output, determinism, the
//! input limits and the UTF-8 boundary.

#[allow(dead_code)]
mod support;

use priorart::taxonomy::MAX_TEXT_BYTES;
use serde_json::Value;
use support::{manifest_dir, run, TempDir};

fn path(rel: &str) -> String {
    manifest_dir().join(rel).to_string_lossy().to_string()
}

fn composite() -> String {
    path("tests/fixtures/taxonomy/finding-composite.md")
}

fn clean() -> String {
    path("tests/fixtures/taxonomy/finding-clean.md")
}

fn negated() -> String {
    path("tests/fixtures/taxonomy/finding-negated.md")
}

fn classes<'a>(set: &'a Value, key: &str) -> Vec<&'a str> {
    set[key]
        .as_array()
        .unwrap_or_else(|| panic!("{key} is not an array: {set}"))
        .iter()
        .map(|m| m["class"].as_str().expect("class is a string"))
        .collect()
}

#[test]
fn classify_json_has_the_schema_and_the_composite_classes() {
    let r = run(&["taxonomy", "classify", &composite(), "--json"]);
    assert_eq!(r.code, 0, "{} {}", r.stdout, r.stderr);
    let report: Value = serde_json::from_str(&r.stdout).expect("classify JSON");
    assert_eq!(report["schema"], "priorart.taxonomy-classify/v1");
    assert_eq!(report["taxonomy_version"], 1);
    assert_eq!(report["taxonomy_source"], "vocab/taxonomy.yaml");
    let results = report["results"].as_array().expect("results");
    assert_eq!(results.len(), 1);
    assert_eq!(
        results[0]["file"].as_str().unwrap(),
        composite().replace('\\', "/")
    );
    assert!(results[0]["text_sha256"].as_str().unwrap().len() == 64);
    assert!(results[0]["text_bytes"].as_u64().unwrap() > 0);

    let mechanism = &results[0]["mechanism"];
    assert_eq!(mechanism["schema"], "priorart.taxonomy/v1");
    assert_eq!(mechanism["abstain_reason"], Value::Null);
    assert!(classes(mechanism, "root_causes").contains(&"reentrancy"));
    assert!(classes(mechanism, "impacts").contains(&"fund-loss"));
    assert!(classes(mechanism, "assets").contains(&"specific-token"));

    // Every span is an exact byte range of the input holding the term.
    let text = std::fs::read_to_string(composite()).unwrap();
    for key in ["root_causes", "impacts", "assets"] {
        for m in mechanism[key].as_array().unwrap() {
            let start = m["span"]["start_byte"].as_u64().unwrap() as usize;
            let end = m["span"]["end_byte"].as_u64().unwrap() as usize;
            assert_eq!(&text[start..end], m["term"].as_str().unwrap());
        }
    }
}

#[test]
fn classify_text_output_names_each_section() {
    let r = run(&["taxonomy", "classify", &composite()]);
    assert_eq!(r.code, 0, "{}", r.stderr);
    assert!(r.stdout.contains("MECHANISM reentrancy"), "{}", r.stdout);
    assert!(r.stdout.contains("IMPACT    fund-loss"), "{}", r.stdout);
    assert!(
        r.stdout.contains("ASSET     specific-token"),
        "{}",
        r.stdout
    );
    assert!(!r.stdout.contains("ABSTAIN"), "{}", r.stdout);

    let clean = run(&["taxonomy", "classify", &clean()]);
    assert_eq!(clean.code, 0, "{}", clean.stderr);
    assert!(
        clean.stdout.contains("ABSTAIN   no-mechanism"),
        "{}",
        clean.stdout
    );
}

#[test]
fn negated_and_anti_cued_fixtures_abstain() {
    for file in [clean(), negated()] {
        let r = run(&["taxonomy", "classify", &file, "--json"]);
        assert_eq!(r.code, 0, "{} {}", r.stdout, r.stderr);
        let report: Value = serde_json::from_str(&r.stdout).unwrap();
        let mechanism = &report["results"][0]["mechanism"];
        assert_eq!(
            mechanism["abstain_reason"], "no-mechanism",
            "{file}: {report}"
        );
        for key in ["root_causes", "impacts", "assets"] {
            assert!(
                mechanism[key].as_array().unwrap().is_empty(),
                "{file}: {key} should be empty: {report}"
            );
        }
    }
}

#[test]
fn classify_is_byte_identical_across_runs() {
    let composite = composite();
    for flags in [vec!["--json"], vec![]] {
        let mut args = vec!["taxonomy", "classify", composite.as_str()];
        args.extend(flags);
        let first = run(&args);
        let second = run(&args);
        assert_eq!(first.code, 0, "{}", first.stderr);
        assert_eq!(second.code, 0, "{}", second.stderr);
        assert_eq!(first.stdout, second.stdout, "output must be byte-identical");
        assert_eq!(first.stderr, second.stderr);
    }
}

#[test]
fn several_files_keep_their_argument_order() {
    let r = run(&[
        "taxonomy",
        "classify",
        &clean(),
        &composite(),
        &negated(),
        "--json",
    ]);
    assert_eq!(r.code, 0, "{}", r.stderr);
    let report: Value = serde_json::from_str(&r.stdout).unwrap();
    let results = report["results"].as_array().unwrap();
    assert_eq!(results.len(), 3);
    assert!(results[0]["file"]
        .as_str()
        .unwrap()
        .ends_with("finding-clean.md"));
    assert!(results[1]["file"]
        .as_str()
        .unwrap()
        .ends_with("finding-composite.md"));
    assert!(results[2]["file"]
        .as_str()
        .unwrap()
        .ends_with("finding-negated.md"));
}

#[test]
fn a_missing_file_is_a_usage_error() {
    let tmp = TempDir::new("taxonomy-missing");
    let r = run(&[
        "taxonomy",
        "classify",
        tmp.join("nope.md").to_str().unwrap(),
    ]);
    assert_eq!(r.code, 2, "{}", r.stdout);
    assert!(r.stderr.contains("cannot read text"), "{}", r.stderr);
}

#[test]
fn a_directory_is_a_usage_error() {
    let tmp = TempDir::new("taxonomy-dir");
    let r = run(&["taxonomy", "classify", tmp.path().to_str().unwrap()]);
    assert_eq!(r.code, 2, "{}", r.stdout);
    assert!(r.stderr.contains("regular files"), "{}", r.stderr);
}

#[test]
fn no_file_is_a_usage_error() {
    let r = run(&["taxonomy", "classify"]);
    assert_eq!(r.code, 2, "{}", r.stdout);
    assert!(r.stderr.contains("FILE"), "{}", r.stderr);
}

#[test]
fn non_utf8_text_is_a_usage_error_not_a_panic() {
    let tmp = TempDir::new("taxonomy-utf8");
    let bad = tmp.write_bytes("bad.md", &[0xff, 0xfe, 0x00, 0x41, 0x80]);
    let r = run(&["taxonomy", "classify", bad.to_str().unwrap()]);
    assert_eq!(r.code, 2, "{} {}", r.stdout, r.stderr);
    assert!(r.stderr.contains("UTF-8"), "{}", r.stderr);
    assert!(
        !r.stderr.to_lowercase().contains("panicked"),
        "{}",
        r.stderr
    );
    assert!(r.stdout.is_empty(), "no partial result: {}", r.stdout);
}

#[test]
fn text_over_the_cap_is_a_usage_error() {
    let tmp = TempDir::new("taxonomy-over-cap");
    let big = tmp.write_bytes("big.md", &vec![b'a'; MAX_TEXT_BYTES + 1]);
    let r = run(&["taxonomy", "classify", big.to_str().unwrap()]);
    assert_eq!(r.code, 2, "{} {}", r.stdout, r.stderr);
    assert!(r.stderr.contains("16 MiB"), "{}", r.stderr);
    assert!(
        !r.stderr.to_lowercase().contains("panicked"),
        "{}",
        r.stderr
    );
}

#[test]
fn text_at_the_cap_does_not_panic() {
    let tmp = TempDir::new("taxonomy-at-cap");
    let big = tmp.write_bytes("cap.md", &vec![b'a'; MAX_TEXT_BYTES]);
    let r = run(&["taxonomy", "classify", big.to_str().unwrap(), "--json"]);
    assert_eq!(r.code, 0, "{}", r.stderr);
    assert!(
        !r.stderr.to_lowercase().contains("panicked"),
        "{}",
        r.stderr
    );
    let report: Value = serde_json::from_str(&r.stdout).expect("classify JSON at the cap");
    assert_eq!(
        report["results"][0]["mechanism"]["abstain_reason"],
        "no-mechanism"
    );
    assert_eq!(
        report["results"][0]["text_bytes"].as_u64().unwrap() as usize,
        MAX_TEXT_BYTES
    );
}
