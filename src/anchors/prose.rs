//! Prose anchors: a second extraction pass that runs only before a text would
//! abstain on "no anchor".
//!
//! [`crate::anchors::citations`] (PA-B1) already reads backticked identifiers,
//! `Contract.function` pairs, `function name`, `.sol` paths and GitHub URLs.
//! When none of those resolves, this pass reads the shapes PA-B1 leaves out:
//!
//! 1. a bare call-shaped identifier outside backticks, `rageQuit()` or
//!    `_checkBalances(int256 x, int256 y)`;
//! 2. an identifier written before the word `function` or `modifier`
//!    (`the batchMigrateFollows function`), when the identifier looks like code
//!    (it contains `_` or a lower-to-upper case step);
//! 3. a line reference (`line 117`, `lines 12-34`, `L117`, `L12-L34`) bound to
//!    the one `.sol` file the same text cites. With no cited file, or with
//!    more than one, a line reference is dropped: it never guesses a file.
//!
//! Every new citation goes through the same PA-B1 resolver, so a prose anchor
//! is always the span of a parsed declaration (or a line range of a parsed
//! file) and an unresolvable or ambiguous name still abstains. The pass is
//! text-only, deterministic and offline.

use std::collections::BTreeSet;
use std::sync::OnceLock;

use super::cite::{citations, is_builtin_name, is_ident_name, Citation, CitationKind};
use super::index::IndexSet;
use super::resolve::resolve_citations;
use super::AnchorSet;

/// The shapes this pass adds, as citations. Sorted and deduplicated; the
/// PA-B1 citations of the same text are not repeated.
pub fn prose_citations(text: &str) -> Vec<Citation> {
    let base = citations(text);
    let mut found: Vec<Citation> = Vec::new();

    if let Some(re) = call_re() {
        for caps in re.captures_iter(text) {
            let name = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            let args = caps.get(2).map(|m| m.as_str()).unwrap_or("");
            if !callable_name(name) || plural_marker(args) {
                continue;
            }
            found.push(identifier(
                caps.get(0).map(|m| m.as_str()).unwrap_or(name),
                name,
            ));
        }
    }

    if let Some(re) = suffix_word_re() {
        for caps in re.captures_iter(text) {
            let name = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            if !callable_name(name) || !looks_like_code(name) {
                continue;
            }
            found.push(Citation {
                raw: caps.get(0).map(|m| m.as_str()).unwrap_or(name).to_string(),
                kind: CitationKind::FunctionName,
                file: None,
                line_start: None,
                line_end: None,
                symbol: Some(name.to_string()),
                qualifier: None,
                repo_hint: None,
            });
        }
    }

    // Line references bind only to the single file the text cites.
    let files: BTreeSet<(String, Option<String>)> = base
        .iter()
        .filter_map(|c| c.file.clone().map(|f| (f, c.repo_hint.clone())))
        .collect();
    let distinct_files: BTreeSet<&String> = files.iter().map(|(f, _)| f).collect();
    if distinct_files.len() == 1 {
        let (file, repo_hint) = files.iter().next().expect("one file").clone();
        for (raw, start, end) in line_references(text) {
            found.push(Citation {
                raw,
                kind: CitationKind::FilePath,
                file: Some(file.clone()),
                line_start: Some(start),
                line_end: Some(end),
                symbol: None,
                qualifier: None,
                repo_hint: repo_hint.clone(),
            });
        }
    }

    let known: BTreeSet<CitedKey> = base.iter().map(cited_key).collect();
    found.retain(|c| !known.contains(&cited_key(c)));
    found.sort();
    found.dedup_by(|a, b| {
        a.kind == b.kind
            && a.symbol == b.symbol
            && a.file == b.file
            && a.line_start == b.line_start
            && a.line_end == b.line_end
    });
    found
}

/// Resolve `text` with PA-B1, and only when that yields no anchor, again with
/// the prose citations added. A text that already has an anchor is returned
/// exactly as [`crate::anchors::resolve_text`] returns it.
pub fn resolve_text_with_prose(text: &str, index: &IndexSet) -> AnchorSet {
    let base = super::resolve_text(text, index);
    if !base.anchors.is_empty() || index.repos.is_empty() {
        return base;
    }
    let extra = prose_citations(text);
    if extra.is_empty() {
        return base;
    }
    let mut all = citations(text);
    all.extend(extra);
    all.sort();
    all.dedup();
    resolve_citations(all, index)
}

// ------------------------------------------------------------------ helpers

/// What a citation names: symbol, file and line range.
type CitedKey = (Option<String>, Option<String>, Option<u32>, Option<u32>);

fn cited_key(c: &Citation) -> CitedKey {
    (c.symbol.clone(), c.file.clone(), c.line_start, c.line_end)
}

fn identifier(raw: &str, name: &str) -> Citation {
    Citation {
        raw: raw.to_string(),
        kind: CitationKind::Identifier,
        file: None,
        line_start: None,
        line_end: None,
        symbol: Some(name.to_string()),
        qualifier: None,
        repo_hint: None,
    }
}

/// A name that can be a declared function: an identifier of 3+ characters
/// that is not a Solidity keyword or builtin.
fn callable_name(name: &str) -> bool {
    name.len() >= 3 && is_ident_name(name) && !is_builtin_name(name)
}

/// `token(s)`, `user(es)`: an English plural marker, not a call.
fn plural_marker(args: &str) -> bool {
    matches!(args.trim(), "s" | "es" | "S" | "ES")
}

/// An identifier that reads as code rather than an English word: it contains
/// an underscore, or a lowercase letter followed by an uppercase one.
fn looks_like_code(name: &str) -> bool {
    if name.contains('_') {
        return true;
    }
    let bytes = name.as_bytes();
    bytes
        .windows(2)
        .any(|w| w[0].is_ascii_lowercase() && w[1].is_ascii_uppercase())
}

/// Every `line N`, `lines N-M`, `lines N to M`, `LN` and `LN-LM` reference,
/// with a 1-based, ordered range. `L` forms need two or more digits, so QA
/// labels such as `L4` or `[L-01]` are not read as lines.
fn line_references(text: &str) -> Vec<(String, u32, u32)> {
    let mut out = Vec::new();
    if let Some(re) = line_word_re() {
        for caps in re.captures_iter(text) {
            if let Some(range) = range_of(&caps) {
                out.push((
                    caps.get(0).expect("group 0").as_str().to_string(),
                    range.0,
                    range.1,
                ));
            }
        }
    }
    if let Some(re) = line_l_re() {
        for caps in re.captures_iter(text) {
            if let Some(range) = range_of(&caps) {
                out.push((
                    caps.get(0).expect("group 0").as_str().to_string(),
                    range.0,
                    range.1,
                ));
            }
        }
    }
    out
}

fn range_of(caps: &regex::Captures<'_>) -> Option<(u32, u32)> {
    let start = caps
        .get(1)?
        .as_str()
        .parse::<u32>()
        .ok()
        .filter(|n| *n > 0)?;
    let end = caps
        .get(2)
        .and_then(|m| m.as_str().parse::<u32>().ok())
        .filter(|n| *n >= start)
        .unwrap_or(start);
    Some((start, end))
}

// ------------------------------------------------------------------ regexes

fn call_re() -> Option<&'static regex::Regex> {
    static RE: OnceLock<Option<regex::Regex>> = OnceLock::new();
    RE.get_or_init(|| {
        // Not preceded by an identifier character, `.`, `$` or a backtick (a
        // member call or a backticked token is PA-B1's); arguments stay on one
        // line and are at most 200 bytes.
        regex::Regex::new(r"(?:^|[^A-Za-z0-9_.`$])([A-Za-z_][A-Za-z0-9_]*)\(([^()\n`]{0,200})\)")
            .ok()
    })
    .as_ref()
}

fn suffix_word_re() -> Option<&'static regex::Regex> {
    static RE: OnceLock<Option<regex::Regex>> = OnceLock::new();
    RE.get_or_init(|| {
        regex::Regex::new(
            r"(?:^|[^A-Za-z0-9_.`$])([A-Za-z_][A-Za-z0-9_]*)\s+(?:function|modifier)\b",
        )
        .ok()
    })
    .as_ref()
}

fn line_word_re() -> Option<&'static regex::Regex> {
    static RE: OnceLock<Option<regex::Regex>> = OnceLock::new();
    RE.get_or_init(|| {
        regex::Regex::new(r"(?i)\blines?\s+#?(\d{1,6})(?:\s*(?:-|–|to)\s*#?(\d{1,6}))?\b").ok()
    })
    .as_ref()
}

fn line_l_re() -> Option<&'static regex::Regex> {
    static RE: OnceLock<Option<regex::Regex>> = OnceLock::new();
    RE.get_or_init(|| {
        regex::Regex::new(r"(?:^|[^A-Za-z0-9_#/-])L(\d{2,6})(?:\s*-\s*L?(\d{2,6}))?\b").ok()
    })
    .as_ref()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anchors::index::{parse_repo, IndexOptions};
    use crate::anchors::{resolve_text, AnchorKind};
    use std::path::PathBuf;

    fn symbols(text: &str) -> Vec<String> {
        prose_citations(text)
            .into_iter()
            .filter_map(|c| c.symbol)
            .collect()
    }

    fn fixture() -> IndexSet {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/anchors-repo");
        let index = parse_repo(
            &root,
            "example/anchor-fixture",
            "0000000000000000000000000000000000000000",
            &IndexOptions::default(),
        )
        .expect("fixture repo parses");
        IndexSet::single(index)
    }

    // --- pattern 1: bare call shape -------------------------------------

    #[test]
    fn a_bare_call_is_a_prose_citation() {
        assert_eq!(
            symbols("Users lose funds after calling rageQuit() twice."),
            vec!["rageQuit"]
        );
        assert_eq!(
            symbols("Missing check for _checkBalances(int256 x, int256 y) here."),
            vec!["_checkBalances"]
        );
    }

    #[test]
    fn plurals_members_builtins_and_backticks_are_not_bare_calls() {
        assert!(symbols("The token(s) and user(es) are listed.").is_empty());
        assert!(symbols("It calls token.transfer(to, amount) first.").is_empty());
        assert!(symbols("It uses require(ok) and keccak256(data).").is_empty());
        // A backticked call is PA-B1's citation; the prose pass does not repeat it.
        assert!(symbols("`deposit()` is wrong").is_empty());
        assert!(symbols("An aside (see above) in prose.").is_empty());
    }

    // --- pattern 2: `name function` -------------------------------------

    #[test]
    fn a_code_like_name_before_function_is_a_prose_citation() {
        assert_eq!(
            symbols("then using the batchMigrateFollows function to refollow"),
            vec!["batchMigrateFollows"]
        );
        assert_eq!(
            symbols("happens in the _removeTokenFromOwnerEnumeration function."),
            vec!["_removeTokenFromOwnerEnumeration"]
        );
        assert_eq!(
            symbols("the onlyOwner modifier is missing"),
            vec!["onlyOwner"]
        );
    }

    #[test]
    fn an_english_word_before_function_is_not_a_prose_citation() {
        assert!(symbols("This function does not check the caller.").is_empty());
        assert!(symbols("The deposit function is fine and the view function too.").is_empty());
    }

    // --- pattern 3: line references -------------------------------------

    #[test]
    fn a_line_reference_binds_to_the_single_cited_file() {
        let c =
            prose_citations("In contracts/Vault.sol the bug is at line 18; see also lines 12-16.");
        let lines: Vec<_> = c
            .iter()
            .filter(|c| c.kind == CitationKind::FilePath)
            .map(|c| (c.file.clone().unwrap(), c.line_start, c.line_end))
            .collect();
        assert!(
            lines.contains(&("contracts/Vault.sol".to_string(), Some(18), Some(18))),
            "{c:?}"
        );
        assert!(
            lines.contains(&("contracts/Vault.sol".to_string(), Some(12), Some(16))),
            "{c:?}"
        );
        let c = prose_citations("Vault.sol L18-L19 increments the total.");
        assert!(
            c.iter()
                .any(|c| c.line_start == Some(18) && c.line_end == Some(19)),
            "{c:?}"
        );
    }

    #[test]
    fn a_line_reference_without_one_cited_file_is_dropped() {
        // No file: nothing to bind to.
        assert!(prose_citations("The index is incremented in line 117.").is_empty());
        // Two files: ambiguous, never guessed.
        let two = prose_citations("See A.sol and B.sol, line 40.");
        assert!(two.iter().all(|c| c.line_start.is_none()), "{two:?}");
        // QA labels are not lines.
        assert!(prose_citations("Vault.sol [L-01] and L4 and L-11.")
            .iter()
            .all(|c| c.line_start.is_none()));
    }

    // --- resolution -------------------------------------------------------

    #[test]
    fn a_prose_only_text_resolves_through_the_pa_b1_resolver() {
        let text = "Calling compound() after a donation inflates the total.";
        let base = resolve_text(text, &fixture());
        assert!(base.anchors.is_empty(), "{base:#?}");
        let set = resolve_text_with_prose(text, &fixture());
        assert_eq!(set.anchors.len(), 1, "{set:#?}");
        assert_eq!(set.anchors[0].kind, AnchorKind::Function);
        assert_eq!(set.anchors[0].symbol, "Vault.compound");
    }

    #[test]
    fn a_text_that_already_resolves_is_unchanged() {
        let text = "`Vault.deposit` is wrong, and so is compound() and the _helper function.";
        assert_eq!(
            resolve_text_with_prose(text, &fixture()),
            resolve_text(text, &fixture())
        );
    }

    #[test]
    fn an_unknown_prose_name_still_abstains() {
        let text = "Calling notDeclaredAnywhere() breaks it.";
        let set = resolve_text_with_prose(text, &fixture());
        assert!(set.anchors.is_empty(), "{set:#?}");
        assert!(!set.abstentions.is_empty());
    }

    #[test]
    fn prose_extraction_is_deterministic() {
        let text = "rageQuit() then the batchMigrateFollows function at Vault.sol line 12";
        assert_eq!(prose_citations(text), prose_citations(text));
    }
}
