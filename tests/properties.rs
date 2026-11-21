//! Property tests: invariants that must hold for every generated input, not
//! just the examples. Each property states the invariant it checks.
//!
//! Generated cases run in-process through the library, which is fast enough
//! for hundreds of cases. The binary's own output is checked separately, on a
//! few fixed inputs, by the black-box tests in tests/cli.rs.

#[allow(dead_code)]
mod support;

use priorart::check;
use priorart::dossier::build;
use priorart::matching::{self, Ranked, DEFAULT_TOP_K, MAX_TOP_K};
use priorart::model::{Dossier, Item};
use proptest::prelude::*;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use support::{manifest_dir, TempDir};

/// A proptest config with the given case count. Failure persistence is off:
/// the regression file it would write is not wanted in the repository.
fn config(cases: u32) -> ProptestConfig {
    ProptestConfig {
        cases,
        failure_persistence: None,
        ..ProptestConfig::default()
    }
}

/// A 64-digit hex digest, the shape the check schema requires.
const FAKE_SHA: &str = "00000000000000000000000000000000000000000000000000000000000000ff";

/// The committed demo dossier, parsed once.
static DEMO: LazyLock<Dossier> = LazyLock::new(|| {
    let text =
        std::fs::read_to_string(manifest_dir().join("examples/dossier-demo/dossier-demo.dossier"))
            .expect("read demo dossier");
    serde_json::from_str(&text).expect("demo dossier parses")
});

// ----------------------------------------------------------- shared fixtures

/// Files of a small program folder, by relative path.
const PROGRAM_FILES: [(&str, &str); 6] = [
    (
        "scope.md",
        "# Scope\n\n## In scope\n\n- src/LoanBook.sol\n- src/FeeLedger.sol\n\n## Out of scope\n\n- Attacks that require a privileged role.\n",
    ),
    (
        "known-issues.md",
        "# Known issues\n\n- SwapDesk.swapIn can reach its cap during high volume.\n",
    ),
    ("program.yaml", "name: Example Lend\nplatform: ExamplePlatform\n"),
    (
        "audits/a.md",
        "# Review A\n\n## Finding 1\n\n`LoanBook.close` lets any user close a loan.\n",
    ),
    (
        "audits/b.md",
        "# Review B\n\n## Finding 1\n\n`FeeLedger.distributeFees` reverts after `LoanBook.close`.\n",
    ),
    (
        "audits/c.md",
        "# Review C\n\n## Finding 2\n\n`SwapDesk.swapIn` checks a cap that stuck fees count against.\n",
    ),
];

/// Write the program files in `order` into a fresh `program` folder, then
/// build a dossier from it. The folder name is fixed because it becomes the
/// program id when program.yaml names none. Returns the dossier bytes.
fn build_in_order(order: &[usize]) -> Vec<u8> {
    let tmp = TempDir::new("prop-build-order");
    let root = tmp.join("program");
    std::fs::create_dir_all(root.join("audits")).expect("mkdir");
    for &i in order {
        let (rel, body) = PROGRAM_FILES[i];
        std::fs::write(root.join(rel), body).expect("write program file");
    }
    let out = tmp.join("out.dossier");
    build::build(&root, Some(&out), None).expect("build succeeds");
    std::fs::read(&out).expect("read dossier")
}

/// Build from a folder whose single audit is named `audit_name`, returning the
/// dossier with the source path and title removed, so only the name's case is
/// left out of the comparison.
fn build_named_audit(audit_name: &str) -> Value {
    let tmp = TempDir::new("prop-build-case");
    let root = tmp.join("program");
    std::fs::create_dir_all(root.join("audits")).expect("mkdir");
    std::fs::write(root.join("scope.md"), PROGRAM_FILES[0].1).expect("scope");
    std::fs::write(root.join("program.yaml"), PROGRAM_FILES[2].1).expect("program");
    std::fs::write(root.join("audits").join(audit_name), PROGRAM_FILES[3].1).expect("audit");
    let out = tmp.join("out.dossier");
    build::build(&root, Some(&out), None).expect("build succeeds");
    let mut value: Value = serde_json::from_slice(&std::fs::read(&out).unwrap()).unwrap();
    // The file name is recorded as the source path and title, and as the
    // item title. Those three are the only fields a name's case may change.
    for source in value["sources"].as_array_mut().unwrap() {
        source.as_object_mut().unwrap().remove("path");
        source.as_object_mut().unwrap().remove("title");
    }
    for item in value["items"].as_array_mut().unwrap() {
        item.as_object_mut().unwrap().remove("title");
    }
    value
}

/// A dossier item with the given identifiers, in the fields the matcher reads.
fn item(index: usize, identifiers: &[String]) -> Item {
    Item {
        id: format!("item-{:04}", index + 1),
        source_id: "src-0001".to_string(),
        kind: "audit-finding".to_string(),
        title: Some("audit.md".to_string()),
        section: None,
        page: 1,
        quote: "generated".to_string(),
        identifiers: identifiers.to_vec(),
        mechanism_class: None,
        clause_class: None,
        text: "generated".to_string(),
    }
}

/// Qualified names drawn from a small shared pool, so items and findings
/// overlap often. Each is a symbol the matcher counts.
fn shared_symbol(n: usize) -> String {
    format!("Pool{}.call{}", n / 3, n % 3)
}

/// Qualified names no finding ever names.
fn unrelated_symbol(n: usize) -> String {
    format!("Unrelated{}.run{}", n / 3, n % 3)
}

/// Item identifiers: one to four shared symbols.
fn item_identifiers() -> impl Strategy<Value = Vec<String>> {
    proptest::collection::vec(0usize..9, 1..=4)
        .prop_map(|ns| ns.into_iter().map(shared_symbol).collect())
}

/// A finding's text: two to four shared symbols, backticked, in prose.
fn finding_text() -> impl Strategy<Value = String> {
    proptest::collection::vec(0usize..9, 2..=4).prop_map(|ns| {
        let names: Vec<String> = ns
            .into_iter()
            .map(|n| format!("`{}`", shared_symbol(n)))
            .collect();
        format!(
            "# Finding\n\nThe attack uses {} and no privileged role.\n",
            names.join(" and ")
        )
    })
}

fn items_strategy() -> impl Strategy<Value = Vec<Vec<String>>> {
    proptest::collection::vec(item_identifiers(), 1..=12)
}

/// Larger dossiers, so a shared symbol can sit near the generic threshold.
fn dossier_strategy() -> impl Strategy<Value = Vec<Vec<String>>> {
    proptest::collection::vec(item_identifiers(), 1..=30)
}

/// The top-k rows as (item index, shared identifiers): the part of a ranking
/// a reader sees.
fn top_rows(ranked: &[Ranked]) -> Vec<(usize, Vec<String>)> {
    ranked
        .iter()
        .map(|r| (r.index, r.shared_identifiers.clone()))
        .collect()
}

// --------------------------------------------------------- property 1: build

proptest! {
    #![proptest_config(config(24))]

    /// The dossier bytes do not depend on the order in which the files were
    /// created, which is the order a directory listing can come back in.
    #[test]
    fn build_is_byte_identical_under_any_file_creation_order(
        order in Just((0..PROGRAM_FILES.len()).collect::<Vec<usize>>()).prop_shuffle()
    ) {
        let canonical: Vec<usize> = (0..PROGRAM_FILES.len()).collect();
        let baseline = build_in_order(&canonical);
        let permuted = build_in_order(&order);
        prop_assert_eq!(baseline, permuted, "creation order {:?} changed the dossier", order);
    }

    /// The case of an audit's file name changes the recorded path, the source
    /// title and the item title, and nothing else. The item ids, quotes,
    /// identifiers and the rest of the dossier are the same for every case.
    #[test]
    fn build_is_unchanged_by_the_case_of_a_file_name(
        flips in proptest::collection::vec(any::<bool>(), 15)
    ) {
        let stem = "internal-review";
        let cased: String = stem
            .chars()
            .zip(flips.iter().cycle())
            .map(|(c, up)| if *up && c.is_ascii_alphabetic() { c.to_ascii_uppercase() } else { c.to_ascii_lowercase() })
            .collect();
        let name = format!("{cased}.md");
        let lower = build_named_audit("internal-review.md");
        let other = build_named_audit(&name);
        prop_assert_eq!(lower, other, "the name {} changed more than its recorded path", name);
    }
}

// ------------------------------------------------------- property 2: check

proptest! {
    #![proptest_config(config(128))]

    /// `check` output is the same on every run for the same input, and it
    /// validates against the shipped check/v1 schema.
    #[test]
    fn check_json_is_identical_across_runs_and_validates(
        ns in proptest::collection::vec(0usize..9, 0..=6),
        text in finding_text(),
        extra in prop_oneof![
            Just(String::new()),
            Just("Severity: High\n".to_string()),
            Just("Attacker is the owner, a privileged role.\n".to_string()),
            Just("This needs a front-run and an oracle price feed.\n".to_string()),
        ],
    ) {
        let mut body = text;
        body.push_str(&extra);
        body.push_str(&ns.iter().map(|n| format!("`{}` ", shared_symbol(*n))).collect::<String>());

        let path = PathBuf::from("finding.md");
        let dossier_path = Path::new("demo.dossier");
        let render = || {
            let report = check::build(&path, &body, FAKE_SHA, dossier_path, &DEMO, DEFAULT_TOP_K);
            priorart::render::json::pretty(&report).expect("render")
        };
        let first = render();
        let second = render();
        prop_assert_eq!(&first, &second, "two runs over one input differ");

        let value: Value = serde_json::from_str(&first).expect("output is JSON");
        let errors = support::validate(&support::schema("check.schema.json"), &value);
        prop_assert!(errors.is_empty(), "check output fails the schema: {:?}", errors);
    }
}

// ------------------------------------------------------- property 3: ranking

proptest! {
    #![proptest_config(config(128))]

    /// Adding an item that shares no identifier with the finding never changes
    /// the top-k. The new item is appended, so existing indices are unchanged.
    /// The generic-symbol share is measured over the items that share at least
    /// one symbol with the finding, so an unrelated item cannot move it.
    #[test]
    fn adding_an_unrelated_item_never_changes_the_top_k(
        items in dossier_strategy(),
        text in finding_text(),
        unrelated in proptest::collection::vec(0usize..9, 1..=4),
    ) {
        let base: Vec<Item> = items.iter().enumerate().map(|(i, ids)| item(i, ids)).collect();
        let extra_ids: Vec<String> = unrelated.into_iter().map(unrelated_symbol).collect();
        let mut grown = base.clone();
        grown.push(item(base.len(), &extra_ids));

        let before = top_rows(&matching::rank(&base, &text, DEFAULT_TOP_K));
        let after = top_rows(&matching::rank(&grown, &text, DEFAULT_TOP_K));
        prop_assert_eq!(before, after);
    }
}

/// A deterministic companion to the property: two items share the finding's
/// two symbols and rank; appending items that share none of them leaves the
/// ranked rows exactly as they were. Before the denominator was changed to the
/// relevant items, enough appended noise pushed a generic symbol under the 20%
/// share and made previously filtered items rank.
#[test]
fn unrelated_items_do_not_re_rank_a_nonempty_top_k() {
    let mut items: Vec<Item> = Vec::new();
    for i in 0..2 {
        items.push(item(
            i,
            &["Pool0.call0".to_string(), "Pool1.call0".to_string()],
        ));
    }
    for i in 2..10 {
        items.push(item(i, &[format!("Filler{i}.call{i}")]));
    }
    let finding = "# Finding\n\n`Pool0.call0` and `Pool1.call0` both fail.\n";
    let before = top_rows(&matching::rank(&items, finding, DEFAULT_TOP_K));
    assert!(!before.is_empty(), "the fixture must rank something");

    for i in 10..20 {
        items.push(item(i, &[format!("Unrelated{i}.run{i}")]));
    }
    let after = top_rows(&matching::rank(&items, finding, DEFAULT_TOP_K));

    assert_eq!(before, after, "unrelated items changed the ranked rows");
}

proptest! {
    #![proptest_config(config(128))]

    /// The ranking is deterministic and its prefix is stable: the top k is the
    /// first k of the top MAX_TOP_K, for every k.
    #[test]
    fn the_top_k_is_a_prefix_of_the_larger_ranking(
        items in items_strategy(),
        text in finding_text(),
        k in 1usize..=MAX_TOP_K,
    ) {
        let base: Vec<Item> = items.iter().enumerate().map(|(i, ids)| item(i, ids)).collect();
        let small = top_rows(&matching::rank(&base, &text, k));
        let large = top_rows(&matching::rank(&base, &text, MAX_TOP_K));
        prop_assert_eq!(&small[..], &large[..small.len().min(large.len())]);
    }
}

// ---------------------------------------------------- property 4: scope globs

proptest! {
    #![proptest_config(config(96))]

    /// `**/` matches zero or more whole directories, while `*` stays inside
    /// one. The two patterns differ exactly when the path has a directory
    /// between `src/` and the file.
    #[test]
    fn double_star_crosses_directories_and_star_does_not(
        dirs in proptest::collection::vec("[a-z]{1,8}", 0..4),
    ) {
        let mut path = String::from("src/");
        for d in &dirs {
            path.push_str(d);
            path.push('/');
        }
        path.push_str("Vault.sol");
        prop_assert!(matching::in_scope(&path, "src/**/*.sol"), "{}", path);
        prop_assert_eq!(
            matching::in_scope(&path, "src/*.sol"),
            dirs.is_empty(),
            "{}",
            path
        );
    }
}

// ------------------------------------------------------- property 5: top-k bound

proptest! {
    #![proptest_config(config(256))]

    /// `--top-k N` returns at most N items, for every N the CLI accepts.
    #[test]
    fn top_k_returns_at_most_n_items(
        items in items_strategy(),
        text in finding_text(),
        n in 1usize..=MAX_TOP_K,
    ) {
        let base: Vec<Item> = items.iter().enumerate().map(|(i, ids)| item(i, ids)).collect();
        let ranked = matching::rank(&base, &text, n);
        prop_assert!(ranked.len() <= n, "{} items for --top-k {n}", ranked.len());
    }
}

/// The CLI bounds --top-k to 1..=MAX_TOP_K and rejects the rest with exit 2.
#[test]
fn the_cli_rejects_top_k_outside_its_bounds() {
    let dir = manifest_dir().join("examples");
    let finding = dir.join("finding-demo.md");
    let dossier = dir.join("dossier-demo/dossier-demo.dossier");
    for bad in ["0", "51"] {
        let r = support::run(&[
            "check",
            finding.to_str().unwrap(),
            "--dossier",
            dossier.to_str().unwrap(),
            "--top-k",
            bad,
        ]);
        assert_eq!(
            r.code, 2,
            "--top-k {bad} should be a usage error: {}",
            r.stderr
        );
    }
}
