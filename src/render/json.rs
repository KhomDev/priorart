//! JSON output. One pretty-printer, one trailing newline, so a file written
//! by the tool is byte-stable and diffable.

use serde::Serialize;

pub fn pretty<T: Serialize>(value: &T) -> Result<String, String> {
    let mut body = serde_json::to_string_pretty(value)
        .map_err(|e| format!("cannot serialise JSON output: {e}"))?;
    body.push('\n');
    Ok(body)
}
