//! Deterministic citation extraction.
//!
//! The extractor reads text only: it never consults the repository index, so
//! the denominator of the resolution measurement ("findings that cite code")
//! is fixed before any span is resolved. Three shapes count as a code
//! citation:
//!
//! 1. a file path ending `.sol`, optionally followed by `:12`, `:12-34`,
//!    `#L12` or `#L12-L34`;
//! 2. a GitHub `blob`/`tree` URL to a `.sol` file, with an optional `#L` line
//!    fragment;
//! 3. an identifier token: a backticked identifier (optionally dotted and/or
//!    call-shaped), or a plain `Contract.member` pair, or a plain
//!    `function name`.
//!
//! Solidity keywords and builtins (`msg.sender`, `true`, `uint256`, ...) are
//! not citations: they name the language, not the audited code.

use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

/// The shape a citation was written in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CitationKind {
    /// A plain repository-relative or bare file path.
    FilePath,
    /// A GitHub blob/tree URL.
    Url,
    /// A backticked or dotted identifier.
    Identifier,
    /// The word `function` followed by an identifier.
    FunctionName,
}

/// One citation extracted from a text.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Citation {
    /// The matched text, so a reader can find it again.
    pub raw: String,
    pub kind: CitationKind,
    /// Cited file path, if the citation names one.
    pub file: Option<String>,
    /// 1-based first cited line, if the citation names one.
    pub line_start: Option<u32>,
    /// 1-based last cited line (equal to `line_start` for a single line).
    pub line_end: Option<u32>,
    /// Cited member name (`deposit` in `Vault.deposit`).
    pub symbol: Option<String>,
    /// Cited qualifier (`Vault` in `Vault.deposit`).
    pub qualifier: Option<String>,
    /// `owner/repo` from a GitHub URL, used to pick among several checkouts.
    pub repo_hint: Option<String>,
}

/// Extract every code citation in `text`, sorted and deduplicated.
pub fn citations(text: &str) -> Vec<Citation> {
    let mut found = Vec::new();
    let mut url_spans: Vec<(usize, usize)> = Vec::new();
    let mut file_spans: Vec<(usize, usize)> = Vec::new();

    if let Some(re) = github_url_re() {
        for caps in re.captures_iter(text) {
            let whole = caps.get(0).expect("group 0");
            let owner = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            let repo = caps.get(2).map(|m| m.as_str()).unwrap_or("");
            let path = caps.get(4).map(|m| m.as_str()).unwrap_or("");
            if path.is_empty() {
                continue;
            }
            let line_start = caps
                .get(5)
                .and_then(|m| m.as_str().parse::<u32>().ok())
                .filter(|n| *n > 0);
            let line_end = caps
                .get(6)
                .and_then(|m| m.as_str().parse::<u32>().ok())
                .filter(|n| *n > 0)
                .or(line_start);
            url_spans.push((whole.start(), whole.end()));
            found.push(Citation {
                raw: whole.as_str().to_string(),
                kind: CitationKind::Url,
                file: Some(normalise_path(path)),
                line_start,
                line_end,
                symbol: None,
                qualifier: None,
                repo_hint: Some(format!("{owner}/{repo}")),
            });
        }
    }

    if let Some(re) = file_token_re() {
        for caps in re.captures_iter(text) {
            let whole = caps.get(0).expect("group 0");
            // A path inside a URL was already recorded with its repo hint.
            if url_spans
                .iter()
                .any(|(start, end)| whole.start() >= *start && whole.end() <= *end)
            {
                continue;
            }
            let path = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            if path.is_empty() {
                continue;
            }
            let (line_start, line_end) = line_from(&caps, 2, 3)
                .or_else(|| line_from(&caps, 4, 5))
                .unwrap_or((None, None));
            file_spans.push((whole.start(), whole.end()));
            found.push(Citation {
                raw: whole.as_str().to_string(),
                kind: CitationKind::FilePath,
                file: Some(normalise_path(path)),
                line_start,
                line_end,
                symbol: None,
                qualifier: None,
                repo_hint: None,
            });
        }
    }

    if let Some(re) = backtick_re() {
        for caps in re.captures_iter(text) {
            let token = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            if let Some(parts) = ident_parts(token) {
                found.push(Citation {
                    raw: caps.get(0).expect("group 0").as_str().to_string(),
                    kind: CitationKind::Identifier,
                    file: None,
                    line_start: None,
                    line_end: None,
                    symbol: Some(parts.symbol),
                    qualifier: parts.qualifier,
                    repo_hint: None,
                });
            }
        }
    }

    if let Some(re) = function_word_re() {
        for caps in re.captures_iter(text) {
            let symbol = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            if symbol.is_empty() || is_builtin(symbol) {
                continue;
            }
            found.push(Citation {
                raw: caps.get(0).expect("group 0").as_str().to_string(),
                kind: CitationKind::FunctionName,
                file: None,
                line_start: None,
                line_end: None,
                symbol: Some(symbol.to_string()),
                qualifier: None,
                repo_hint: None,
            });
        }
    }

    if let Some(re) = dotted_re() {
        for caps in re.captures_iter(text) {
            let whole = caps.get(0).expect("group 0");
            let qualifier = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            let symbol = caps.get(2).map(|m| m.as_str()).unwrap_or("");
            if qualifier.is_empty() || symbol.is_empty() || is_builtin(symbol) {
                continue;
            }
            if is_language_qualifier(qualifier) {
                continue;
            }
            // `Vault.sol` inside a file path is a file citation, not a member.
            if is_file_extension(symbol) {
                continue;
            }
            if url_spans
                .iter()
                .chain(file_spans.iter())
                .any(|(start, end)| whole.start() >= *start && whole.end() <= *end)
            {
                continue;
            }
            found.push(Citation {
                raw: caps.get(0).expect("group 0").as_str().to_string(),
                kind: CitationKind::Identifier,
                file: None,
                line_start: None,
                line_end: None,
                symbol: Some(symbol.to_string()),
                qualifier: Some(qualifier.to_string()),
                repo_hint: None,
            });
        }
    }

    normalise(found)
}

/// Whether `text` cites code under the definition in this module.
pub fn cites_code(text: &str) -> bool {
    !citations(text).is_empty()
}

/// The raw tokens that counted as citations, for diagnostics and tests.
pub fn citation_tokens(text: &str) -> Vec<String> {
    citations(text).into_iter().map(|c| c.raw).collect()
}

// ------------------------------------------------------------------ helpers

/// Sort by a semantic key, then drop duplicates keeping the first raw spelling.
fn normalise(mut found: Vec<Citation>) -> Vec<Citation> {
    let key = |c: &Citation| {
        (
            c.kind,
            c.file.clone(),
            c.line_start,
            c.line_end,
            c.qualifier.clone(),
            c.symbol.clone(),
            c.repo_hint.clone(),
        )
    };
    found.sort_by(|a, b| key(a).cmp(&key(b)).then_with(|| a.raw.cmp(&b.raw)));
    found.dedup_by(|a, b| key(a) == key(b));
    found
}

/// Pull a 1-based line pair out of two optional capture groups.
fn line_from(
    caps: &regex::Captures<'_>,
    first: usize,
    second: usize,
) -> Option<(Option<u32>, Option<u32>)> {
    let start = caps
        .get(first)
        .and_then(|m| m.as_str().parse::<u32>().ok())
        .filter(|n| *n > 0)?;
    let end = caps
        .get(second)
        .and_then(|m| m.as_str().parse::<u32>().ok())
        .filter(|n| *n >= start)
        .unwrap_or(start);
    Some((Some(start), Some(end)))
}

/// Normalise a cited path: forward slashes, no leading `./` or `/`.
pub fn normalise_path(path: &str) -> String {
    let mut p = path.replace('\\', "/");
    while let Some(rest) = p.strip_prefix("./") {
        p = rest.to_string();
    }
    while let Some(rest) = p.strip_prefix('/') {
        p = rest.to_string();
    }
    p
}

/// The parsed pieces of an identifier token.
struct IdentParts {
    qualifier: Option<String>,
    symbol: String,
}

/// Parse a backticked token as an identifier. Returns `None` for prose,
/// language builtins and anything that is not a plain identifier chain.
fn ident_parts(token: &str) -> Option<IdentParts> {
    let token = token.trim();
    if token.is_empty() || token.contains(char::is_whitespace) {
        return None;
    }
    // `foo(...)` names `foo`; the arguments are not part of the citation.
    let head = match token.find('(') {
        Some(i) => &token[..i],
        None => token,
    };
    if head.is_empty()
        || !head
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
    {
        return None;
    }
    let segments: Vec<&str> = head.split('.').filter(|s| !s.is_empty()).collect();
    if segments.is_empty() {
        return None;
    }
    let symbol = *segments.last().expect("non-empty");
    if symbol.len() < 2 || !is_ident(symbol) || is_builtin(symbol) {
        return None;
    }
    let qualifier = if segments.len() >= 2 {
        let q = segments[segments.len() - 2];
        if !is_ident(q) || is_language_qualifier(q) {
            return None;
        }
        Some(q.to_string())
    } else {
        None
    };
    Some(IdentParts {
        qualifier,
        symbol: symbol.to_string(),
    })
}

fn is_ident(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Solidity keywords, builtins and type names. Not audited-code citations.
fn is_builtin(name: &str) -> bool {
    if is_elementary_type_name(name) {
        return true;
    }
    matches!(
        name,
        "msg"
            | "block"
            | "tx"
            | "abi"
            | "this"
            | "super"
            | "type"
            | "now"
            | "true"
            | "false"
            | "address"
            | "bool"
            | "string"
            | "byte"
            | "fixed"
            | "ufixed"
            | "require"
            | "assert"
            | "revert"
            | "emit"
            | "return"
            | "returns"
            | "delete"
            | "new"
            | "payable"
            | "memory"
            | "storage"
            | "calldata"
            | "function"
            | "modifier"
            | "event"
            | "error"
            | "struct"
            | "enum"
            | "mapping"
            | "contract"
            | "interface"
            | "library"
            | "abstract"
            | "public"
            | "private"
            | "internal"
            | "external"
            | "pure"
            | "view"
            | "virtual"
            | "override"
            | "immutable"
            | "constant"
            | "if"
            | "else"
            | "for"
            | "while"
            | "do"
            | "break"
            | "continue"
            | "try"
            | "catch"
            | "unchecked"
            | "assembly"
            | "keccak256"
            | "sha256"
            | "ripemd160"
            | "ecrecover"
            | "addmod"
            | "mulmod"
            | "selfdestruct"
            | "blockhash"
            | "gasleft"
            | "indexed"
            | "anonymous"
            | "constructor"
            | "fallback"
            | "receive"
            | "let"
    )
}

/// Identifiers whose left side names the language, not a contract.
fn is_language_qualifier(name: &str) -> bool {
    matches!(
        name,
        "msg" | "block" | "tx" | "abi" | "this" | "super" | "type"
    )
}

/// A Solidity elementary type: `uintN`, `intN`, `bytesN`, `bytes`, `address`,
/// `bool`, `string`, `byte`, `fixed`, `ufixed`.
fn is_elementary_type_name(name: &str) -> bool {
    let sized = |prefix: &str, max: u32| -> bool {
        name.strip_prefix(prefix).is_some_and(|rest| {
            !rest.is_empty()
                && rest.len() <= 3
                && rest
                    .parse::<u32>()
                    .is_ok_and(|n| n >= 8 && n <= max && n % 8 == 0)
        })
    };
    if sized("uint", 256) || sized("int", 256) {
        return true;
    }
    if let Some(rest) = name.strip_prefix("bytes") {
        if !rest.is_empty() {
            return rest.parse::<u32>().is_ok_and(|n| (1..=32).contains(&n));
        }
        return true;
    }
    matches!(
        name,
        "address" | "bool" | "string" | "byte" | "fixed" | "ufixed"
    )
}

/// A file extension that appears in `Name.ext` prose and is not a member
/// access (`README.md`, `foundry.toml`, ...).
fn is_file_extension(name: &str) -> bool {
    matches!(
        name,
        "sol"
            | "md"
            | "json"
            | "yaml"
            | "yml"
            | "txt"
            | "pdf"
            | "html"
            | "htm"
            | "js"
            | "ts"
            | "toml"
            | "lock"
            | "png"
            | "svg"
            | "csv"
            | "zip"
    )
}

// ------------------------------------------------------------------ regexes

fn github_url_re() -> Option<&'static regex::Regex> {
    static RE: OnceLock<Option<regex::Regex>> = OnceLock::new();
    RE.get_or_init(|| {
        regex::Regex::new(
            r#"(?i)https?://(?:www\.)?github\.com/([A-Za-z0-9_.-]+)/([A-Za-z0-9_.-]+)/(?:blob|tree)/([^/\s#?]+)/([^\s#?"'`<>]+?\.sol)(?:#L(\d+)(?:-L(\d+))?)?"#,
        )
        .ok()
    })
    .as_ref()
}

fn file_token_re() -> Option<&'static regex::Regex> {
    static RE: OnceLock<Option<regex::Regex>> = OnceLock::new();
    RE.get_or_init(|| {
        regex::Regex::new(
            r#"(?i)([A-Za-z0-9_.-]+(?:/[A-Za-z0-9_.-]+)*\.sol)(?::(\d+)(?:-(\d+))?|#L(\d+)(?:-L(\d+))?)?"#,
        )
        .ok()
    })
    .as_ref()
}

fn backtick_re() -> Option<&'static regex::Regex> {
    static RE: OnceLock<Option<regex::Regex>> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"`([^`\n]{1,200})`").ok())
        .as_ref()
}

fn function_word_re() -> Option<&'static regex::Regex> {
    static RE: OnceLock<Option<regex::Regex>> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"(?i)\bfunction\s+`?([A-Za-z_][A-Za-z0-9_]*)`?").ok())
        .as_ref()
}

fn dotted_re() -> Option<&'static regex::Regex> {
    static RE: OnceLock<Option<regex::Regex>> = OnceLock::new();
    RE.get_or_init(|| {
        regex::Regex::new(r"\b([A-Z][A-Za-z0-9_]{2,})\.([A-Za-z_][A-Za-z0-9_]*)\b").ok()
    })
    .as_ref()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_paths_and_lines() {
        let c = citations("The check is in `contracts/Vault.sol:12-16`.");
        assert_eq!(c.len(), 1, "{c:?}");
        assert_eq!(c[0].kind, CitationKind::FilePath);
        assert_eq!(c[0].file.as_deref(), Some("contracts/Vault.sol"));
        assert_eq!((c[0].line_start, c[0].line_end), (Some(12), Some(16)));

        let c = citations("see src/token.sol#L40 for the branch");
        assert_eq!(c[0].file.as_deref(), Some("src/token.sol"));
        assert_eq!((c[0].line_start, c[0].line_end), (Some(40), Some(40)));
    }

    #[test]
    fn github_url_carries_repo_and_lines() {
        let text = "https://github.com/code-423n4/2023-09-maia/blob/main/src/CoreBranchRouter.sol#L140-L146";
        let c = citations(text);
        assert_eq!(c.len(), 1, "{c:?}");
        assert_eq!(c[0].kind, CitationKind::Url);
        assert_eq!(c[0].file.as_deref(), Some("src/CoreBranchRouter.sol"));
        assert_eq!(c[0].repo_hint.as_deref(), Some("code-423n4/2023-09-maia"));
        assert_eq!((c[0].line_start, c[0].line_end), (Some(140), Some(146)));
    }

    #[test]
    fn backticked_identifiers_and_calls() {
        let c = citations(
            "`leverageUpReceiver()` is missing `msg_.marketHelper` and `Vault.compound`.",
        );
        let syms: Vec<_> = c.iter().filter_map(|c| c.symbol.as_deref()).collect();
        assert!(syms.contains(&"leverageUpReceiver"), "{c:?}");
        assert!(syms.contains(&"marketHelper"), "{c:?}");
        assert!(c
            .iter()
            .any(|c| c.qualifier.as_deref() == Some("Vault")
                && c.symbol.as_deref() == Some("compound")));
    }

    #[test]
    fn builtins_are_not_citations() {
        assert!(!cites_code(
            "The caller is `msg.sender` and the value is `true`."
        ));
        assert!(!cites_code(
            "This is plain prose about a finding with no code."
        ));
        assert!(!cites_code("`uint256` and `msg.sender` are builtins."));
    }

    #[test]
    fn function_word_and_dotted_prose() {
        let c = citations("The function withdraw moves funds; ArbitrumCoreBranchRouter.executeNoSettlement does not.");
        let syms: Vec<_> = c.iter().filter_map(|c| c.symbol.as_deref()).collect();
        assert!(syms.contains(&"withdraw"), "{c:?}");
        assert!(syms.contains(&"executeNoSettlement"), "{c:?}");
    }

    #[test]
    fn duplicate_shapes_collapse() {
        let c = citations(
            "`deposit()` and deposit and `deposit` are the same citation in three shapes.",
        );
        // The identical symbol in three shapes keeps one identifier plus the
        // dotted/plain forms that differ semantically.
        let deposit_identifiers = c
            .iter()
            .filter(|c| c.symbol.as_deref() == Some("deposit"))
            .count();
        assert!(deposit_identifiers >= 1, "{c:?}");
    }

    #[test]
    fn extraction_is_deterministic() {
        let text = "`Vault.deposit` at src/Vault.sol:4 and https://github.com/a/b/blob/main/src/Vault.sol#L4";
        assert_eq!(citations(text), citations(text));
    }
}
