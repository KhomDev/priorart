//! Reading a corpus of prior items, and preparing one for fast reuse.
//!
//! Three corpus forms are accepted:
//!
//! - a `.dossier` built by `priorart build` (the prior audit and known-issue
//!   items it holds);
//! - a directory of `*.md` / `*.txt` files, one prior item per file, sorted by
//!   file name; or
//! - a prepared corpus (`priorart candidates --prepare FILE`), which records
//!   each item's anchors and mechanism so a repeated run does not resolve and
//!   classify the whole corpus again.
//!
//! The reader refuses symlinks, FIFOs and devices, and enforces a size cap on
//! every file, matching the rest of the tool. No path outside the one the user
//! names is read.

use crate::anchors::{Anchor, IndexSet};
use crate::candidates::{CANDIDATES_CORPUS_SCHEMA, MAX_ITEM_BYTES, MAX_JSON_BYTES};
use crate::dossier::{self, import, sha256};
use crate::matching::Profile;
use crate::model::DOSSIER_SCHEMA;
use crate::taxonomy::{self, MechanismSet};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

/// The corpus form that was read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CorpusKind {
    /// A `priorart.dossier/v1` file.
    Dossier,
    /// A directory of `*.md` / `*.txt` files.
    Directory,
    /// A `priorart.candidates-corpus/v1` file with anchors and mechanisms.
    Prepared,
}

/// One corpus of prior items.
#[derive(Debug, Clone, PartialEq)]
pub struct Corpus {
    pub kind: CorpusKind,
    /// The path as given, forward slashes.
    pub path: String,
    pub items: Vec<CorpusItem>,
    /// The `repo@commit` pins recorded by a prepared corpus.
    pub repositories: Vec<String>,
    pub prepared: bool,
}

/// One prior item. `anchors` and `mechanism` are `Some` in a prepared corpus;
/// in a dossier or directory they are resolved on first use.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CorpusItem {
    pub id: String,
    pub title: Option<String>,
    pub text: String,
    #[serde(default)]
    pub text_sha256: Option<String>,
    #[serde(default)]
    pub identifiers: Vec<String>,
    #[serde(default)]
    pub anchors: Option<Vec<Anchor>>,
    #[serde(default)]
    pub mechanism: Option<MechanismSet>,
}

/// A prepared corpus (`priorart.candidates-corpus/v1`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PreparedCorpus {
    pub schema: String,
    pub tool_version: String,
    pub repositories: Vec<String>,
    pub items: Vec<CorpusItem>,
}

/// Read a corpus from a dossier file, a directory of text files, or a prepared
/// corpus file.
pub fn load_corpus(path: &Path) -> Result<Corpus, String> {
    let meta = match std::fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(e) => {
            return Err(format!(
                "cannot read corpus {}: {e}. Pass a .dossier file, a directory of .md/.txt files, or a prepared corpus JSON.",
                path.display()
            ))
        }
    };
    if meta.file_type().is_dir() {
        return load_directory(path);
    }
    import::require_regular_file_from(path, "corpus")?;
    let bytes = read_capped(path, MAX_JSON_BYTES, "corpus")?;
    let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(|e| {
        format!(
            "{}: not a dossier or prepared corpus JSON file: {e}. Pass a .dossier file, a directory of .md/.txt files, or a prepared corpus JSON.",
            path.display()
        )
    })?;
    let schema = value.get("schema").and_then(|s| s.as_str()).unwrap_or("");
    match schema {
        DOSSIER_SCHEMA => load_dossier(path),
        CANDIDATES_CORPUS_SCHEMA => load_prepared(path, value),
        other => Err(format!(
            "{}: schema {other:?} is not {DOSSIER_SCHEMA} or {CANDIDATES_CORPUS_SCHEMA}. Pass a .dossier file, a directory of .md/.txt files, or a prepared corpus JSON.",
            path.display()
        )),
    }
}

fn load_dossier(path: &Path) -> Result<Corpus, String> {
    let dossier = dossier::load::load(path)?;
    let mut items = Vec::new();
    for item in dossier.items {
        if matches!(
            item.kind.as_str(),
            "scope-clause" | "scope-path" | "payout-tier"
        ) {
            continue;
        }
        let text = if item.text.is_empty() {
            item.quote.clone()
        } else {
            item.text
        };
        items.push(CorpusItem {
            id: item.id,
            title: item.title,
            text_sha256: Some(sha256::hex(text.as_bytes())),
            text,
            identifiers: item.identifiers,
            anchors: None,
            mechanism: None,
        });
    }
    items.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(Corpus {
        kind: CorpusKind::Dossier,
        path: display_path(path),
        items,
        repositories: Vec::new(),
        prepared: false,
    })
}

fn load_prepared(path: &Path, value: serde_json::Value) -> Result<Corpus, String> {
    let prepared: PreparedCorpus = serde_json::from_value(value).map_err(|e| {
        format!(
            "{}: not a valid {CANDIDATES_CORPUS_SCHEMA} file: {e}. Rebuild it with priorart candidates --prepare.",
            path.display()
        )
    })?;
    let mut seen: BTreeMap<&str, usize> = BTreeMap::new();
    for item in &prepared.items {
        *seen.entry(item.id.as_str()).or_default() += 1;
    }
    if let Some((id, _)) = seen.iter().find(|(_, count)| **count > 1) {
        return Err(format!(
            "{}: prior item id {id:?} appears more than once. Rebuild the prepared corpus.",
            path.display()
        ));
    }
    Ok(Corpus {
        kind: CorpusKind::Prepared,
        path: display_path(path),
        items: prepared.items,
        repositories: prepared.repositories,
        prepared: true,
    })
}

fn load_directory(path: &Path) -> Result<Corpus, String> {
    let entries = std::fs::read_dir(path)
        .map_err(|e| format!("cannot read corpus directory {}: {e}", path.display()))?;
    let mut paths: Vec<std::path::PathBuf> = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| {
            format!(
                "cannot read an entry of corpus directory {}: {e}",
                path.display()
            )
        })?;
        let file = entry.path();
        let is_text = file
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("md") || e.eq_ignore_ascii_case("txt"));
        if is_text {
            paths.push(file);
        }
    }
    paths.sort();
    // A directory (`a.md` beside `a.txt`) would give two items one stem. The
    // full file name is the deterministic fallback for both.
    let mut stems: BTreeMap<String, usize> = BTreeMap::new();
    for file in &paths {
        *stems.entry(stem_of(file)).or_default() += 1;
    }
    let mut items = Vec::with_capacity(paths.len());
    for file in &paths {
        let stem = stem_of(file);
        let id = if stems.get(&stem).copied().unwrap_or(0) > 1 {
            file.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or(stem)
        } else {
            stem
        };
        let (text, sha) = read_item_text(file)?;
        items.push(CorpusItem {
            id,
            title: None,
            text_sha256: Some(sha),
            text,
            identifiers: Vec::new(),
            anchors: None,
            mechanism: None,
        });
    }
    items.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(Corpus {
        kind: CorpusKind::Directory,
        path: display_path(path),
        items,
        repositories: Vec::new(),
        prepared: false,
    })
}

fn stem_of(file: &Path) -> String {
    file.file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            file.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default()
        })
}

fn read_item_text(path: &Path) -> Result<(String, String), String> {
    let meta = import::require_regular_file_from(path, "prior item")?;
    if meta.len() > MAX_ITEM_BYTES {
        return Err(format!(
            "prior item {} is {}; priorart reads an item up to {} MiB. Split it.",
            path.display(),
            import::mib(meta.len()),
            MAX_ITEM_BYTES / (1024 * 1024)
        ));
    }
    let bytes = std::fs::read(path)
        .map_err(|e| format!("cannot read prior item {}: {e}", path.display()))?;
    if bytes.len() as u64 > MAX_ITEM_BYTES {
        return Err(format!(
            "prior item {} is {}; priorart reads an item up to {} MiB. Split it.",
            path.display(),
            import::mib(bytes.len() as u64),
            MAX_ITEM_BYTES / (1024 * 1024)
        ));
    }
    let text = import::strip_bom(&String::from_utf8_lossy(&bytes)).replace("\r\n", "\n");
    Ok((text, sha256::hex(&bytes)))
}

fn read_capped(path: &Path, cap: u64, what: &str) -> Result<Vec<u8>, String> {
    let meta = import::require_regular_file_from(path, what)?;
    if meta.len() > cap {
        return Err(format!(
            "{what} {} is {}; priorart reads up to {} MiB of JSON.",
            path.display(),
            import::mib(meta.len()),
            cap / (1024 * 1024)
        ));
    }
    let bytes =
        std::fs::read(path).map_err(|e| format!("cannot read {what} {}: {e}", path.display()))?;
    if bytes.len() as u64 > cap {
        return Err(format!(
            "{what} {} grew to {}; priorart reads up to {} MiB of JSON.",
            path.display(),
            import::mib(bytes.len() as u64),
            cap / (1024 * 1024)
        ));
    }
    Ok(bytes)
}

/// Resolve and classify every item, producing the cache written by
/// `priorart candidates --prepare`.
pub fn prepare_corpus(corpus: &Corpus, index: Option<&IndexSet>) -> PreparedCorpus {
    let index = index.filter(|i| !i.repos.is_empty());
    let mut items = Vec::with_capacity(corpus.items.len());
    for item in &corpus.items {
        let anchors: Vec<Anchor> = match &item.anchors {
            Some(anchors) => anchors.clone(),
            None => match index {
                Some(index) => crate::anchors::resolve_text(&item.text, index).anchors,
                None => Vec::new(),
            },
        };
        let mechanism: MechanismSet = match &item.mechanism {
            Some(mechanism) => mechanism.clone(),
            None => taxonomy::classify(&item.text),
        };
        let identifiers = if item.identifiers.is_empty() {
            Profile::from_text(&item.text).all_identifiers()
        } else {
            item.identifiers.clone()
        };
        items.push(CorpusItem {
            id: item.id.clone(),
            title: item.title.clone(),
            text_sha256: item
                .text_sha256
                .clone()
                .or_else(|| Some(sha256::hex(item.text.as_bytes()))),
            text: item.text.clone(),
            identifiers,
            anchors: Some(anchors),
            mechanism: Some(mechanism),
        });
    }
    let repositories = match index {
        Some(index) => {
            let mut ids: Vec<String> = index
                .repos
                .iter()
                .map(|r| format!("{}@{}", r.repo, r.commit))
                .collect();
            ids.sort();
            ids.dedup();
            ids
        }
        None => corpus.repositories.clone(),
    };
    PreparedCorpus {
        schema: CANDIDATES_CORPUS_SCHEMA.to_string(),
        tool_version: env!("CARGO_PKG_VERSION").to_string(),
        repositories,
        items,
    }
}

/// The summary `priorart candidates --prepare --json` prints.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrepareReport {
    pub schema: String,
    pub tool_version: String,
    pub corpus: String,
    pub corpus_kind: String,
    pub items: usize,
    pub anchors: usize,
    pub mechanisms: usize,
    pub repositories: Vec<String>,
    pub out: String,
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// The corpus kind ids, used in JSON and text.
impl CorpusKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Dossier => "dossier",
            Self::Directory => "directory",
            Self::Prepared => "prepared",
        }
    }
}
