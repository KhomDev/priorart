//! Shared helpers for the integration tests.
//!
//! Deliberately dependency-free: a temp-directory helper, a binary runner and a
//! small JSON-Schema subset validator. The validator covers exactly the
//! keywords the three shipped schemas use, so "the schema is enforced" is a
//! test, not a promise.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// A fresh directory under the system temp dir, removed on drop.
pub struct TempDir {
    path: PathBuf,
}

impl TempDir {
    pub fn new(label: &str) -> Self {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let mut path = std::env::temp_dir();
        path.push(format!(
            "priorart-test-{}-{}-{n}",
            std::process::id(),
            label
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("create temp dir");
        Self { path }
    }

    pub fn join(&self, rel: &str) -> PathBuf {
        self.path.join(rel)
    }

    pub fn write(&self, rel: &str, body: &str) -> PathBuf {
        let path = self.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create parent");
        }
        std::fs::write(&path, body).expect("write fixture");
        path
    }

    pub fn write_bytes(&self, rel: &str, body: &[u8]) -> PathBuf {
        let path = self.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create parent");
        }
        std::fs::write(&path, body).expect("write fixture");
        path
    }

    pub fn copy_dir(&self, from: &Path, to_rel: &str) {
        let to = self.join(to_rel);
        std::fs::create_dir_all(&to).expect("create copy target");
        copy_recursive(from, &to);
    }
}

fn copy_recursive(from: &Path, to: &Path) {
    for entry in std::fs::read_dir(from).expect("read dir").flatten() {
        let src = entry.path();
        let dst = to.join(entry.file_name());
        if src.is_dir() {
            std::fs::create_dir_all(&dst).expect("mkdir");
            copy_recursive(&src, &dst);
        } else {
            std::fs::copy(&src, &dst).expect("copy file");
        }
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// The crate directory, so tests can point at the committed fixtures.
pub fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Run the binary with arguments, from the crate directory.
pub struct Run {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

pub fn run(args: &[&str]) -> Run {
    run_in(&manifest_dir(), args)
}

pub fn run_in(dir: &Path, args: &[&str]) -> Run {
    let output = Command::new(env!("CARGO_BIN_EXE_priorart"))
        .args(args)
        .current_dir(dir)
        .output()
        .expect("run priorart");
    Run {
        code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    }
}

/// Read a shipped schema.
pub fn schema(name: &str) -> serde_json::Value {
    let path = manifest_dir().join("schemas").join(name);
    let text = std::fs::read_to_string(&path).expect("read schema");
    serde_json::from_str(&text).expect("schema is JSON")
}

/// Minimal JSON-Schema (draft 2020-12 subset) validation.
///
/// Supported: type (string or array), required, properties,
/// additionalProperties:false, items, enum, const, pattern, minLength,
/// minimum, minItems. $schema, $id, title, description and format are ignored
/// (format is documentation for a reader, not a check this test makes).
pub fn validate(schema: &serde_json::Value, instance: &serde_json::Value) -> Vec<String> {
    let mut errors = Vec::new();
    check(schema, instance, "$", &mut errors);
    errors
}

fn type_matches(expected: &str, value: &serde_json::Value) -> bool {
    match expected {
        "object" => value.is_object(),
        "array" => value.is_array(),
        "string" => value.is_string(),
        "integer" => value.as_i64().is_some() || value.as_u64().is_some(),
        "number" => value.is_number(),
        "boolean" => value.is_boolean(),
        "null" => value.is_null(),
        _ => true,
    }
}

fn check(
    schema: &serde_json::Value,
    instance: &serde_json::Value,
    path: &str,
    errors: &mut Vec<String>,
) {
    if let Some(kind) = schema.get("type") {
        let ok = match kind {
            serde_json::Value::String(s) => type_matches(s, instance),
            serde_json::Value::Array(types) => types
                .iter()
                .filter_map(|t| t.as_str())
                .any(|t| type_matches(t, instance)),
            _ => true,
        };
        if !ok {
            errors.push(format!("{path}: expected type {kind}, got {instance}"));
            return;
        }
    }
    if let Some(constant) = schema.get("const") {
        if instance != constant {
            errors.push(format!("{path}: expected const {constant}, got {instance}"));
        }
    }
    if let Some(allowed) = schema.get("enum").and_then(|e| e.as_array()) {
        if !allowed.contains(instance) {
            errors.push(format!("{path}: {instance} is not in the enum"));
        }
    }
    if let Some(text) = instance.as_str() {
        if let Some(min) = schema.get("minLength").and_then(|m| m.as_u64()) {
            if (text.chars().count() as u64) < min {
                errors.push(format!("{path}: shorter than minLength {min}"));
            }
        }
        if let Some(pattern) = schema.get("pattern").and_then(|p| p.as_str()) {
            let re = regex::Regex::new(pattern).expect("schema pattern compiles");
            if !re.is_match(text) {
                errors.push(format!("{path}: {text:?} does not match {pattern}"));
            }
        }
    }
    if let Some(min) = schema.get("minimum").and_then(|m| m.as_i64()) {
        if let Some(n) = instance.as_i64() {
            if n < min {
                errors.push(format!("{path}: {n} is below minimum {min}"));
            }
        }
    }
    if let (Some(items), Some(array)) = (schema.get("items"), instance.as_array()) {
        if let Some(min) = schema.get("minItems").and_then(|m| m.as_u64()) {
            if (array.len() as u64) < min {
                errors.push(format!("{path}: fewer than minItems {min}"));
            }
        }
        for (i, item) in array.iter().enumerate() {
            check(items, item, &format!("{path}[{i}]"), errors);
        }
    }
    if let (Some(props), Some(object)) = (schema.get("properties"), instance.as_object()) {
        if let Some(required) = schema.get("required").and_then(|r| r.as_array()) {
            for key in required.iter().filter_map(|k| k.as_str()) {
                if !object.contains_key(key) {
                    errors.push(format!("{path}: missing required {key}"));
                }
            }
        }
        for (key, value) in object {
            if let Some(sub) = props.get(key) {
                check(sub, value, &format!("{path}.{key}"), errors);
            } else if schema.get("additionalProperties") == Some(&serde_json::Value::Bool(false)) {
                errors.push(format!("{path}: unexpected property {key}"));
            }
        }
    }
}
