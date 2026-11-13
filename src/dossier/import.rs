//! Read a saved, offline program folder into sources and items.
//!
//! Nothing here fetches a URL and nothing invents content. A page is a file
//! the user saved; a PDF is read from disk, with pdftotext as a fallback when
//! the uncompressed scrape finds nothing. A PDF that cannot be read is a
//! warning and a skipped source, never a panic.

use crate::dossier::sha256;
use crate::matching::{collapse, evidence_quote, mechanism_of, Profile};
use crate::model::{Item, Payout, PriorAudit, Program, Source};
use crate::vocab;
use std::fs;
use std::path::{Path, PathBuf};

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

/// Read a program directory. Returns a usage error only when the directory or
/// its layout is unusable; a single unreadable input is a note.
pub fn read_program_dir(dir: &Path, program_override: Option<&str>) -> Result<Imported, String> {
    if !dir.is_dir() {
        return Err(format!(
            "{} is not a directory. Point priorart build at a folder of saved inputs; priorart init scaffolds one.",
            dir.display()
        ));
    }
    let mut notes = Vec::new();
    let program_file = read_program_file(dir, &mut notes)?;
    let program = program_of(dir, program_override, program_file.as_ref())?;

    let mut inputs: Vec<Input> = Vec::new();
    // Order is fixed: program page, known issues, then audits by file name.
    // The dossier's item order and ids follow it, so two builds on the same
    // folder are byte-identical.
    if let Some(path) = find_one(dir, &["scope.md", "scope.html", "scope.txt"], &mut notes)? {
        inputs.push(Input::new(&path, "program-page", dir)?);
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
        inputs.push(Input::new(&path, "known-issues", dir)?);
    }
    let audits_dir = dir.join("audits");
    if audits_dir.is_dir() {
        let mut files = Vec::new();
        collect_files(&audits_dir, &mut files)?;
        // A README in audits/ is scaffolding, not a prior audit.
        files.retain(|p| {
            !p.file_name()
                .is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case("README.md"))
        });
        files.sort();
        for path in files {
            inputs.push(Input::new(&path, "audit", dir)?);
        }
    } else if dir.join("audits").exists() {
        notes.push("audits exists but is not a directory; ignored".into());
    }

    if inputs.is_empty() {
        return Err(format!(
            "no inputs found in {}. Expected scope.md (or .html/.txt), known-issues.md/.yaml/.txt, or audits/*.md/.txt/.pdf. Run priorart init to scaffold the layout.",
            dir.display()
        ));
    }

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
    if !path.exists() {
        return Ok(None);
    }
    let text = read_text(&path)?;
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
    let found: Vec<PathBuf> = names
        .iter()
        .map(|n| dir.join(n))
        .filter(|p| p.is_file())
        .collect();
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

fn collect_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let rd = fs::read_dir(dir).map_err(|e| format!("cannot read {}: {e}", dir.display()))?;
    for entry in rd.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        let Ok(meta) = fs::symlink_metadata(&path) else {
            continue;
        };
        if meta.file_type().is_symlink() {
            continue;
        }
        if meta.is_dir() {
            collect_files(&path, out)?;
        } else if is_doc(&path) {
            out.push(path);
        }
    }
    Ok(())
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
    fn new(path: &Path, kind: &str, root: &Path) -> Result<Self, String> {
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
            match pdf_to_text(path) {
                Ok(t) => {
                    pages = 1 + t.matches('\u{0c}').count();
                    text = Some(t);
                }
                Err(e) => warning = Some(e),
            }
        } else {
            let raw = String::from_utf8_lossy(&bytes).to_string();
            let t = strip_bom(&raw).replace("\r\n", "\n").replace('\r', "\n");
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

fn read_text(path: &Path) -> Result<String, String> {
    let bytes = fs::read(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    Ok(strip_bom(&String::from_utf8_lossy(&bytes)).replace("\r\n", "\n"))
}

pub fn strip_bom(s: &str) -> String {
    s.strip_prefix('\u{feff}').unwrap_or(s).to_string()
}

/// Extract text from a PDF: the uncompressed literal scrape first, then
/// pdftotext. A missing pdftotext is a clear error message, never a panic.
pub fn pdf_to_text(path: &Path) -> Result<String, String> {
    let bytes = fs::read(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let scraped = scrape_pdf_literals(&bytes);
    let letters = scraped.chars().filter(|c| c.is_alphanumeric()).count();
    if letters > 40 {
        return Ok(scraped);
    }
    match std::process::Command::new("pdftotext")
        .arg("-layout")
        .arg("-q")
        .arg(path)
        .arg("-")
        .output()
    {
        Ok(out) if out.status.success() => {
            let text = String::from_utf8_lossy(&out.stdout).to_string();
            if text.trim().is_empty() {
                Err(format!("{}: pdftotext returned no text", path.display()))
            } else {
                Ok(text)
            }
        }
        Ok(out) => Err(format!(
            "{}: pdftotext failed ({})",
            path.display(),
            String::from_utf8_lossy(&out.stderr).trim()
        )),
        Err(_) => Err(format!(
            "{}: no extractable text (the uncompressed scrape found nothing and pdftotext is not installed). Convert it with pdftotext -layout and pass the .txt.",
            path.display()
        )),
    }
}

/// Pull literal strings out of an uncompressed PDF. Compressed streams are
/// left for pdftotext. This is enough for a text-showing PDF that was not
/// deflated, and it does not pretend to be a PDF parser.
pub fn scrape_pdf_literals(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    let mut out = String::new();
    let b = text.as_bytes();
    let mut i = 0usize;
    while i < b.len() {
        if b[i] == b'(' {
            let mut j = i + 1;
            let mut buf = String::new();
            let mut escaped = false;
            let mut depth = 1i32;
            while j < b.len() {
                let c = b[j] as char;
                if escaped {
                    buf.push(match c {
                        'n' => '\n',
                        'r' => '\r',
                        't' => '\t',
                        other => other,
                    });
                    escaped = false;
                    j += 1;
                    continue;
                }
                match c {
                    '\\' => escaped = true,
                    '(' => {
                        depth += 1;
                        buf.push(c);
                    }
                    ')' => {
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
            let after = text[j..].trim_start();
            if (after.starts_with("Tj") || after.starts_with('\'') || after.starts_with("TJ"))
                && !buf.trim().is_empty()
            {
                out.push_str(buf.trim());
                out.push('\n');
            }
            i = j;
        } else {
            i += 1;
        }
    }
    out
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
    let mut out = Vec::new();
    for line in lines {
        let t = line.trim();
        let item = t
            .strip_prefix("- ")
            .or_else(|| t.strip_prefix("* "))
            .or_else(|| t.strip_prefix('\u{2022}'));
        if let Some(item) = item {
            let item = item.trim();
            if !item.is_empty() {
                out.push(item.to_string());
            }
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
    let profile = Profile::from_text(text);
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
        assert_eq!(slug("RAIN USDR"), Some("rain-usdr".into()));
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
        let text = "# Audit report\n## H-01 Limiter refund\nThe reduce_outflow helper destroys the refund.\n## M-02 Oracle\nThe oracle price can be stale.\n";
        let items = extract_audit(text, "src-0001", "audit.md");
        assert_eq!(items.len(), 2, "{items:?}");
        assert_eq!(items[0].section.as_deref(), Some("H-01 Limiter refund"));
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
        let pdf = b"BT (reduce_outflow credits the current segment only) Tj ET";
        let text = scrape_pdf_literals(pdf);
        assert!(text.contains("reduce_outflow"), "{text}");
    }
}
