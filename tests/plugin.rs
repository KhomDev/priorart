//! The Claude Code plugin (`plugin/`) is linked to the crate: its version is the
//! crates.io release it installs, so the plugin, the marketplace entry and
//! `Cargo.toml` must carry the same version, and the plugin's skill must be the
//! repository's skill byte for byte. A release that bumps one without the
//! others fails here.

use std::path::Path;

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn read(rel: &str) -> String {
    std::fs::read_to_string(root().join(rel))
        .unwrap_or_else(|e| panic!("{rel}: {e}"))
        .replace("\r\n", "\n")
}

fn json(rel: &str) -> serde_json::Value {
    serde_json::from_str(&read(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

#[test]
fn plugin_version_is_the_crate_version() {
    let want = env!("CARGO_PKG_VERSION");
    let plugin = json("plugin/.claude-plugin/plugin.json");
    assert_eq!(plugin["version"], want, "plugin.json version");
    assert_eq!(plugin["name"], "priorart");
    assert_eq!(plugin["license"], "MIT", "the directory requires a license");
    let market = json(".claude-plugin/marketplace.json");
    let entry = market["plugins"]
        .as_array()
        .and_then(|p| p.iter().find(|e| e["name"] == "priorart"))
        .expect("marketplace lists the priorart plugin");
    assert_eq!(entry["version"], want, "marketplace.json version");
    assert_eq!(entry["source"], "./plugin");
}

#[test]
fn plugin_skill_is_the_repository_skill() {
    assert_eq!(
        read("plugin/skills/priorart/SKILL.md"),
        read("skills/priorart/SKILL.md"),
        "copy skills/priorart/SKILL.md into plugin/skills/priorart/ when it changes"
    );
}

/// The directory blocks a plugin README under 40 words outside code blocks.
#[test]
fn plugin_readme_is_long_enough_for_the_directory() {
    let text = read("plugin/README.md");
    let mut in_code = false;
    let mut words = 0;
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            in_code = !in_code;
            continue;
        }
        if !in_code {
            words += line.split_whitespace().count();
        }
    }
    assert!(
        words >= 40,
        "plugin/README.md has {words} words outside code"
    );
    for must in ["network", "crates.io", "DEV-ONLY"] {
        assert!(
            text.contains(must),
            "plugin/README.md must say what it fetches: {must}"
        );
    }
}

#[test]
fn the_installer_pins_the_plugin_version_and_only_uses_cargo() {
    let script = read("plugin/bin/priorart-ensure");
    assert!(
        script.starts_with("#!/usr/bin/env bash\n"),
        "plain shell script"
    );
    assert!(
        script.contains(".claude-plugin/plugin.json"),
        "reads its version from the manifest"
    );
    assert!(script.contains("cargo install --locked --force priorart --version"));
    assert!(
        script.contains("--status"),
        "has a report-only mode for the SessionStart hook"
    );
    let hooks = json("plugin/hooks/hooks.json");
    let cmd = hooks["hooks"]["SessionStart"][0]["hooks"][0]["command"]
        .as_str()
        .expect("SessionStart command");
    assert_eq!(
        cmd, "bash \"${CLAUDE_PLUGIN_ROOT}/bin/priorart-ensure\" --status",
        "shell-form hook: quoted plugin root, report-only"
    );
}

/// Command bodies and their `allowed-tools` Bash rules must name the same
/// `${CLAUDE_PLUGIN_ROOT}` path, with `:*` only as the trailing wildcard.
#[test]
fn command_allowed_tools_expand_the_plugin_root() {
    for name in ["check", "install", "upgrade"] {
        let text = read(&format!("plugin/commands/{name}.md"));
        let front = text
            .split_once("---")
            .and_then(|(_, rest)| rest.split_once("---"))
            .map(|(front, _)| front)
            .unwrap_or_else(|| panic!("{name}: no frontmatter"));
        let tools = front
            .lines()
            .find_map(|line| line.strip_prefix("allowed-tools:"))
            .unwrap_or_else(|| panic!("{name}: no allowed-tools"))
            .trim();
        let rule = "Bash(${CLAUDE_PLUGIN_ROOT}/bin/priorart-ensure:*)";
        assert!(
            tools.split(',').any(|part| part.trim() == rule),
            "{name}: allowed-tools must contain `{rule}`, got `{tools}`"
        );
        let body = text.split("---").nth(2).unwrap();
        assert!(
            body.contains("${CLAUDE_PLUGIN_ROOT}/bin/priorart-ensure"),
            "{name}: body must run the same path the rule allows"
        );
        assert!(
            !tools.contains("priorart fetch") && !body.contains("priorart fetch"),
            "{name}: the plugin never runs the network fetch"
        );
    }
}

/// Claude Code connects a plugin MCP server at session start, before the
/// pinned CLI may be installed. See `plugin/README.md`.
#[test]
fn the_plugin_does_not_declare_an_mcp_server() {
    assert!(!root().join("plugin/.mcp.json").exists());
    let plugin = json("plugin/.claude-plugin/plugin.json");
    assert!(plugin.get("mcpServers").is_none());
}

/// The directory blocks system files and refuses binaries in the plugin folder.
#[test]
fn the_plugin_folder_holds_only_text() {
    fn walk(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
        for e in std::fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                walk(&p, out)
            } else {
                out.push(p)
            }
        }
    }
    let mut files = Vec::new();
    walk(&root().join("plugin"), &mut files);
    assert!(files.len() <= 512);
    for f in files {
        let name = f.file_name().unwrap().to_string_lossy().to_string();
        assert!(
            !matches!(name.as_str(), ".DS_Store" | "Thumbs.db" | "desktop.ini"),
            "{name}"
        );
        let bytes = std::fs::read(&f).unwrap();
        assert!(bytes.len() < 256 * 1024, "{} is over 256 KiB", f.display());
        assert!(
            std::str::from_utf8(&bytes).is_ok(),
            "{} is not text",
            f.display()
        );
    }
}
