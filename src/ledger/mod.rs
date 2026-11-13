//! Ledger schema v1 validation: the Rust port of bounty-economics
//! `scripts/validate.py`.
//!
//! The ledger is the researcher's private transcription of a program's terms.
//! The schema (schemas/ledger.schema.json, shipped byte-identical) checks
//! unknown keys, types, enums and formats; this module also applies the rules a
//! schema cannot express - above all that every entry is CITED, and that an
//! unstated floor is never read as "Low pays".
//!
//! Output shape and exit codes are the Python validator's, so the ledger
//! fixture files from its test suite remain golden files:
//!   0 valid, 1 validation errors (or warnings under --strict), 2 parse/usage.

use crate::vocab;
use regex::RegexBuilder;
use serde_json::Value;
use std::path::{Path, PathBuf};

/// The ledger format version this build implements.
pub const SCHEMA_VERSION: u32 = 1;

/// Days after which a `retrieved` date draws a staleness warning.
pub const STALE_DAYS: i64 = 180;

const FLOOR_STATUSES: [&str; 3] = ["documented", "unstated", "conflicting"];
const FLOOR_SEVERITIES: [&str; 4] = ["critical", "high", "medium", "low"];

/// Keys the schema allows on one entry.
const ENTRY_KEYS: [&str; 13] = [
    "schema_version",
    "id",
    "tier_shape",
    "floor_status",
    "floor_severity",
    "source_url",
    "retrieved",
    "conflicts",
    "exclusions",
    "prior_audits",
    "post_audit_delta",
    "notes",
    "engagement",
];

const EXCLUSION_KEYS: [&str; 3] = ["class", "clause", "pattern"];
const AUDIT_KEYS: [&str; 4] = ["firm", "url", "covers", "known_issues"];

/// One ledger file's findings.
#[derive(Debug, Clone, serde::Serialize)]
pub struct LedgerReport {
    pub ledger: String,
    pub example_mode: bool,
    pub entries: usize,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

/// The whole run.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Report {
    pub schema_version: u32,
    pub ok: bool,
    pub strict: bool,
    pub ledgers: Vec<LedgerReport>,
    pub entries: usize,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ledger: Option<String>,
}

/// Validate one or more ledger files. `Err` is a parse or IO failure (exit 2).
pub fn validate(paths: &[PathBuf], strict: bool, example_mode: bool) -> Result<Report, String> {
    if paths.is_empty() {
        return Err("a ledger path is required. Pass at least one YAML file.".to_string());
    }
    for path in paths {
        if !path.exists() {
            return Err(format!("{} not found", path.display()));
        }
    }
    let mut ledgers = Vec::new();
    for path in paths {
        let is_example = example_mode || path.to_string_lossy().ends_with(".example.yaml");
        ledgers.push(validate_file(path, is_example)?);
    }
    let several = ledgers.len() > 1;
    let label = |path: &str, message: &str| {
        if several {
            format!("{path}: {message}")
        } else {
            message.to_string()
        }
    };
    let all_errors: Vec<String> = ledgers
        .iter()
        .flat_map(|r| r.errors.iter().map(move |m| label(&r.ledger, m)))
        .collect();
    let all_warnings: Vec<String> = ledgers
        .iter()
        .flat_map(|r| r.warnings.iter().map(move |m| label(&r.ledger, m)))
        .collect();
    let entries = ledgers.iter().map(|r| r.entries).sum();
    let ok = all_errors.is_empty() && !(strict && !all_warnings.is_empty());
    Ok(Report {
        schema_version: SCHEMA_VERSION,
        ok,
        strict,
        ledgers,
        entries,
        errors: all_errors,
        warnings: all_warnings,
        ledger: if several {
            None
        } else {
            Some(paths[0].to_string_lossy().replace('\\', "/"))
        },
    })
}

fn validate_file(path: &Path, example_mode: bool) -> Result<LedgerReport, String> {
    let raw = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let text = crate::dossier::import::strip_bom(&raw).replace("\r\n", "\n");
    let documents = split_documents(&text);
    if documents.len() != 1 {
        return Err(format!(
            "ledger must be a single YAML document ({} found): {}",
            documents.len(),
            path.display()
        ));
    }
    let data: Value = serde_saphyr::from_str(&documents[0])
        .map_err(|e| format!("cannot parse YAML in {}: {e}", path.display()))?;
    if data.is_null() {
        return Err(format!("ledger is empty: {}", path.display()));
    }
    let Some(entries) = data.as_array() else {
        return Err(format!(
            "ledger must be a list of entries: {}",
            path.display()
        ));
    };
    let tier_shapes: Vec<String> = vocab::tier_shape_ids()
        .iter()
        .map(|s| s.to_string())
        .collect();
    let clause_classes: Vec<String> = vocab::clause_class_ids()
        .iter()
        .map(|s| s.to_string())
        .collect();
    let today = today_days();
    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    for (index, entry) in entries.iter().enumerate() {
        let (mut e, mut w) = validate_entry(
            entry,
            index,
            &tier_shapes,
            &clause_classes,
            example_mode,
            today,
        );
        errors.append(&mut e);
        warnings.append(&mut w);
        if let Some(id) = entry.get("id").and_then(Value::as_str) {
            if seen.iter().any(|s| s == id) {
                errors.push(format!("duplicate id: {id:?}"));
            }
            seen.push(id.to_string());
        }
    }
    Ok(LedgerReport {
        ledger: path.to_string_lossy().replace('\\', "/"),
        example_mode,
        entries: entries.len(),
        errors,
        warnings,
    })
}

/// Split a YAML stream into documents. Only a line that is exactly the
/// separator counts, which is what YAML itself requires.
fn split_documents(text: &str) -> Vec<String> {
    let mut docs = Vec::new();
    let mut cur = String::new();
    for line in text.lines() {
        if line.trim_end() == "---" {
            docs.push(std::mem::take(&mut cur));
        } else {
            cur.push_str(line);
            cur.push('\n');
        }
    }
    docs.push(cur);
    docs.retain(|d| !d.trim().is_empty());
    docs
}

fn validate_entry(
    entry: &Value,
    index: usize,
    tier_shapes: &[String],
    clause_classes: &[String],
    example_mode: bool,
    today: i64,
) -> (Vec<String>, Vec<String>) {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    let mut where_ = format!("entry[{index}]");
    let Some(obj) = entry.as_object() else {
        return (vec![format!("{where_}: not a mapping")], warnings);
    };
    if let Some(id) = obj.get("id").and_then(Value::as_str) {
        where_ = format!("entry[{index}] ({id})");
        if !is_slug(id) {
            errors.push(format!("{where_}: id must match ^[a-z0-9][a-z0-9._-]*$"));
        }
    } else {
        errors.push(format!("{where_}: missing required field 'id'"));
    }

    // --- schema: unknown keys and types ---------------------------------
    for key in obj.keys() {
        if !ENTRY_KEYS.contains(&key.as_str()) {
            errors.push(format!(
                "{where_}: schema: (entry): Additional properties are not allowed ('{key}' was unexpected)"
            ));
        }
    }
    for key in ["schema_version", "conflicts", "post_audit_delta", "notes"] {
        if let Some(v) = obj.get(key) {
            let ok = match key {
                "schema_version" => v.is_u64(),
                "conflicts" => v.as_array().is_some_and(|a| a.iter().all(Value::is_string)),
                _ => v.is_null() || v.is_string(),
            };
            if !ok {
                errors.push(format!(
                    "{where_}: schema: {key}: is not the type the schema declares"
                ));
            }
        }
    }

    // --- ledger format version -------------------------------------------
    if let Some(version) = obj.get("schema_version").and_then(Value::as_u64) {
        if version != u64::from(SCHEMA_VERSION) {
            errors.push(format!(
                "{where_}: schema_version {version} is not implemented by this validator (it implements {SCHEMA_VERSION}). Update the tool, or follow the migration notes in CHANGELOG.md."
            ));
        }
    }

    // --- tier shape -------------------------------------------------------
    let shape = obj.get("tier_shape").and_then(Value::as_str);
    match shape {
        None => {
            if !obj.contains_key("tier_shape") {
                errors.push(format!("{where_}: missing required field 'tier_shape'"));
            } else {
                errors.push(format!("{where_}: tier_shape must be a string"));
            }
        }
        Some("any") => errors.push(format!(
            "{where_}: tier_shape 'any' was removed. If the page explicitly states 'SEVERITY FLOOR: NONE' or 'any', use 'low_and_above' with floor_status 'documented' - informational findings are acknowledged rather than paid, so Low is the lowest tier that pays. If the page says nothing about a floor, use 'unknown' with floor_status 'unstated'."
        )),
        Some(other) if !tier_shapes.iter().any(|s| s == other) => errors.push(format!(
            "{where_}: tier_shape {other:?} not in {tier_shapes:?}"
        )),
        Some("unknown") => warnings.push(format!(
            "{where_}: tier_shape is 'unknown' - this is NOT permissive. Hunting an untranscribed program is an unhedged bet."
        )),
        Some(other) => {
            if let Some(spec) = vocab::tier_shape(other) {
                if spec.pays.len() <= 2 {
                    warnings.push(format!(
                        "{where_}: tier_shape '{other}' pays only {}. Confirm your thesis can reach it before hunting.",
                        spec.pays.join("/")
                    ));
                }
            }
        }
    }

    // --- floor consistency -------------------------------------------------
    let floor = match obj.get("floor_severity") {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) => Some(s.as_str()),
        Some(_) => {
            errors.push(format!("{where_}: floor_severity must be a string or null"));
            None
        }
    };
    if floor == Some("informational") {
        errors.push(format!(
            "{where_}: floor_severity 'informational' is not a real floor - no program pays informational in practice. If the page explicitly states no floor, record 'low' with floor_status 'documented'; if it says nothing about a floor, leave floor_severity null with floor_status 'unstated'."
        ));
    } else if let Some(f) = floor {
        if !FLOOR_SEVERITIES.contains(&f) {
            errors.push(format!(
                "{where_}: floor_severity {f:?} not in {FLOOR_SEVERITIES:?}"
            ));
        }
    }
    if let (Some(shape), Some(floor)) = (shape, floor) {
        if let Some(spec) = vocab::tier_shape(shape) {
            if let Some(expected) = spec.floor.as_deref() {
                if floor != expected {
                    warnings.push(format!(
                        "{where_}: tier_shape '{shape}' implies floor '{expected}' but floor_severity is '{floor}'. The payout gate uses the stricter of the two; fix the one that does not match the reward table."
                    ));
                }
            }
        }
    }
    if let (Some(shape), None) = (shape, floor) {
        if shape != "unknown" {
            warnings.push(format!(
                "{where_}: tier_shape is set but floor_severity is empty."
            ));
        }
    }

    // --- floor_status: was the floor READ? --------------------------------
    let status = obj.get("floor_status").and_then(Value::as_str);
    match status {
        None => warnings.push(format!(
            "{where_}: floor_status is missing. Record whether the floor was 'documented', 'unstated' or 'conflicting'. Without it the validator cannot tell a floor you read from one you never looked for, and the payout gate reports no result."
        )),
        Some(s) if !FLOOR_STATUSES.contains(&s) => errors.push(format!(
            "{where_}: floor_status {s:?} not in {FLOOR_STATUSES:?}"
        )),
        Some("documented") => {
            if floor.is_none() {
                errors.push(format!(
                    "{where_}: floor_status is 'documented' but floor_severity is empty. Record the lowest severity with a non-zero reward."
                ));
            }
            if shape == Some("unknown") {
                errors.push(format!(
                    "{where_}: floor_status is 'documented' but tier_shape is 'unknown' - 'unknown' means the page has not been read. Record the shape you read, or set floor_status to 'unstated'."
                ));
            }
        }
        Some("unstated") => {
            if floor.is_some() {
                errors.push(format!(
                    "{where_}: floor_status is 'unstated' but floor_severity is {floor:?}. An unstated floor has no severity: leave floor_severity null and tier_shape 'unknown'."
                ));
            }
            if shape.is_some_and(|s| s != "unknown") {
                errors.push(format!(
                    "{where_}: floor_status is 'unstated' but tier_shape is {shape:?}. An unstated floor is not evidence that Low pays; it is evidence the page has not been read. Keep tier_shape 'unknown' until you find the reward table."
                ));
            } else {
                warnings.push(format!(
                    "{where_}: floor_status is 'unstated' - the floor is not evidence-backed. Do not read it as 'Low pays'. Open the rewards page before spending hours here."
                ));
            }
        }
        Some(_) => {
            // conflicting
            if shape.is_some_and(|s| s != "unknown") {
                errors.push(format!(
                    "{where_}: floor_status is 'conflicting' but tier_shape is {shape:?} - the program's materials disagree, so the shape is not settled. Keep tier_shape 'unknown' until you resolve it."
                ));
            }
            warnings.push(format!(
                "{where_}: the program's own materials disagree about the floor. Record both readings in 'conflicts' rather than resolving it silently."
            ));
        }
    }

    // --- citation ----------------------------------------------------------
    let url = obj.get("source_url").and_then(Value::as_str);
    let url = match url {
        Some(u) => {
            if u != u.trim() {
                warnings.push(format!(
                    "{where_}: source_url has surrounding whitespace. It was checked without the space; quote the URL so every consumer agrees."
                ));
                Some(u.trim().to_string())
            } else {
                Some(u.to_string())
            }
        }
        None => {
            if obj.contains_key("source_url") {
                errors.push(format!("{where_}: source_url must be a string"));
            }
            None
        }
    };
    match url.as_deref() {
        None | Some("") => errors.push(format!(
            "{where_}: missing 'source_url'. An uncited entry is a guess, and a guessed payout floor is worse than none."
        )),
        Some(u) => {
            if !(u.starts_with("http://") || u.starts_with("https://")) {
                errors.push(format!("{where_}: source_url must be http(s)"));
            }
            if let Some(host) = placeholder_host(u) {
                if example_mode {
                    warnings.push(format!(
                        "{where_}: source_url uses a placeholder host ({host}) - expected in an example file, not in a real ledger."
                    ));
                } else {
                    errors.push(format!(
                        "{where_}: source_url still points at a placeholder host ({host}). Replace it with the real page."
                    ));
                }
            }
        }
    }

    // --- retrieved ---------------------------------------------------------
    let retrieved = obj.get("retrieved").and_then(Value::as_str);
    let retrieved = match retrieved {
        Some(r) => {
            if r != r.trim() {
                warnings.push(format!(
                    "{where_}: retrieved has surrounding whitespace. It was checked without the space; quote the date ({:?}).",
                    r.trim()
                ));
                Some(r.trim().to_string())
            } else {
                Some(r.to_string())
            }
        }
        None => {
            if obj.contains_key("retrieved") {
                errors.push(format!("{where_}: retrieved must be a string"));
            }
            None
        }
    };
    match retrieved.as_deref() {
        None | Some("") => errors.push(format!("{where_}: missing 'retrieved' date.")),
        Some(r) => match parse_iso_date(r) {
            None => errors.push(format!("{where_}: retrieved must be YYYY-MM-DD, got {r:?}")),
            Some(days) => {
                let age = today - days;
                if age < 0 {
                    errors.push(format!(
                        "{where_}: retrieved {r} is in the future. Record the date you actually read the page."
                    ));
                } else if age > STALE_DAYS {
                    warnings.push(format!(
                        "{where_}: retrieved {age} days ago. Program terms change without notice - re-read the page before relying on this."
                    ));
                }
            }
        },
    }

    // --- exclusions ---------------------------------------------------------
    match obj.get("exclusions") {
        None | Some(Value::Null) => {}
        Some(Value::Array(items)) => {
            for (j, exc) in items.iter().enumerate() {
                let Some(exc_obj) = exc.as_object() else {
                    errors.push(format!("{where_}: exclusions[{j}] is not a mapping"));
                    continue;
                };
                for key in exc_obj.keys() {
                    if !EXCLUSION_KEYS.contains(&key.as_str()) {
                        errors.push(format!(
                            "{where_}: schema: exclusions.{j}: Additional properties are not allowed ('{key}' was unexpected)"
                        ));
                    }
                }
                let class = exc_obj.get("class").and_then(Value::as_str);
                match class {
                    Some(c) if clause_classes.iter().any(|k| k == c) => {}
                    Some(c) => errors.push(format!(
                        "{where_}: exclusions[{j}].class {c:?} not in {clause_classes:?}"
                    )),
                    None => errors.push(format!(
                        "{where_}: exclusions[{j}].class {class:?} not in {clause_classes:?}"
                    )),
                }
                match exc_obj.get("clause").and_then(Value::as_str) {
                    Some(c) if !c.trim().is_empty() => {}
                    _ => errors.push(format!(
                        "{where_}: exclusions[{j}] missing verbatim 'clause'"
                    )),
                }
                match exc_obj.get("pattern") {
                    None | Some(Value::Null) => {}
                    Some(Value::String(p)) => {
                        let stripped = p.trim();
                        if stripped.is_empty() {
                            errors.push(format!(
                                "{where_}: exclusions[{j}].pattern is empty. Omit the key, or write the expression."
                            ));
                        } else {
                            if stripped != p {
                                errors.push(format!(
                                    "{where_}: exclusions[{j}].pattern has leading or trailing whitespace. The payout gate trims it before compiling, so the two tools would not test the same expression."
                                ));
                            }
                            if let Some(problem) = regex_problem(stripped) {
                                errors.push(format!("{where_}: exclusions[{j}].pattern {problem}"));
                            }
                        }
                    }
                    Some(other) => {
                        let text = value_text(other);
                        if let Some(problem) = regex_problem(&text) {
                            errors.push(format!("{where_}: exclusions[{j}].pattern {problem}"));
                        }
                    }
                }
            }
        }
        Some(_) => errors.push(format!("{where_}: exclusions must be a list")),
    }

    // --- prior audits -------------------------------------------------------
    match obj.get("prior_audits") {
        None | Some(Value::Null) => {}
        Some(Value::Array(items)) => {
            for (j, audit) in items.iter().enumerate() {
                let Some(audit_obj) = audit.as_object() else {
                    errors.push(format!("{where_}: prior_audits[{j}] is not a mapping"));
                    continue;
                };
                for key in audit_obj.keys() {
                    if !AUDIT_KEYS.contains(&key.as_str()) {
                        errors.push(format!(
                            "{where_}: schema: prior_audits.{j}: Additional properties are not allowed ('{key}' was unexpected)"
                        ));
                    }
                }
                match audit_obj.get("firm").and_then(Value::as_str) {
                    Some(f) if !f.trim().is_empty() => {}
                    _ => errors.push(format!("{where_}: prior_audits[{j}] missing 'firm'")),
                }
                if audit_obj
                    .get("url")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .trim()
                    .is_empty()
                {
                    warnings.push(format!(
                        "{where_}: prior_audits[{j}] has no 'url'. Cite the report, as for the floor."
                    ));
                }
                let empty_covers = match audit_obj.get("covers") {
                    Some(Value::Array(a)) => a.is_empty(),
                    Some(Value::Null) | None => true,
                    Some(_) => {
                        errors.push(format!(
                            "{where_}: schema: prior_audits.{j}.covers: must be an array"
                        ));
                        true
                    }
                };
                if empty_covers {
                    warnings.push(format!(
                        "{where_}: prior_audits[{j}] has no 'covers' list. A report naming a file is not the same as covering your function."
                    ));
                }
            }
        }
        Some(_) => errors.push(format!("{where_}: prior_audits must be a list")),
    }

    (errors, warnings)
}

fn value_text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

fn is_slug(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_lowercase() || c.is_ascii_digit() => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-'))
}

// ------------------------------------------------------- regex dialect

/// Why a pattern is outside the dialect both the Python and the Rust engines
/// share, or None. Ported from the Python validator, including the character
/// class handling that keeps `[*+]` from being read as a possessive quantifier.
pub fn dialect_problem(pattern: &str) -> Option<String> {
    let chars: Vec<char> = pattern.chars().collect();
    let n = chars.len();
    let mut i = 0usize;
    let mut in_class = false;
    while i < n {
        let c = chars[i];
        if c == '\\' {
            if i + 1 >= n {
                break;
            }
            let next = chars[i + 1];
            if next.is_ascii_digit() {
                return Some("a backreference or octal escape".into());
            }
            if next == 'Z' {
                return Some("the \\Z end marker (Rust regex recognizes \\z, not \\Z)".into());
            }
            if next == 'g' {
                return Some("a \\g backreference".into());
            }
            if next == 'k' && i + 2 < n && (chars[i + 2] == '<' || chars[i + 2] == '\'') {
                return Some("a named backreference".into());
            }
            i += 2;
            continue;
        }
        if in_class {
            if c == ']' {
                in_class = false;
            }
            i += 1;
            continue;
        }
        if c == '[' {
            in_class = true;
            i += 1;
            if i < n && chars[i] == '^' {
                i += 1;
            }
            if i < n && chars[i] == ']' {
                i += 1;
            }
            continue;
        }
        if pattern[i..].starts_with("(?") {
            let rest: String = chars[i + 2..].iter().collect();
            if rest.starts_with('=')
                || rest.starts_with('!')
                || rest.starts_with("<=")
                || rest.starts_with("<!")
            {
                return Some("lookaround".into());
            }
            if rest.starts_with('>') {
                return Some("an atomic group".into());
            }
            if rest.starts_with('#') {
                return Some("a comment group".into());
            }
            if rest.starts_with('(') {
                return Some("a conditional".into());
            }
            if rest.starts_with("P=") {
                return Some("a named backreference".into());
            }
            if !rest.starts_with("P<") && !rest.starts_with('<') {
                let mut flags = Vec::new();
                for ch in rest.chars() {
                    if ch == ':' || ch == ')' {
                        break;
                    }
                    if "aiLmsux".contains(ch) {
                        flags.push(ch);
                    }
                }
                if flags.iter().any(|f| *f == 'a' || *f == 'L') {
                    return Some("a Python-only inline flag".into());
                }
            }
        }
        if matches!(c, '*' | '+' | '?') && i + 1 < n && chars[i + 1] == '+' {
            return Some("a possessive quantifier".into());
        }
        if c == '}' && i + 1 < n && chars[i + 1] == '+' && brace_quantifier_closes(&chars, i) {
            return Some("a possessive quantifier".into());
        }
        i += 1;
    }
    None
}

fn brace_quantifier_closes(chars: &[char], close: usize) -> bool {
    let mut j = close as isize - 1;
    let mut saw_digit = false;
    while j >= 0 && chars[j as usize].is_ascii_digit() {
        saw_digit = true;
        j -= 1;
    }
    if j >= 0 && chars[j as usize] == ',' {
        j -= 1;
        while j >= 0 && chars[j as usize].is_ascii_digit() {
            saw_digit = true;
            j -= 1;
        }
    }
    saw_digit && j >= 0 && chars[j as usize] == '{'
}

/// Why a pattern is unusable, or None (compiles under the Rust engine and is in
/// the shared dialect).
pub fn regex_problem(pattern: &str) -> Option<String> {
    if let Some(reason) = dialect_problem(pattern) {
        return Some(format!(
            "uses {reason}; the two regex engines do not agree on this construct"
        ));
    }
    match RegexBuilder::new(pattern).case_insensitive(true).build() {
        Ok(_) => None,
        Err(err) => Some(format!("invalid regex: {err}")),
    }
}

// ------------------------------------------------------------ placeholder hosts

/// The reserved documentation domains (RFC 2606), including every subdomain.
const RESERVED_EXAMPLE_DOMAINS: [&str; 3] = ["example.com", "example.org", "example.net"];
const PLACEHOLDER_HOSTS: [&str; 4] = ["localhost", "127.0.0.1", "::1", "0.0.0.0"];
const PLACEHOLDER_SUFFIXES: [&str; 2] = [".invalid", ".localhost"];
const PLACEHOLDER_LABELS: [&str; 3] = ["todo", "tbd", "changeme"];

/// The host of an http(s) URL, lowercased and without brackets or port.
pub fn url_host(url: &str) -> Option<String> {
    let rest = url
        .strip_prefix("http://")
        .or_else(|| url.strip_prefix("https://"))?;
    let authority = rest.split(['/', '?', '#']).next()?;
    let authority = authority.rsplit('@').next()?;
    let host = if let Some(inner) = authority.strip_prefix('[') {
        let end = inner.find(']')?;
        &inner[..end]
    } else {
        authority.split(':').next()?
    };
    let host = host.trim_end_matches('.').to_lowercase();
    if host.is_empty() {
        None
    } else {
        Some(host)
    }
}

/// The placeholder host in `url`, or None. A real host that merely contains
/// the word "todo" is not a placeholder.
pub fn placeholder_host(url: &str) -> Option<String> {
    let host = url_host(url)?;
    if PLACEHOLDER_HOSTS.contains(&host.as_str()) {
        return Some(host);
    }
    if PLACEHOLDER_SUFFIXES.iter().any(|s| host.ends_with(s)) {
        return Some(host);
    }
    if RESERVED_EXAMPLE_DOMAINS
        .iter()
        .any(|root| host == *root || host.ends_with(&format!(".{root}")))
    {
        return Some(host);
    }
    if host
        .split('.')
        .any(|label| PLACEHOLDER_LABELS.contains(&label))
    {
        return Some(host);
    }
    None
}

// -------------------------------------------------------------------- dates

/// Days since 1970-01-01 for a UTC calendar date (Howard Hinnant's algorithm).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The inverse: (year, month, day) from days since the epoch.
fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Days since the epoch for a strict YYYY-MM-DD date, or None when the string
/// is not a real calendar date.
pub fn parse_iso_date(iso: &str) -> Option<i64> {
    let b = iso.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return None;
    }
    let y: i64 = iso[0..4].parse().ok()?;
    let m: i64 = iso[5..7].parse().ok()?;
    let d: i64 = iso[8..10].parse().ok()?;
    if !(1..=12).contains(&m) {
        return None;
    }
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let max = match m {
        2 if leap => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    if !(1..=max).contains(&d) {
        return None;
    }
    Some(days_from_civil(y, m, d))
}

/// Today in days since the epoch, UTC - the same clock the Python validator
/// uses, so a date cannot be "future" in one tool and past in the other.
pub fn today_days() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
        .div_euclid(86_400)
}

/// Today as YYYY-MM-DD, UTC. Used by tests and by the taxonomies payload.
pub fn today_iso() -> String {
    let (y, m, d) = civil_from_days(today_days());
    format!("{y:04}-{m:02}-{d:02}")
}

/// The `--taxonomy --json` payload: the shipped vocabularies, machine-readable.
pub fn taxonomy_payload() -> Result<Value, String> {
    let tiers: Value = serde_saphyr::from_str(vocab::TIER_SHAPES_YAML)
        .map_err(|e| format!("tier-shapes.yaml did not parse: {e}"))?;
    let classes: Value = serde_saphyr::from_str(vocab::CLAUSE_CLASSES_YAML)
        .map_err(|e| format!("clause-classes.yaml did not parse: {e}"))?;
    let tier_items = tiers
        .get("shapes")
        .cloned()
        .unwrap_or_else(|| Value::Array(Vec::new()));
    let class_items = classes
        .get("classes")
        .cloned()
        .unwrap_or_else(|| Value::Array(Vec::new()));
    Ok(serde_json::json!({
        "schema_version": SCHEMA_VERSION,
        "tier_shapes": {
            "source": "vocab/tier-shapes.yaml",
            "count": tier_items.as_array().map(|a| a.len()).unwrap_or(0),
            "items": tier_items,
        },
        "clause_classes": {
            "source": "vocab/clause-classes.yaml",
            "count": class_items.as_array().map(|a| a.len()).unwrap_or(0),
            "items": class_items,
        },
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(overrides: &[(&str, Value)]) -> Value {
        let mut map = serde_json::Map::new();
        map.insert("id".into(), Value::String("test-program".into()));
        map.insert(
            "tier_shape".into(),
            Value::String("medium_and_above".into()),
        );
        map.insert("floor_status".into(), Value::String("documented".into()));
        map.insert("floor_severity".into(), Value::String("medium".into()));
        map.insert(
            "source_url".into(),
            Value::String("https://docs.real-program.org/rewards".into()),
        );
        map.insert("retrieved".into(), Value::String("2026-10-01".into()));
        for (k, v) in overrides {
            if v.is_null() {
                map.remove(*k);
            } else {
                map.insert((*k).into(), v.clone());
            }
        }
        Value::Object(map)
    }

    fn check(entry: &Value) -> (Vec<String>, Vec<String>) {
        let tiers: Vec<String> = vocab::tier_shape_ids()
            .iter()
            .map(|s| s.to_string())
            .collect();
        let classes: Vec<String> = vocab::clause_class_ids()
            .iter()
            .map(|s| s.to_string())
            .collect();
        validate_entry(
            entry,
            0,
            &tiers,
            &classes,
            false,
            days_from_civil(2026, 11, 12),
        )
    }

    #[test]
    fn a_minimal_cited_entry_passes() {
        let (errors, _) = check(&entry(&[]));
        assert!(errors.is_empty(), "{errors:?}");
    }

    #[test]
    fn an_uncited_entry_fails() {
        let (errors, _) = check(&entry(&[("source_url", Value::Null)]));
        assert!(
            errors.iter().any(|e| e.contains("source_url")),
            "{errors:?}"
        );
        let (errors, _) = check(&entry(&[("retrieved", Value::Null)]));
        assert!(errors.iter().any(|e| e.contains("retrieved")), "{errors:?}");
    }

    #[test]
    fn an_unstated_floor_beside_low_and_above_is_an_error() {
        let (errors, _) = check(&entry(&[
            ("tier_shape", Value::String("low_and_above".into())),
            ("floor_severity", Value::Null),
            ("floor_status", Value::String("unstated".into())),
        ]));
        assert!(
            errors.join(" ").contains("not evidence that Low pays"),
            "{errors:?}"
        );
    }

    #[test]
    fn documented_floor_requires_a_severity_and_a_settled_shape() {
        let (errors, _) = check(&entry(&[
            ("tier_shape", Value::String("unknown".into())),
            ("floor_severity", Value::Null),
        ]));
        assert!(errors.join(" ").contains("documented"), "{errors:?}");
    }

    #[test]
    fn conflicting_floor_with_a_settled_shape_is_an_error() {
        let (errors, _) = check(&entry(&[(
            "floor_status",
            Value::String("conflicting".into()),
        )]));
        assert!(errors.join(" ").contains("conflicting"), "{errors:?}");
    }

    #[test]
    fn the_removed_any_shape_gives_a_migration() {
        let (errors, _) = check(&entry(&[("tier_shape", Value::String("any".into()))]));
        let joined = errors.join(" ");
        assert!(
            joined.contains("low_and_above") && joined.contains("unstated"),
            "{joined}"
        );
    }

    #[test]
    fn an_informational_floor_is_rejected_with_the_fix() {
        let (errors, _) = check(&entry(&[
            ("tier_shape", Value::String("low_and_above".into())),
            ("floor_severity", Value::String("informational".into())),
        ]));
        let joined = errors.join(" ");
        assert!(
            joined.contains("informational") && joined.contains("low"),
            "{joined}"
        );
    }

    #[test]
    fn a_placeholder_host_is_an_error_outside_example_mode() {
        let (errors, _) = check(&entry(&[(
            "source_url",
            Value::String("https://example.invalid/rewards".into()),
        )]));
        assert!(errors.join(" ").contains("placeholder"), "{errors:?}");
    }

    #[test]
    fn a_real_url_containing_a_placeholder_word_is_fine() {
        for url in [
            "https://github.com/acme/todo-list-protocol/blob/main/BOUNTY.md",
            "https://docs.real-program.org/todo/rewards",
            "https://myexample.com/rewards",
            "https://example.com.evil.example/rewards",
        ] {
            let (errors, _) = check(&entry(&[("source_url", Value::String(url.into()))]));
            assert!(
                !errors.join(" ").contains("placeholder"),
                "{url}: {errors:?}"
            );
        }
    }

    #[test]
    fn placeholder_hosts_are_caught_by_host_not_substring() {
        for url in [
            "https://example.invalid/rewards",
            "http://localhost:8000/rewards",
            "https://todo/rewards",
            "https://rewards.example.com/bounty",
            "http://[::1]/rewards",
            "http://app.localhost/rewards",
        ] {
            assert!(placeholder_host(url).is_some(), "{url}");
        }
        assert!(placeholder_host("https://docs.real-program.org/rewards").is_none());
    }

    #[test]
    fn a_future_date_is_an_error_and_old_dates_warn() {
        let (errors, _) = check(&entry(&[("retrieved", Value::String("2027-01-15".into()))]));
        assert!(errors.join(" ").contains("future"), "{errors:?}");
        let (_, warnings) = check(&entry(&[("retrieved", Value::String("2025-01-01".into()))]));
        assert!(warnings.join(" ").contains("days ago"), "{warnings:?}");
    }

    #[test]
    fn pattern_dialect_matches_the_python_validator() {
        for bad in [
            "(?<!un)privileged",
            "(?=admin)owner",
            "(owner)\\1",
            "(?P<r>owner)(?P=r)",
            "a++",
            "a{1,2}+",
            "(?#comment)admin",
            "(a)?(?(1)admin|owner)",
            "foo\\Z",
            "(?a)admin",
            "(?<n>a)\\k<n>",
        ] {
            assert!(regex_problem(bad).is_some(), "{bad}");
        }
        for good in [
            "[*+]",
            "[a*+?]",
            "a}+",
            "\\*+",
            "(?P<name>owner|admin)",
            "(?i)admin",
        ] {
            assert!(
                regex_problem(good).is_none(),
                "{good}: {:?}",
                regex_problem(good)
            );
        }
    }

    #[test]
    fn unknown_keys_and_bad_types_are_schema_errors() {
        let (errors, _) = check(&entry(&[("exclusionz", Value::Array(vec![]))]));
        assert!(errors.join(" ").contains("exclusionz"), "{errors:?}");
        let (errors, _) = check(&entry(&[("conflicts", Value::String("one string".into()))]));
        assert!(errors.join(" ").contains("conflicts"), "{errors:?}");
    }

    #[test]
    fn a_second_yaml_document_is_not_silently_dropped() {
        assert_eq!(split_documents("a: 1\n---\nb: 2\n").len(), 2);
        assert_eq!(split_documents("---\na: 1\n").len(), 1);
    }

    #[test]
    fn dates_round_trip() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(
            parse_iso_date("2026-02-29"),
            None,
            "2026 is not a leap year"
        );
        assert_eq!(
            parse_iso_date("2024-02-29"),
            Some(days_from_civil(2024, 2, 29))
        );
    }
}
