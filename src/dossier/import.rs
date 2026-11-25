//! Read a saved, offline program folder into sources and items.
//!
//! Nothing here fetches a URL and nothing invents content. A page is a file
//! the user saved; a saved `.html` page is converted to text by
//! [`crate::dossier::html`]; a PDF is read from disk, with pdftotext as a
//! fallback when the uncompressed scrape finds nothing. A PDF that cannot be
//! read is a warning and a skipped source, never a panic.

use crate::dossier::{html, sha256};
use crate::matching::{collapse, evidence_quote, mechanism_of, Profile};
use crate::model::{Item, Payout, PriorAudit, Program, Source};
use crate::vocab;
use std::ffi::OsStr;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::{Duration, Instant};

/// Limits on one build. A folder over a limit is a usage error (exit 2) that
/// names the limit; the build never reads without bound. They are constants,
/// not flags: a larger audit is converted or split, and docs/dossier.md says so.
pub const MAX_INPUT_BYTES: u64 = 64 * 1024 * 1024;
/// Documents read from one program folder, across scope, known issues and audits.
pub const MAX_INPUT_FILES: usize = 1_000;
/// Total bytes of the documents read from one program folder.
pub const MAX_TOTAL_BYTES: u64 = 512 * 1024 * 1024;
/// Directory depth under audits/.
pub const MAX_WALK_DEPTH: usize = 16;
/// Directory entries visited under audits/, files and folders alike.
pub const MAX_WALK_ENTRIES: usize = 10_000;

/// pdftotext runs for at most this long, then is killed. A source that times
/// out is skipped with a warning, like any other PDF that yields no text.
pub const PDFTOTEXT_TIMEOUT: Duration = Duration::from_secs(60);
/// pdftotext may write this many bytes to stdout. More and it is killed.
pub const PDFTOTEXT_MAX_OUTPUT: usize = 16 * 1024 * 1024;
/// Stderr kept for an error message. The rest is read and discarded, so a
/// chatty child cannot block on a full pipe.
const PDFTOTEXT_MAX_STDERR: usize = 4096;

/// Options for reading one program folder.
#[derive(Debug, Clone, Copy)]
pub struct ImportOptions {
    /// Run `pdftotext` when the in-process scrape finds no text. `build
    /// --no-pdftotext` turns this off, and then a PDF the built-in scrape
    /// cannot read is reported unread, never handed to poppler.
    pub use_pdftotext: bool,
}

impl Default for ImportOptions {
    fn default() -> Self {
        Self {
            use_pdftotext: true,
        }
    }
}

/// One program folder, read.
#[derive(Debug)]
pub struct Imported {
    pub program: Program,
    pub sources: Vec<Source>,
    pub items: Vec<Item>,
    /// Warnings for the caller to print. A skipped input is always listed.
    pub notes: Vec<String>,
}

/// program.yaml, as a user writes it. Unknown keys are ignored on purpose:
/// real engagement notes grow extra fields, and dropping the file over a key
/// this build does not read would be worse than ignoring it.
#[derive(Debug, Default, serde::Deserialize)]
struct ProgramFile {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    platform: Option<String>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    scope_url: Option<String>,
    #[serde(default)]
    payout: Option<PayoutFile>,
    #[serde(default)]
    prior_audits: Vec<PriorAuditFile>,
}

#[derive(Debug, serde::Deserialize)]
struct PayoutFile {
    tier_shape: String,
    #[serde(default)]
    floor_status: Option<String>,
    #[serde(default)]
    floor_severity: Option<String>,
    #[serde(default)]
    source_url: Option<String>,
    #[serde(default)]
    retrieved: Option<String>,
    #[serde(default)]
    section: Option<String>,
    #[serde(default)]
    quote: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct PriorAuditFile {
    #[serde(default)]
    firm: Option<String>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    covers: Vec<String>,
    #[serde(default)]
    known_issues: Vec<String>,
}

/// Read a program directory with the default options.
///
/// Returns a usage error only when the directory or its layout is unusable; a
/// single unreadable input is a note.
pub fn read_program_dir(dir: &Path, program_override: Option<&str>) -> Result<Imported, String> {
    read_program_dir_with(dir, program_override, &ImportOptions::default())
}

/// Read a program directory with explicit options.
pub fn read_program_dir_with(
    dir: &Path,
    program_override: Option<&str>,
    options: &ImportOptions,
) -> Result<Imported, String> {
    if !dir.is_dir() {
        return Err(format!(
            "{} is not a directory. Point priorart build at a folder of saved inputs; priorart init scaffolds one.",
            dir.display()
        ));
    }
    let mut notes = Vec::new();
    let program_file = read_program_file(dir, &mut notes)?;
    let program = program_of(dir, program_override, program_file.as_ref())?;

    // Order is fixed: program page, known issues, then audits by file name.
    // The dossier's item order and ids follow it, so two builds on the same
    // folder are byte-identical.
    let mut found: Vec<(PathBuf, &str)> = Vec::new();
    if let Some(path) = find_one(
        dir,
        &["scope.md", "scope.html", "scope.htm", "scope.txt"],
        &mut notes,
    )? {
        found.push((path, "program-page"));
    }
    if let Some(path) = find_one(
        dir,
        &[
            "known-issues.md",
            "known-issues.yaml",
            "known-issues.yml",
            "known-issues.txt",
        ],
        &mut notes,
    )? {
        found.push((path, "known-issues"));
    }
    let audits_dir = dir.join("audits");
    match fs::symlink_metadata(&audits_dir) {
        Ok(meta) if meta.file_type().is_dir() => {
            let mut walk = Walk::default();
            collect_files(&audits_dir, &mut walk, WalkLimits::DEFAULT)?;
            // The walk skips symlinks and non-regular files; name each one so
            // an audit cannot disappear from a dossier silently. Sorting keeps
            // the notes independent of filesystem order.
            walk.skipped.sort();
            for (path, kind) in &walk.skipped {
                let rel = path
                    .strip_prefix(dir)
                    .unwrap_or(path)
                    .to_string_lossy()
                    .replace('\\', "/");
                notes.push(format!(
                    "{rel} is {kind}; priorart reads regular files only and skipped it"
                ));
            }
            let mut files = walk.docs;
            // A README in audits/ is scaffolding, not a prior audit.
            files.retain(|p| {
                !p.file_name()
                    .is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case("README.md"))
            });
            files.sort();
            found.extend(files.into_iter().map(|p| (p, "audit")));
        }
        Ok(meta) => notes.push(format!(
            "audits is {}; priorart reads a regular directory and skipped it",
            file_kind(&meta.file_type())
        )),
        Err(_) => {}
    }

    if found.is_empty() {
        let skipped = if notes.is_empty() {
            String::new()
        } else {
            format!(" Skipped: {}.", notes.join("; "))
        };
        return Err(format!(
            "no inputs found in {}. Expected scope.md (or .html/.htm/.txt), known-issues.md/.yaml/.txt, or audits/*.md/.txt/.pdf. Run priorart init to scaffold the layout.{skipped}",
            dir.display()
        ));
    }
    check_input_limits(&found, MAX_INPUT_FILES, MAX_INPUT_BYTES, MAX_TOTAL_BYTES)?;
    let inputs: Vec<Input> = found
        .iter()
        .map(|(path, kind)| Input::new(path, kind, dir, options))
        .collect::<Result<_, _>>()?;

    let mut sources = Vec::new();
    let mut items = Vec::new();
    let mut program_payout = program.payout.clone();
    for input in &inputs {
        let source_id = format!("src-{:04}", sources.len() + 1);
        let Some(text) = input.text(&mut notes) else {
            continue;
        };
        sources.push(Source {
            id: source_id.clone(),
            kind: input.kind.clone(),
            title: Some(input.title.clone()),
            path: input.rel_path.clone(),
            sha256: input.sha256.clone(),
            pages: input.pages,
        });
        let extracted = match input.kind.as_str() {
            "program-page" => extract_program_page(&text, &source_id, &input.title),
            "known-issues" => extract_known_issues(&text, &source_id, &input.title, input.is_yaml),
            _ => extract_audit(&text, &source_id, &input.title),
        };
        // A reward-table quote is taken from the page itself when the payout
        // block names no quote, so the citation is always the page.
        if let Some(payout) = program_payout.as_mut() {
            if payout.quote.is_none() {
                for item in &extracted {
                    if item.kind == "payout-tier" {
                        let low = item.text.to_lowercase();
                        if payout
                            .floor_severity
                            .as_deref()
                            .is_some_and(|sev| low.contains(&sev.to_lowercase()))
                        {
                            payout.quote = Some(item.quote.clone());
                            payout.section = item.section.clone();
                            break;
                        }
                    }
                }
            }
        }
        items.extend(extracted);
    }
    for (n, item) in items.iter_mut().enumerate() {
        item.id = format!("item-{:04}", n + 1);
    }
    let mut program = program;
    program.payout = program_payout;
    let audit_count = sources.iter().filter(|s| s.kind == "audit").count();
    notes.push(format!(
        "{} source(s) indexed ({} audit(s)); {} item(s) extracted",
        sources.len(),
        audit_count,
        items.len()
    ));
    if items
        .iter()
        .all(|i| i.kind == "audit-finding" || i.kind == "audit-section")
    {
        notes.push(
            "no program page or known-issues items were found; the exclusion and payout gates have nothing to quote".into(),
        );
    }
    Ok(Imported {
        program,
        sources,
        items,
        notes,
    })
}

fn read_program_file(dir: &Path, notes: &mut Vec<String>) -> Result<Option<ProgramFile>, String> {
    let path = dir.join("program.yaml");
    let meta = match fs::symlink_metadata(&path) {
        Ok(meta) => meta,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("cannot read {}: {e}", path.display())),
    };
    if !meta.file_type().is_file() {
        return Err(format!(
            "{} is {}; priorart reads program.yaml only as a regular file. Replace it with a regular file.",
            path.display(),
            file_kind(&meta.file_type())
        ));
    }
    check_file_size(&path, meta.len(), MAX_INPUT_BYTES)?;
    let text = read_regular_text(&path)?;
    let parsed: ProgramFile = serde_saphyr::from_str(&text).map_err(|e| {
        format!(
            "{}: not a program.yaml this build reads: {e}",
            path.display()
        )
    })?;
    if let Some(payout) = parsed.payout.as_ref() {
        if vocab::tier_shape(&payout.tier_shape).is_none() {
            return Err(format!(
                "{}: payout.tier_shape {:?} is not one of the five tier shapes ({}). Run priorart vocab list to see them.",
                path.display(),
                payout.tier_shape,
                vocab::tier_shape_ids().join(", ")
            ));
        }
        for key in [
            ("floor_status", payout.floor_status.as_deref()),
            ("floor_severity", payout.floor_severity.as_deref()),
        ] {
            if let Some(value) = key.1 {
                let ok = match key.0 {
                    "floor_status" => ["documented", "unstated", "conflicting"].contains(&value),
                    _ => ["critical", "high", "medium", "low"].contains(&value),
                };
                if !ok {
                    return Err(format!(
                        "{}: payout.{} {:?} is not a value the payout gate understands",
                        path.display(),
                        key.0,
                        value
                    ));
                }
            }
        }
        if payout.source_url.as_deref().unwrap_or("").trim().is_empty() {
            notes.push(
                "program.yaml has a payout block with no source_url: the payout gate will report the terms as uncited".into(),
            );
        }
    }
    Ok(Some(parsed))
}

fn program_of(
    dir: &Path,
    program_override: Option<&str>,
    file: Option<&ProgramFile>,
) -> Result<Program, String> {
    let from_file = file.and_then(|f| f.id.as_deref());
    let stem = dir
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "program".into());
    let raw = program_override.or(from_file).unwrap_or(&stem);
    let id = slug(raw).ok_or_else(|| {
        format!(
            "program id {raw:?} is empty after slugification. Pass a --program id of letters, digits, dots, dashes or underscores."
        )
    })?;
    Ok(Program {
        id,
        name: file.and_then(|f| f.name.clone()),
        platform: file.and_then(|f| f.platform.clone()),
        url: file.and_then(|f| f.url.clone()),
        scope_url: file.and_then(|f| f.scope_url.clone()),
        payout: file.and_then(|f| f.payout.as_ref()).map(|p| Payout {
            tier_shape: p.tier_shape.clone(),
            floor_status: p.floor_status.clone(),
            floor_severity: p.floor_severity.clone(),
            source_url: p.source_url.clone(),
            retrieved: p.retrieved.clone(),
            section: p.section.clone(),
            quote: p.quote.clone(),
        }),
        prior_audits: file
            .map(|f| {
                f.prior_audits
                    .iter()
                    .map(|a| PriorAudit {
                        firm: a.firm.clone(),
                        url: a.url.clone(),
                        covers: a.covers.clone(),
                        known_issues: a.known_issues.clone(),
                    })
                    .collect()
            })
            .unwrap_or_default(),
    })
}

/// Lowercase slug: letters, digits, dot, dash, underscore; runs of other
/// characters become one dash. Empty input yields None.
pub fn slug(raw: &str) -> Option<String> {
    let mut out = String::new();
    for c in raw.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if matches!(c, '.' | '-' | '_') {
            out.push(c);
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    let trimmed = out.trim_matches('-').to_string();
    if trimmed.chars().any(|c| c.is_ascii_alphanumeric()) {
        Some(trimmed)
    } else {
        None
    }
}

fn find_one(
    dir: &Path,
    names: &[&str],
    notes: &mut Vec<String>,
) -> Result<Option<PathBuf>, String> {
    let mut found: Vec<PathBuf> = Vec::new();
    for name in names {
        let path = dir.join(name);
        match fs::symlink_metadata(&path) {
            Ok(meta) if meta.file_type().is_file() => found.push(path),
            // A symlink, FIFO or directory is not the file the user saved.
            // Name it instead of following or reading it.
            Ok(meta) => notes.push(format!(
                "{} is {}; priorart reads regular files only and skipped it",
                display_name(&path),
                file_kind(&meta.file_type())
            )),
            Err(_) => {}
        }
    }
    match found.len() {
        0 => Ok(None),
        1 => Ok(Some(found[0].clone())),
        _ => {
            notes.push(format!(
                "several program-page files match ({}); using {}",
                found
                    .iter()
                    .map(|p| display_name(p))
                    .collect::<Vec<_>>()
                    .join(", "),
                display_name(&found[0])
            ));
            Ok(Some(found[0].clone()))
        }
    }
}

fn display_name(path: &Path) -> String {
    path.file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

/// What the walk under audits/ has seen so far.
#[derive(Default)]
struct Walk {
    docs: Vec<PathBuf>,
    /// Symlinks and special files that were skipped, with the kind to print.
    skipped: Vec<(PathBuf, &'static str)>,
    entries: usize,
}

/// The walk's limits, a parameter so a test can exercise them with small values.
#[derive(Clone, Copy)]
struct WalkLimits {
    depth: usize,
    entries: usize,
}

impl WalkLimits {
    const DEFAULT: Self = Self {
        depth: MAX_WALK_DEPTH,
        entries: MAX_WALK_ENTRIES,
    };
}

fn collect_files(dir: &Path, walk: &mut Walk, limits: WalkLimits) -> Result<(), String> {
    collect_at_depth(dir, 0, walk, limits)
}

fn collect_at_depth(
    dir: &Path,
    depth: usize,
    walk: &mut Walk,
    limits: WalkLimits,
) -> Result<(), String> {
    if depth > limits.depth {
        return Err(format!(
            "{} is nested more than {} folders deep under audits/. priorart stops at that depth; flatten the folder.",
            dir.display(),
            limits.depth
        ));
    }
    let rd = fs::read_dir(dir).map_err(|e| format!("cannot read {}: {e}", dir.display()))?;
    for entry in rd.flatten() {
        walk.entries += 1;
        if walk.entries > limits.entries {
            return Err(format!(
                "audits/ holds more than {} entries. priorart stops there; move the rest out of the program folder.",
                limits.entries
            ));
        }
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        let Ok(meta) = fs::symlink_metadata(&path) else {
            continue;
        };
        let ft = meta.file_type();
        if ft.is_symlink() {
            // Every symlink under audits/ is named, file or directory, hidden
            // or not. A symlinked subdirectory is a whole tree the user may
            // think was indexed, so it must not disappear silently.
            walk.skipped.push((path, "a symlink"));
            continue;
        }
        if name.starts_with('.') {
            // Hidden regular files and directories are scaffolding, not
            // audits; they stay skipped as before.
            continue;
        }
        if ft.is_dir() {
            collect_at_depth(&path, depth + 1, walk, limits)?;
        } else if is_doc(&path) {
            if ft.is_file() {
                walk.docs.push(path);
            } else {
                walk.skipped.push((path, file_kind(&ft)));
            }
        }
    }
    Ok(())
}

fn file_len(path: &Path) -> Result<u64, String> {
    Ok(require_regular_file(path)?.len())
}

/// The kind of file a `symlink_metadata` reports, for an error message.
fn file_kind(ft: &fs::FileType) -> &'static str {
    if ft.is_symlink() {
        "a symlink"
    } else if ft.is_dir() {
        "a directory"
    } else if ft.is_file() {
        "a regular file"
    } else {
        "a special file, not a regular file"
    }
}

/// Reject anything that is not a regular file before it is opened.
///
/// A symlink can point outside the program folder, a FIFO can block the build
/// forever, and a device is not page text. This is a named boundary, not an
/// atomic open: the check and the later read are separate system calls, so a
/// concurrent writer can still swap the path between them. A local writer can
/// choose the bytes anyway; the goal is to refuse an accidental or stale
/// symlink/FIFO, not to sandbox the folder.
pub fn require_regular_file(path: &Path) -> Result<fs::Metadata, String> {
    require_regular_file_from(path, "file")
}

/// [`require_regular_file`], with the role name (`finding`, `dossier`) that the
/// caller's error message should carry.
pub fn require_regular_file_from(path: &Path, what: &str) -> Result<fs::Metadata, String> {
    let meta = fs::symlink_metadata(path)
        .map_err(|e| format!("cannot read {what} {}: {e}", path.display()))?;
    let ft = meta.file_type();
    if !ft.is_file() {
        return Err(format!(
            "cannot read {what} {}: it is {}; priorart reads regular files only (no symlinks, FIFOs or devices). Replace it with a regular file.",
            path.display(),
            file_kind(&ft)
        ));
    }
    Ok(meta)
}

/// Read a regular file, refusing symlinks and special files first.
pub fn read_regular_file(path: &Path) -> Result<Vec<u8>, String> {
    require_regular_file(path)?;
    fs::read(path).map_err(|e| format!("cannot read {}: {e}", path.display()))
}

/// Refuse a folder over the count, per-file or total size limits, before any
/// file is read. The error names the limit and the file that broke it.
fn check_input_limits(
    found: &[(PathBuf, &str)],
    max_files: usize,
    max_file_bytes: u64,
    max_total_bytes: u64,
) -> Result<(), String> {
    if found.len() > max_files {
        return Err(format!(
            "{} input documents found; priorart reads at most {max_files} per program folder. Move the rest out of the folder, or build them in smaller folders.",
            found.len()
        ));
    }
    let mut total = 0u64;
    for (path, _) in found {
        let len = file_len(path)?;
        check_file_size(path, len, max_file_bytes)?;
        total = total.saturating_add(len);
    }
    if total > max_total_bytes {
        return Err(format!(
            "the input documents total {}; priorart reads at most {} per program folder. Split the folder.",
            mib(total),
            mib(max_total_bytes)
        ));
    }
    Ok(())
}

fn check_file_size(path: &Path, len: u64, max: u64) -> Result<(), String> {
    if len > max {
        return Err(format!(
            "{} is {}; priorart reads each input up to {}. Convert or split it, then build again.",
            path.display(),
            mib(len),
            mib(max)
        ));
    }
    Ok(())
}

pub(crate) fn mib(bytes: u64) -> String {
    format!("{:.1} MiB", bytes as f64 / (1024.0 * 1024.0))
}
fn is_doc(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .as_deref(),
        Some("md" | "markdown" | "txt" | "pdf")
    )
}

/// One input file, hashed and decoded once.
struct Input {
    kind: String,
    rel_path: String,
    title: String,
    sha256: String,
    pages: usize,
    is_yaml: bool,
    text: Option<String>,
    warning: Option<String>,
}

impl Input {
    fn new(path: &Path, kind: &str, root: &Path, options: &ImportOptions) -> Result<Self, String> {
        // Re-check at read time: discovery and reading are two steps.
        let meta = require_regular_file(path)?;
        check_file_size(path, meta.len(), MAX_INPUT_BYTES)?;
        let bytes = fs::read(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        let rel = path
            .strip_prefix(root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        let title = path
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| rel.clone());
        let is_yaml = matches!(
            path.extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_ascii_lowercase())
                .as_deref(),
            Some("yaml" | "yml")
        );
        let mut text = None;
        let mut pages = 1;
        let mut warning = None;
        if is_pdf(path) {
            match pdf_to_text_with(path, options.use_pdftotext) {
                Ok(t) => {
                    pages = 1 + t.matches('\u{0c}').count();
                    text = Some(t);
                }
                Err(e) => warning = Some(e),
            }
        } else {
            let raw = String::from_utf8_lossy(&bytes).to_string();
            let raw = strip_bom(&raw);
            let t = if html::is_html_path(path) {
                // A saved page is converted conservatively: headings, bullets
                // and paragraphs survive, markup and scripts do not.
                html::to_text(&raw)
            } else {
                raw.replace("\r\n", "\n").replace('\r', "\n")
            };
            pages = 1 + t.matches('\u{0c}').count();
            text = Some(t);
        }
        Ok(Self {
            kind: kind.into(),
            rel_path: rel,
            title,
            sha256: sha256::hex(&bytes),
            pages,
            is_yaml,
            text,
            warning,
        })
    }

    fn text(&self, notes: &mut Vec<String>) -> Option<String> {
        match &self.text {
            Some(t) => Some(t.clone()),
            None => {
                if let Some(w) = &self.warning {
                    notes.push(format!("{w} - source skipped"));
                }
                None
            }
        }
    }
}

fn is_pdf(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("pdf"))
}

fn read_regular_text(path: &Path) -> Result<String, String> {
    let bytes = read_regular_file(path)?;
    Ok(strip_bom(&String::from_utf8_lossy(&bytes)).replace("\r\n", "\n"))
}

pub fn strip_bom(s: &str) -> String {
    s.strip_prefix('\u{feff}').unwrap_or(s).to_string()
}

/// Extract text from a PDF with the default policy: the uncompressed literal
/// scrape first, then pdftotext. A missing pdftotext is a clear error message,
/// never a panic.
pub fn pdf_to_text(path: &Path) -> Result<String, String> {
    pdf_to_text_with(path, true)
}

/// Extract text from a PDF. `use_pdftotext` false means the built-in scrape is
/// the only reader: when it finds no text, the source is reported unread and
/// no native parser is started.
pub fn pdf_to_text_with(path: &Path, use_pdftotext: bool) -> Result<String, String> {
    // A PDF is read as a regular file like every other input: a symlink, FIFO
    // or device is refused by name before anything is opened.
    let bytes = read_regular_file(path)?;
    let scraped = scrape_pdf_literals(&bytes);
    let letters = scraped.chars().filter(|c| c.is_alphanumeric()).count();
    if letters > 40 {
        return Ok(scraped);
    }
    if !use_pdftotext {
        return Err(format!(
            "{}: no extractable text (the uncompressed scrape found nothing and pdftotext is disabled with --no-pdftotext). Convert it with a text tool and pass the .txt.",
            path.display()
        ));
    }
    let Some(program) = find_on_path(std::env::var_os("PATH").as_deref()) else {
        return Err(format!(
            "{}: no extractable text (the uncompressed scrape found nothing and pdftotext is not installed). Convert it with pdftotext -layout and pass the .txt.",
            path.display()
        ));
    };
    run_pdftotext(&program, path, PDFTOTEXT_TIMEOUT, PDFTOTEXT_MAX_OUTPUT)
        .map_err(|e| format!("{}: {e}", path.display()))
}

/// The pdftotext executable on PATH, or None. Only absolute PATH entries are
/// searched. A relative entry is the current directory in disguise, and on
/// Windows `Command::new` would also search the current directory first, so
/// the lookup is done here and the child is started by its full path.
pub fn find_on_path(path_var: Option<&OsStr>) -> Option<PathBuf> {
    let names: &[&str] = if cfg!(windows) {
        &["pdftotext.exe", "pdftotext.bat", "pdftotext.cmd"]
    } else {
        &["pdftotext"]
    };
    for dir in std::env::split_paths(path_var?) {
        if !dir.is_absolute() {
            continue;
        }
        for name in names {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

/// Run `pdftotext -layout -q <pdf> -` with a time limit and an output limit.
///
/// The PDF is passed as an absolute path. An absolute path never starts with
/// `-`, so pdftotext cannot read a file name as an option, and it does not
/// depend on the current directory. A child that runs past `timeout`, or
/// writes more than `max_output` bytes to stdout, is killed and this returns
/// an error. Reader threads are not joined after a kill: a grandchild that
/// still holds the pipe must not block the build.
pub fn run_pdftotext(
    program: &Path,
    pdf: &Path,
    timeout: Duration,
    max_output: usize,
) -> Result<String, String> {
    let abs = std::path::absolute(pdf).map_err(|e| format!("cannot resolve the path: {e}"))?;
    let mut cmd = Command::new(program);
    cmd.arg("-layout")
        .arg("-q")
        .arg(&abs)
        .arg("-")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = spawn_with_retry(&mut cmd)
        .map_err(|e| format!("could not start {}: {e}", program.display()))?;
    let stdout = child.stdout.take().expect("stdout is piped");
    let stderr = child.stderr.take().expect("stderr is piped");

    let overflow = Arc::new(AtomicBool::new(false));
    let (out_tx, out_rx) = mpsc::channel();
    let flag = Arc::clone(&overflow);
    thread::spawn(move || {
        let _ = out_tx.send(read_capped(stdout, max_output, &flag));
    });
    let (err_tx, err_rx) = mpsc::channel();
    thread::spawn(move || {
        let _ = err_tx.send(read_head(stderr, PDFTOTEXT_MAX_STDERR));
    });

    let started = Instant::now();
    let status = loop {
        if overflow.load(Ordering::SeqCst) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!(
                "pdftotext wrote more than {} MiB of text; killed",
                max_output / (1024 * 1024)
            ));
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {}
            Err(e) => {
                let _ = child.kill();
                return Err(format!("could not wait for pdftotext: {e}"));
            }
        }
        if started.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!(
                "pdftotext did not finish within {} s; killed",
                timeout.as_secs()
            ));
        }
        thread::sleep(Duration::from_millis(10));
    };

    // The child has exited. Its stdout is read to the end by the thread; a
    // pipe still held by a grandchild is reported, not waited on forever.
    let received = out_rx.recv_timeout(Duration::from_secs(5));
    if overflow.load(Ordering::SeqCst) {
        return Err(format!(
            "pdftotext wrote more than {} MiB of text",
            max_output / (1024 * 1024)
        ));
    }
    let bytes = match received {
        Ok(Ok(bytes)) => bytes,
        Ok(Err(e)) => return Err(format!("could not read pdftotext output: {e}")),
        Err(_) => return Err("pdftotext output did not end".into()),
    };
    if !status.success() {
        let err = err_rx
            .recv_timeout(Duration::from_secs(1))
            .unwrap_or_default();
        return Err(format!(
            "pdftotext failed ({})",
            String::from_utf8_lossy(&err).trim()
        ));
    }
    let text = String::from_utf8_lossy(&bytes).to_string();
    if text.trim().is_empty() {
        Err("pdftotext returned no text".into())
    } else {
        Ok(text)
    }
}

/// Start the child, retrying a few times when the executable is busy.
///
/// Executing a file that was written a moment earlier can fail with ETXTBSY
/// (os error 26) on Linux, or a sharing violation (os error 32) on Windows,
/// when another thread forked while the file was still open for writing: the
/// forked child holds the write handle until it execs. The test suite does
/// exactly that (it writes a fake `pdftotext` and runs it while other tests
/// spawn processes), and so can any tool that installs `pdftotext` while a
/// build runs. Only those two errors are retried, ten times, 20 ms apart; any
/// other error is returned at once.
fn spawn_with_retry(cmd: &mut Command) -> std::io::Result<std::process::Child> {
    let mut last = None;
    for _ in 0..10 {
        match cmd.spawn() {
            Ok(child) => return Ok(child),
            Err(e) if matches!(e.raw_os_error(), Some(26) | Some(32)) => {
                thread::sleep(Duration::from_millis(20));
                last = Some(e);
            }
            Err(e) => return Err(e),
        }
    }
    Err(last.expect("a retryable error was recorded"))
}

/// Read a pipe to its end, keeping at most `max` bytes. Past `max`, the
/// reader sets the flag and stops; the caller kills the child.
fn read_capped(mut pipe: impl Read, max: usize, overflow: &AtomicBool) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        match pipe.read(&mut chunk) {
            Ok(0) => return Ok(out),
            Ok(n) => {
                if out.len() + n > max {
                    overflow.store(true, Ordering::SeqCst);
                    return Err("output limit reached".into());
                }
                out.extend_from_slice(&chunk[..n]);
            }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e.to_string()),
        }
    }
}

/// Read a pipe to its end, keeping the first `max` bytes and discarding the rest.
fn read_head(mut pipe: impl Read, max: usize) -> Vec<u8> {
    let mut kept = Vec::new();
    let mut chunk = [0u8; 1024];
    while let Ok(n) = pipe.read(&mut chunk) {
        if n == 0 {
            break;
        }
        let room = max.saturating_sub(kept.len()).min(n);
        kept.extend_from_slice(&chunk[..room]);
    }
    kept
}

/// Pull literal strings out of an uncompressed PDF. Compressed streams are
/// left for pdftotext. This is enough for a text-showing PDF that was not
/// deflated, and it does not pretend to be a PDF parser.
///
/// A literal's bytes are decoded as UTF-16BE or UTF-16LE when they start with
/// the matching byte-order mark (the usual way a PDF carries non-Latin text),
/// as UTF-8 when they are valid UTF-8, and byte-per-character only as a last
/// resort. Before 0.2.0 every byte became a Latin-1 char, so a UTF-16 literal
/// produced text interleaved with NUL characters.
pub fn scrape_pdf_literals(bytes: &[u8]) -> String {
    let mut out = String::new();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'(' {
            let mut j = i + 1;
            let mut buf: Vec<u8> = Vec::new();
            let mut escaped = false;
            let mut depth = 1i32;
            while j < bytes.len() {
                let c = bytes[j];
                if escaped {
                    match c {
                        b'n' => buf.push(b'\n'),
                        b'r' => buf.push(b'\r'),
                        b't' => buf.push(b'\t'),
                        b'b' => buf.push(0x08),
                        b'f' => buf.push(0x0c),
                        b'0'..=b'7' => {
                            // A PDF octal escape is up to three digits.
                            let mut value = (c - b'0') as u16;
                            let mut digits = 1;
                            while digits < 3 {
                                match bytes.get(j + 1) {
                                    Some(&next) if (b'0'..=b'7').contains(&next) => {
                                        value = value * 8 + (next - b'0') as u16;
                                        j += 1;
                                        digits += 1;
                                    }
                                    _ => break,
                                }
                            }
                            buf.push(value as u8);
                        }
                        other => buf.push(other),
                    }
                    escaped = false;
                    j += 1;
                    continue;
                }
                match c {
                    b'\\' => escaped = true,
                    b'(' => {
                        depth += 1;
                        buf.push(c);
                    }
                    b')' => {
                        depth -= 1;
                        if depth == 0 {
                            j += 1;
                            break;
                        }
                        buf.push(c);
                    }
                    _ => buf.push(c),
                }
                j += 1;
            }
            let after = skip_pdf_whitespace(bytes, j);
            if (starts_with_bytes(bytes, after, b"Tj")
                || bytes.get(after) == Some(&b'\'')
                || starts_with_bytes(bytes, after, b"TJ"))
                && buf.iter().any(|b| !b.is_ascii_whitespace())
            {
                let decoded = decode_pdf_string(&buf);
                let trimmed = decoded.trim();
                if !trimmed.is_empty() {
                    out.push_str(trimmed);
                    out.push('\n');
                }
            }
            i = j;
        } else {
            i += 1;
        }
    }
    out
}

fn skip_pdf_whitespace(bytes: &[u8], mut i: usize) -> usize {
    while bytes.get(i).is_some_and(|c| c.is_ascii_whitespace()) {
        i += 1;
    }
    i
}

fn starts_with_bytes(bytes: &[u8], at: usize, needle: &[u8]) -> bool {
    bytes.len() >= at + needle.len() && &bytes[at..at + needle.len()] == needle
}

/// Decode one PDF literal's bytes. A BOM picks UTF-16; valid UTF-8 is used as
/// UTF-8; otherwise each byte is one character, which is the old behaviour and
/// only a fallback.
fn decode_pdf_string(bytes: &[u8]) -> String {
    if let Some(rest) = bytes.strip_prefix(&[0xFE, 0xFF]) {
        return utf16_to_string(rest, u16::from_be_bytes);
    }
    if let Some(rest) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        return utf16_to_string(rest, u16::from_le_bytes);
    }
    if let Ok(text) = std::str::from_utf8(bytes) {
        return text.to_string();
    }
    bytes.iter().map(|b| char::from(*b)).collect()
}

fn utf16_to_string(bytes: &[u8], decode: fn([u8; 2]) -> u16) -> String {
    let units: Vec<u16> = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| decode(*pair))
        .collect();
    String::from_utf16_lossy(&units)
}

// ------------------------------------------------------------ extraction

/// A line plus the page it falls on, so every item carries a real page number.
struct Line {
    text: String,
    page: usize,
}

fn lines_with_pages(text: &str) -> Vec<Line> {
    let mut page = 1usize;
    let mut out = Vec::new();
    for raw in text.split('\n') {
        let mut line = raw.to_string();
        while let Some(at) = line.find('\u{0c}') {
            line.replace_range(at..at + 1, "");
            page += 1;
        }
        out.push(Line { text: line, page });
    }
    out
}

/// A section: heading plus the lines under it.
struct Section {
    heading: Option<String>,
    page: usize,
    lines: Vec<String>,
}

fn sections(lines: &[Line], max_level: usize) -> Vec<Section> {
    let mut out: Vec<Section> = Vec::new();
    let mut current = Section {
        heading: None,
        page: lines.first().map(|l| l.page).unwrap_or(1),
        lines: Vec::new(),
    };
    for line in lines {
        if let Some((level, heading)) = heading_of(&line.text) {
            if level <= max_level && (current.heading.is_some() || !current.lines.is_empty()) {
                out.push(current);
                current = Section {
                    heading: None,
                    page: line.page,
                    lines: Vec::new(),
                };
            }
            if current.heading.is_none() && level <= max_level {
                current.heading = Some(heading);
                current.page = line.page;
                continue;
            }
        }
        current.lines.push(line.text.clone());
    }
    if current.heading.is_some() || !current.lines.is_empty() {
        out.push(current);
    }
    out
}

fn heading_of(line: &str) -> Option<(usize, String)> {
    let t = line.trim_start();
    let hashes = t.chars().take_while(|c| *c == '#').count();
    if hashes == 0 || hashes > 6 {
        return None;
    }
    let rest = t[hashes..].trim();
    if rest.is_empty() {
        return None;
    }
    Some((hashes, rest.to_string()))
}

fn bullets(lines: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for line in lines {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        let item = t
            .strip_prefix("- ")
            .or_else(|| t.strip_prefix("* "))
            .or_else(|| t.strip_prefix('\u{2022}'));
        if let Some(item) = item {
            let item = item.trim();
            if !item.is_empty() {
                out.push(item.to_string());
            }
        } else if !out.is_empty() && !t.starts_with('#') {
            // A wrapped bullet's continuation line belongs to the bullet
            // above it; before this fix each line became an item of its own.
            let last = out.last_mut().expect("non-empty");
            last.push(' ');
            last.push_str(t);
        }
    }
    out
}

fn item(
    source_id: &str,
    kind: &str,
    title: Option<String>,
    section: Option<String>,
    page: usize,
    text: &str,
    clause_class: Option<String>,
) -> Item {
    // The heading is part of the claim: a prior item titled "M-03:
    // settle_rebate loses the rebate" names the symbol even when the body
    // does not repeat it.
    let mut profile_text = String::new();
    if let Some(section) = &section {
        profile_text.push_str(section);
        profile_text.push('\n');
    }
    profile_text.push_str(text);
    let profile = Profile::from_text(&profile_text);
    Item {
        id: String::new(),
        source_id: source_id.to_string(),
        kind: kind.to_string(),
        title,
        section,
        page,
        quote: evidence_quote(text),
        identifiers: profile.all_identifiers(),
        mechanism_class: mechanism_of(text).map(str::to_string),
        clause_class,
        text: text.trim().to_string(),
    }
}

fn is_reward_heading(h: &str) -> bool {
    let low = h.to_lowercase();
    [
        "reward",
        "payout",
        "severity",
        "bounty",
        "impacts in scope",
        "impact in scope",
    ]
    .iter()
    .any(|n| low.contains(n))
}

fn is_exclusion_heading(h: &str) -> bool {
    let low = h.to_lowercase();
    [
        "out of scope",
        "out-of-scope",
        "exclusion",
        "prohibited",
        "not eligible",
        "ineligible",
        "not in scope",
    ]
    .iter()
    .any(|n| low.contains(n))
}

fn is_scope_heading(h: &str) -> bool {
    let low = h.to_lowercase();
    (low.contains("scope") || low.contains("assets in scope") || low.contains("in-scope"))
        && !is_exclusion_heading(h)
}

fn is_known_heading(h: &str) -> bool {
    let low = h.to_lowercase();
    [
        "known issue",
        "previously known",
        "prior audit",
        "previous audit",
        "already known",
    ]
    .iter()
    .any(|n| low.contains(n))
}

fn is_issue_heading(h: &str) -> bool {
    let trimmed = h.trim().trim_start_matches('#').trim();
    let low = trimmed.to_lowercase();
    if low.starts_with("finding") || low.starts_with("issue ") {
        return true;
    }
    trimmed
        .split(|c: char| !c.is_ascii_alphanumeric() && c != '-')
        .any(|token| {
            let low = token.to_lowercase();
            let mut chars = low.chars();
            let Some(first) = chars.next() else {
                return false;
            };
            if !"chmliqs".contains(first) {
                return false;
            }
            if chars.next() != Some('-') {
                return false;
            }
            let rest: String = chars.collect();
            (1..=3).contains(&rest.len()) && rest.chars().all(|c| c.is_ascii_digit())
        })
}

fn extract_program_page(text: &str, source_id: &str, title: &str) -> Vec<Item> {
    let mut out = Vec::new();
    for section in sections(&lines_with_pages(text), 3) {
        let heading = section.heading.clone();
        let h = heading.as_deref().unwrap_or("");
        if is_exclusion_heading(h) {
            for b in bullets(&section.lines) {
                let class = vocab::classify_clause(&b).map(str::to_string);
                out.push(item(
                    source_id,
                    "scope-clause",
                    Some(title.to_string()),
                    heading.clone(),
                    section.page,
                    &b,
                    class,
                ));
            }
        } else if is_reward_heading(h) {
            for b in bullets(&section.lines) {
                out.push(item(
                    source_id,
                    "payout-tier",
                    Some(title.to_string()),
                    heading.clone(),
                    section.page,
                    &b,
                    None,
                ));
            }
        } else if is_scope_heading(h) || is_known_heading(h) {
            let kind = if is_known_heading(h) {
                "known-issue"
            } else {
                "scope-path"
            };
            for b in bullets(&section.lines) {
                out.push(item(
                    source_id,
                    kind,
                    Some(title.to_string()),
                    heading.clone(),
                    section.page,
                    &b,
                    None,
                ));
            }
        }
    }
    if out.is_empty() {
        // A page with no bullets is still evidence: index its sections so a
        // finding can match its text.
        for section in sections(&lines_with_pages(text), 2) {
            let body = section.lines.join("\n");
            if body.trim().chars().count() < 40 {
                continue;
            }
            out.push(item(
                source_id,
                "page-section",
                Some(title.to_string()),
                section.heading.clone(),
                section.page,
                &body,
                None,
            ));
        }
    }
    out
}

fn extract_known_issues(text: &str, source_id: &str, title: &str, is_yaml: bool) -> Vec<Item> {
    if is_yaml {
        if let Some(items) = known_issues_yaml(text, source_id, title) {
            return items;
        }
    }
    let lines = lines_with_pages(text);
    let mut out = Vec::new();
    for section in sections(&lines, 3) {
        let body = section.lines.join("\n");
        let heading = section.heading.clone();
        let kind = "known-issue";
        let bullets_here = bullets(&section.lines);
        if bullets_here.is_empty() {
            if body.trim().chars().count() >= 20 {
                out.push(item(
                    source_id,
                    kind,
                    Some(title.to_string()),
                    heading,
                    section.page,
                    &body,
                    None,
                ));
            }
        } else {
            for b in bullets_here {
                out.push(item(
                    source_id,
                    kind,
                    Some(title.to_string()),
                    heading.clone(),
                    section.page,
                    &b,
                    None,
                ));
            }
        }
    }
    out
}

#[derive(Debug, serde::Deserialize)]
struct KnownIssueRow {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    summary: Option<String>,
    #[serde(default)]
    text: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct KnownIssueFile {
    #[serde(default)]
    known_issues: Vec<KnownIssueRow>,
}

fn known_issues_yaml(text: &str, source_id: &str, title: &str) -> Option<Vec<Item>> {
    let rows: Vec<KnownIssueRow> =
        if let Ok(rows) = serde_saphyr::from_str::<Vec<KnownIssueRow>>(text) {
            rows
        } else {
            let file: KnownIssueFile = serde_saphyr::from_str(text).ok()?;
            file.known_issues
        };
    let mut out = Vec::new();
    for row in rows {
        let Some(body) = row.summary.or(row.text) else {
            continue;
        };
        if body.trim().is_empty() {
            continue;
        }
        let section = row.id.clone();
        out.push(item(
            source_id,
            "known-issue",
            Some(title.to_string()),
            section,
            1,
            &body,
            None,
        ));
    }
    Some(out)
}

fn extract_audit(text: &str, source_id: &str, title: &str) -> Vec<Item> {
    let lines = lines_with_pages(text);
    let issue_sections: Vec<Section> = sections(&lines, 4)
        .into_iter()
        .filter(|s| s.heading.as_deref().is_some_and(is_issue_heading))
        .collect();
    let chosen = if issue_sections.is_empty() {
        let h2: Vec<Section> = sections(&lines, 2)
            .into_iter()
            .filter(|s| s.heading.is_some())
            .collect();
        if h2.is_empty() {
            let body = text.trim();
            if body.chars().count() < 20 {
                return Vec::new();
            }
            return vec![item(
                source_id,
                "audit-section",
                Some(title.to_string()),
                None,
                1,
                body,
                None,
            )];
        }
        h2
    } else {
        issue_sections
    };
    let mut out = Vec::new();
    for section in chosen {
        let body = section.lines.join("\n");
        let heading = section.heading.clone();
        let kind = if heading.as_deref().is_some_and(is_issue_heading) {
            "audit-finding"
        } else {
            "audit-section"
        };
        if body.trim().chars().count() < 20 {
            continue;
        }
        out.push(item(
            source_id,
            kind,
            Some(title.to_string()),
            heading,
            section.page,
            &body,
            None,
        ));
    }
    out
}

/// Whitespace-collapsed one-line excerpt, used when printing an item.
pub fn excerpt(text: &str) -> String {
    collapse(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_is_stable_and_rejects_empty_input() {
        assert_eq!(slug("EXAMPLE LEND"), Some("example-lend".into()));
        assert_eq!(slug("  "), None);
        assert_eq!(slug("../.."), None);
        assert_eq!(slug("a.b_c-d"), Some("a.b_c-d".into()));
    }

    #[test]
    fn headings_and_pages_are_tracked() {
        let text = "# Rewards\n- Critical: $50,000\n\u{0c}# Second page\n- High: $10,000\n";
        let sections = sections(&lines_with_pages(text), 3);
        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0].heading.as_deref(), Some("Rewards"));
        assert_eq!(sections[0].page, 1);
        assert_eq!(sections[1].page, 2, "form feed starts page 2");
    }

    #[test]
    fn audit_findings_are_split_on_issue_headings() {
        let text = "# Audit report\n## H-01 Rebate rounding\nThe settle_rebate helper rounds the rebate down twice.\n## M-02 Oracle\nThe oracle price can be stale.\n";
        let items = extract_audit(text, "src-0001", "audit.md");
        assert_eq!(items.len(), 2, "{items:?}");
        assert_eq!(items[0].section.as_deref(), Some("H-01 Rebate rounding"));
        assert_eq!(items[0].kind, "audit-finding");
    }

    #[test]
    fn out_of_scope_bullets_are_classified_from_the_taxonomy() {
        let text = "# Scope\n## In scope\n- src/Vault.sol\n## Out of scope\n- Attacks requiring access to privileged addresses\n";
        let items = extract_program_page(text, "src-0001", "scope.md");
        let clause = items
            .iter()
            .find(|i| i.kind == "scope-clause")
            .expect("a clause item");
        assert_eq!(clause.clause_class.as_deref(), Some("privileged_role"));
        assert!(items.iter().any(|i| i.kind == "scope-path"));
    }

    #[test]
    fn an_uncompressed_pdf_literal_is_scraped() {
        let pdf = b"BT (settle_rebate rounds the rebate down twice) Tj ET";
        let text = scrape_pdf_literals(pdf);
        assert!(text.contains("settle_rebate"), "{text}");
    }

    #[test]
    fn a_symlinked_pdf_is_refused_before_it_is_read() {
        let dir = scratch("symlink-pdf");
        let real = dir.join("real.pdf");
        fs::write(&real, b"%PDF-1.4\nBT (settle_rebate) Tj ET\n").unwrap();
        let link = dir.join("link.pdf");
        let made = {
            #[cfg(unix)]
            {
                std::os::unix::fs::symlink(&real, &link).is_ok()
            }
            #[cfg(windows)]
            {
                std::os::windows::fs::symlink_file(&real, &link).is_ok()
            }
        };
        if !made {
            eprintln!("SKIP: this account cannot create a file symlink");
            return;
        }
        let err = pdf_to_text_with(&link, false).expect_err("a symlink is not read");
        assert!(err.contains("symlink"), "{err}");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_utf16be_pdf_literal_is_decoded() {
        let mut pdf = b"BT (".to_vec();
        pdf.extend_from_slice(&[0xFE, 0xFF]);
        for unit in "settle_rebate rounds the rebate down twice".encode_utf16() {
            pdf.extend_from_slice(&unit.to_be_bytes());
        }
        pdf.extend_from_slice(b") Tj ET");
        let text = scrape_pdf_literals(&pdf);
        assert!(
            text.contains("settle_rebate rounds the rebate down twice"),
            "{text:?}"
        );
        assert!(!text.contains('\u{0}'), "NUL characters survived: {text:?}");
    }

    #[test]
    fn a_utf16le_pdf_literal_is_decoded() {
        let mut pdf = b"BT (".to_vec();
        pdf.extend_from_slice(&[0xFF, 0xFE]);
        for unit in "FeeLedger.distributeFees".encode_utf16() {
            pdf.extend_from_slice(&unit.to_le_bytes());
        }
        pdf.extend_from_slice(b") Tj ET");
        let text = scrape_pdf_literals(&pdf);
        assert!(text.contains("FeeLedger.distributeFees"), "{text:?}");
        assert!(!text.contains('\u{0}'), "NUL characters survived: {text:?}");
    }

    #[test]
    fn a_utf8_pdf_literal_is_not_read_as_latin1() {
        let mut pdf = b"BT (r".to_vec();
        pdf.extend_from_slice("é".as_bytes());
        pdf.extend_from_slice(b"serve) Tj ET");
        let text = scrape_pdf_literals(&pdf);
        assert!(text.contains("réserve"), "{text:?}");
        assert!(
            !text.contains('\u{00c3}'),
            "UTF-8 was read as Latin-1: {text:?}"
        );
    }

    #[test]
    fn pdf_octal_escapes_are_decoded() {
        let text = scrape_pdf_literals(b"BT (\\101\\102\\103) Tj ET");
        assert!(text.contains("ABC"), "{text:?}");
    }

    /// A fresh directory for one test's files.
    fn scratch(label: &str) -> PathBuf {
        static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = N.fetch_add(1, Ordering::SeqCst);
        let dir =
            std::env::temp_dir().join(format!("priorart-unit-{}-{label}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Write a stand-in for pdftotext whose body is a shell script, and return
    /// the program to run. Unix: an executable script. Windows: a .bat that runs
    /// the script under bash, as realism-gate's fake forge does. Written under a
    /// temporary name and renamed into place, so nothing execs a file still open
    /// for writing.
    fn fake_pdftotext(dir: &Path, body: &str) -> PathBuf {
        let script = dir.join(if cfg!(windows) {
            "pdftotext.sh"
        } else {
            "pdftotext"
        });
        let tmp = dir.join("script.tmp");
        fs::write(&tmp, format!("#!/bin/sh\n{body}\n")).unwrap();
        fs::rename(&tmp, &script).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
            script
        }
        #[cfg(windows)]
        {
            let bash = [
                "C:\\Program Files\\Git\\bin\\bash.exe",
                "C:\\Program Files\\Git\\usr\\bin\\bash.exe",
            ]
            .into_iter()
            .find(|p| Path::new(p).is_file())
            .unwrap_or("bash");
            let bat = dir.join("pdftotext.bat");
            fs::write(
                &bat,
                format!(
                    "@echo off\r\nset \"d=%~dp0\"\r\nset \"d=%d:\\=/%\"\r\n\"{bash}\" \"%d%pdftotext.sh\" %*\r\n"
                ),
            )
            .unwrap();
            bat
        }
    }

    #[test]
    fn pdftotext_is_looked_up_on_absolute_path_entries_only() {
        let dir = scratch("path");
        let name = if cfg!(windows) {
            "pdftotext.exe"
        } else {
            "pdftotext"
        };
        fs::write(dir.join(name), b"not run").unwrap();
        // A relative entry that holds a pdftotext is never used.
        let rel = PathBuf::from("target").join(format!("priorart-rel-{}", std::process::id()));
        fs::create_dir_all(&rel).unwrap();
        fs::write(rel.join(name), b"not run").unwrap();

        let path_var = std::env::join_paths([rel.as_path(), dir.as_path()]).unwrap();
        assert_eq!(
            find_on_path(Some(path_var.as_os_str())),
            Some(dir.join(name))
        );

        let only_relative = std::env::join_paths([rel.as_path()]).unwrap();
        assert_eq!(find_on_path(Some(only_relative.as_os_str())), None);
        assert_eq!(find_on_path(None), None);
        let _ = fs::remove_dir_all(&rel);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_pdf_path_reaches_pdftotext_absolute_and_never_as_an_option() {
        let dir = scratch("args");
        // Echo the arguments, one per line. The text is then non-empty, so the
        // call succeeds and the test can read what pdftotext was given.
        let program = fake_pdftotext(&dir, "printf '%s\\n' \"$@\"");
        let pdf = Path::new("-evil.pdf");
        let text = run_pdftotext(&program, pdf, Duration::from_secs(30), 1 << 20)
            .expect("the fake echoes its arguments");
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(&lines[..2], ["-layout", "-q"], "{text}");
        let given = Path::new(lines[2]);
        assert!(given.is_absolute(), "not absolute: {}", lines[2]);
        assert_eq!(given.file_name().unwrap(), "-evil.pdf");
        assert_eq!(lines[3], "-");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_pdftotext_that_runs_past_the_timeout_is_killed() {
        let dir = scratch("timeout");
        let program = fake_pdftotext(&dir, "exec sleep 30");
        let started = Instant::now();
        let err = run_pdftotext(
            &program,
            Path::new("a.pdf"),
            Duration::from_secs(1),
            1 << 20,
        )
        .expect_err("it must time out");
        assert!(err.contains("did not finish within 1 s"), "{err}");
        assert!(
            started.elapsed() < Duration::from_secs(20),
            "the timeout did not stop the wait: {:?}",
            started.elapsed()
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_pdftotext_that_floods_stdout_is_killed_at_the_cap() {
        let dir = scratch("flood");
        let program = fake_pdftotext(&dir, "exec yes 0123456789abcdef");
        let err = run_pdftotext(
            &program,
            Path::new("a.pdf"),
            Duration::from_secs(60),
            1 << 20,
        )
        .expect_err("it must hit the output cap");
        assert!(err.contains("wrote more than 1 MiB"), "{err}");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_failing_pdftotext_reports_its_stderr() {
        let dir = scratch("fail");
        let program = fake_pdftotext(&dir, "echo broken-pdf >&2; exit 3");
        let err = run_pdftotext(
            &program,
            Path::new("a.pdf"),
            Duration::from_secs(30),
            1 << 20,
        )
        .expect_err("exit 3 is a failure");
        assert!(err.contains("pdftotext failed (broken-pdf)"), "{err}");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn input_size_is_checked_per_file_and_in_total() {
        let dir = scratch("sizes");
        let a = dir.join("a.md");
        let b = dir.join("b.md");
        fs::write(&a, [b'x'; 10]).unwrap();
        fs::write(&b, [b'y'; 10]).unwrap();
        let one = vec![(a.clone(), "audit")];
        let err = check_input_limits(&one, 10, 5, 100).expect_err("10 bytes over a 5-byte cap");
        assert!(err.contains("a.md") && err.contains("up to"), "{err}");
        let two = vec![(a, "audit"), (b, "audit")];
        let err = check_input_limits(&two, 10, 100, 15).expect_err("20 bytes over a 15-byte total");
        assert!(err.contains("total"), "{err}");
        let err =
            check_input_limits(&two, 1, 100, 100).expect_err("two documents over a count of one");
        assert!(err.contains("2 input documents"), "{err}");
        assert!(check_input_limits(&two, 2, 10, 20).is_ok());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_audits_walk_stops_at_its_depth_and_entry_limits() {
        let dir = scratch("walk");
        let deep = dir.join("a").join("b").join("c");
        fs::create_dir_all(&deep).unwrap();
        fs::write(deep.join("x.md"), "x").unwrap();
        let mut walk = Walk::default();
        let limits = WalkLimits {
            depth: 2,
            entries: 100,
        };
        let err = collect_files(&dir, &mut walk, limits).expect_err("depth 3 is over a limit of 2");
        assert!(err.contains("nested more than 2"), "{err}");

        let flat = scratch("walk-flat");
        for i in 0..5 {
            fs::write(flat.join(format!("{i}.md")), "x").unwrap();
        }
        let mut walk = Walk::default();
        let limits = WalkLimits {
            depth: 16,
            entries: 3,
        };
        let err = collect_files(&flat, &mut walk, limits).expect_err("5 entries over a limit of 3");
        assert!(err.contains("more than 3 entries"), "{err}");
        let _ = fs::remove_dir_all(&dir);
        let _ = fs::remove_dir_all(&flat);
    }

    #[test]
    fn a_heading_identifier_is_part_of_the_item() {
        let it = item(
            "src",
            "audit-finding",
            None,
            Some("M-03 settle_rebate loses the rebate".to_string()),
            1,
            "The body names no symbol.",
            None,
        );
        assert!(
            it.identifiers.iter().any(|i| i.contains("settle_rebate")),
            "{:?}",
            it.identifiers
        );
    }

    #[test]
    fn a_wrapped_bullet_is_one_item_not_several() {
        let lines = vec![
            "- first line".to_string(),
            "  wrapped line".to_string(),
            "- second".to_string(),
        ];
        assert_eq!(
            bullets(&lines),
            vec!["first line wrapped line".to_string(), "second".to_string()]
        );
    }
}
