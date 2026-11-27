//! Release metadata that must move together with the crate version.
//!
//! * The GitHub Action installs `PA_DEFAULT_VERSION` when no `version` input is
//!   given, so it must name the release the action ships in.
//! * The crate package must leave out the sealed evaluation set and the search
//!   logs: a crates.io upload is permanent, and those files are research
//!   records, not inputs the binaries or the packaged tests read.
//! * The action metadata must meet the GitHub Marketplace rules.

use std::path::PathBuf;

fn read(rel: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
        .replace("\r\n", "\n")
}

/// The value of `key: value` on the first line that starts with `key:` after trimming.
fn yaml_value(text: &str, key: &str) -> Option<String> {
    text.lines().find_map(|l| {
        let l = l.trim_start();
        l.strip_prefix(key)
            .and_then(|r| r.strip_prefix(':'))
            .map(|v| v.trim().trim_matches('"').trim_matches('\'').to_string())
    })
}

#[test]
fn the_action_installs_this_release_by_default() {
    let action = read("action.yml");
    let v = yaml_value(&action, "PA_DEFAULT_VERSION").expect("PA_DEFAULT_VERSION in action.yml");
    assert_eq!(
        v,
        format!("v{}", env!("CARGO_PKG_VERSION")),
        "bump PA_DEFAULT_VERSION in action.yml with the crate version (CONTRIBUTING, Releasing step 1)"
    );
}

#[test]
fn the_crate_package_leaves_out_sealed_and_search_records() {
    let cargo = read("Cargo.toml");
    let start = cargo
        .find("\nexclude = [")
        .expect("an exclude list in [package]");
    let list = &cargo[start..start + cargo[start..].find(']').expect("closing ]")];
    for needed in [
        "eval/sealed-multilang/",
        "eval/dupbench/SEARCH_LOG.md",
        "eval/dupbench/pairs.json",
        "eval/dupbench/pairs-swap/",
        "eval/P2R*.md",
        "docs/assets/",
    ] {
        assert!(
            list.contains(&format!("\"{needed}\"")),
            "Cargo.toml exclude must list {needed}"
        );
    }
    // The packaged tests read these; they must stay in the crate.
    for kept in ["eval/dupbench/weights/", "eval/dupbench/manifest.json"] {
        assert!(
            !list.contains(&format!("\"{kept}")),
            "{kept} is read by tests and must stay in the package"
        );
    }
}

#[test]
fn the_action_metadata_meets_the_marketplace_rules() {
    let action = read("action.yml");
    let top: Vec<&str> = action
        .lines()
        .filter(|l| !l.starts_with(' ') && !l.starts_with('#') && l.contains(':'))
        .collect();
    let field = |k: &str| {
        top.iter()
            .find_map(|l| l.strip_prefix(k).and_then(|r| r.strip_prefix(':')))
            .map(|v| v.trim().trim_matches('"').trim_matches('\'').to_string())
            .unwrap_or_else(|| panic!("top-level {k} in action.yml"))
    };
    let name = field("name");
    assert!(!name.is_empty());
    // A Marketplace name may not match an existing user or organization; `priorart` is one.
    assert_ne!(name.to_lowercase(), "priorart");
    let description = field("description");
    assert!(
        !description.is_empty() && description.chars().count() < 125,
        "Marketplace description must be under 125 characters, got {}",
        description.chars().count()
    );
    let branding = &action[action.find("\nbranding:").expect("branding block")..];
    assert!(yaml_value(branding, "icon").is_some_and(|v| !v.is_empty()));
    assert!(yaml_value(branding, "color").is_some_and(|v| !v.is_empty()));
}
