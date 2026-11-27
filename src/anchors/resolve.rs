//! Citation resolution.
//!
//! A file citation is resolved by exact path, then by suffix, then (for a bare
//! name) by basename. An identifier is resolved against the declarations in the
//! index: a qualified `Contract.member` through that contract and its bases, an
//! unqualified name through every declaration, preferring the files the same
//! text already resolved. Anything that cannot be decided abstains.
//!
//! The resolver never invents a span. A returned [`Anchor`] is always the span
//! of something the index actually parsed.

use crate::anchors::cite::{citations, Citation};
use crate::anchors::index::{IndexSet, RepoIndex, SymbolInfo};
use crate::anchors::{
    AbstainReason, Abstention, Anchor, AnchorKind, AnchorSet, Span, ANCHORS_SCHEMA,
};
use std::collections::BTreeSet;

/// Resolve every citation in `text` against `index`.
pub fn resolve_text(text: &str, index: &IndexSet) -> AnchorSet {
    resolve_citations(citations(text), index)
}

/// Resolve an already-extracted citation list (sorted and deduplicated by the
/// caller) against `index`. [`resolve_text`] is this over
/// [`citations`]`(text)`; the prose pass adds its own citations first.
pub(crate) fn resolve_citations(citations: Vec<Citation>, index: &IndexSet) -> AnchorSet {
    if citations.is_empty() {
        return AnchorSet {
            schema: ANCHORS_SCHEMA.to_string(),
            cites_code: false,
            citations,
            anchors: Vec::new(),
            abstentions: vec![Abstention {
                citation: None,
                reason: AbstainReason::NoCitation,
                detail: "the text contains no code citation".to_string(),
            }],
        };
    }
    if index.repos.is_empty() {
        let abstentions = citations
            .iter()
            .map(|c| Abstention {
                citation: Some(c.raw.clone()),
                reason: AbstainReason::RepoUnavailable,
                detail: "no audited repository was supplied".to_string(),
            })
            .collect();
        return AnchorSet {
            schema: ANCHORS_SCHEMA.to_string(),
            cites_code: true,
            citations,
            anchors: Vec::new(),
            abstentions,
        };
    }

    let mut anchors = Vec::new();
    let mut abstentions = Vec::new();
    let mut resolved_files: Vec<BTreeSet<String>> =
        index.repos.iter().map(|_| BTreeSet::new()).collect();
    let mut deferred: Vec<&Citation> = Vec::new();

    // Pass 1: citations that name a file. Their resolved paths become context
    // for the identifier pass, so an unqualified name in a finding that cites
    // one file is resolved inside that file first.
    for citation in &citations {
        if citation.file.is_some() {
            match resolve_file(citation, index) {
                Ok((found, repo)) => {
                    for anchor in &found {
                        resolved_files[repo].insert(anchor.file.clone());
                    }
                    anchors.extend(found);
                }
                Err(abstention) => abstentions.push(abstention),
            }
        } else {
            deferred.push(citation);
        }
    }

    // Pass 2: identifiers.
    for citation in deferred {
        match resolve_symbol(citation, index, &resolved_files) {
            Ok(mut found) => anchors.append(&mut found),
            Err(abstention) => abstentions.push(abstention),
        }
    }

    anchors.sort();
    anchors.dedup();
    abstentions.sort();
    abstentions.dedup();
    AnchorSet {
        schema: ANCHORS_SCHEMA.to_string(),
        cites_code: true,
        citations,
        anchors,
        abstentions,
    }
}

// ------------------------------------------------------------------- files

fn resolve_file(citation: &Citation, index: &IndexSet) -> Result<(Vec<Anchor>, usize), Abstention> {
    let file = citation
        .file
        .as_deref()
        .expect("resolve_file called with a file citation");
    let mut matches: Vec<(usize, String)> = Vec::new();
    for repo in index.candidates(citation.repo_hint.as_deref()) {
        for path in &index.lookup(repo).files {
            if file_matches(file, path) {
                matches.push((repo, path.clone()));
            }
        }
    }
    if matches.is_empty() {
        return Err(Abstention {
            citation: Some(citation.raw.clone()),
            reason: AbstainReason::FileMissing,
            detail: format!("no indexed file matches {file}"),
        });
    }
    // Exact path beats a suffix or basename match; otherwise the shortest path
    // wins, which prefers the in-scope file over a vendored copy.
    let exact: Vec<_> = matches
        .iter()
        .filter(|(_, path)| path == file)
        .cloned()
        .collect();
    let (repo, path) = if exact.len() == 1 {
        exact.into_iter().next().expect("one exact match")
    } else if exact.len() > 1 {
        return Err(Abstention {
            citation: Some(citation.raw.clone()),
            reason: AbstainReason::Ambiguous,
            detail: format!("{} indexed files are exactly {file}", exact.len()),
        });
    } else {
        let mut by_length = matches.clone();
        by_length.sort_by(|a, b| a.1.len().cmp(&b.1.len()).then_with(|| a.1.cmp(&b.1)));
        let shortest = by_length[0].1.len();
        if by_length
            .iter()
            .filter(|(_, p)| p.len() == shortest)
            .count()
            > 1
        {
            return Err(Abstention {
                citation: Some(citation.raw.clone()),
                reason: AbstainReason::Ambiguous,
                detail: format!("{} indexed files match {file}", matches.len()),
            });
        }
        by_length.into_iter().next().expect("non-empty matches")
    };
    resolve_in_file(citation, index, repo, &path).map(|anchors| (anchors, repo))
}

/// The in-scope file preferred over a vendored copy with the same tail.
fn file_matches(cited: &str, path: &str) -> bool {
    if cited == path {
        return true;
    }
    if cited.contains('/') {
        if path.ends_with(&format!("/{cited}")) {
            return true;
        }
        // A citation may keep the repository directory in front of the path
        // (`TapiocaZ/contracts/X.sol`) while the checkout root is the
        // sub-project itself.
        if let Some((_, rest)) = cited.split_once('/') {
            if rest == path {
                return true;
            }
        }
        return false;
    }
    path.rsplit('/').next() == Some(cited)
}

fn resolve_in_file(
    citation: &Citation,
    index: &IndexSet,
    repo_idx: usize,
    path: &str,
) -> Result<Vec<Anchor>, Abstention> {
    let repo = &index.repos[repo_idx];
    let fact = match repo.files.get(path) {
        Some(fact) => fact,
        None => {
            return Err(Abstention {
                citation: Some(citation.raw.clone()),
                reason: AbstainReason::FileMissing,
                detail: format!("no indexed file matches {path}"),
            })
        }
    };
    if !fact.parsed {
        return Err(Abstention {
            citation: Some(citation.raw.clone()),
            reason: AbstainReason::ParseFailure,
            detail: format!(
                "{path} did not parse: {}",
                fact.parse_error.as_deref().unwrap_or("unknown reason")
            ),
        });
    }

    if let (Some(start), Some(end)) = (citation.line_start, citation.line_end) {
        if start == 0
            || end < start
            || start > fact.line_count
            || end > fact.line_count
            || fact.line_starts.is_empty()
        {
            return Err(Abstention {
                citation: Some(citation.raw.clone()),
                reason: AbstainReason::LineOutOfRange,
                detail: format!(
                    "lines {start}-{end} fall outside {path} ({} lines)",
                    fact.line_count
                ),
            });
        }
        // A symbol named by the same citation, when it lives inside the cited
        // lines, is the strongest anchor: the function declaration itself.
        if let Some(name) = &citation.symbol {
            let found = symbols_in_file(repo, path, name, citation.qualifier.as_deref());
            let inside: Vec<&SymbolInfo> = found
                .iter()
                .copied()
                .filter(|s| s.span.start_line <= end && s.span.end_line >= start)
                .collect();
            if !inside.is_empty() {
                return Ok(inside.iter().map(|s| anchor_of(repo, s, 99)).collect());
            }
        } else {
            // A line fragment that exactly contains one declaration (the
            // `#L12-L16` a GitHub URL links to) names that declaration.
            let contained: Vec<&SymbolInfo> = repo
                .contracts
                .iter()
                .chain(repo.functions.iter())
                .chain(repo.state_variables.iter())
                .filter(|s| s.file == path && s.span.start_line >= start && s.span.end_line <= end)
                .collect();
            if contained.len() == 1 {
                return Ok(vec![anchor_of(repo, contained[0], 97)]);
            }
        }
        let start_byte = fact.line_starts[start as usize - 1];
        let end_byte = fact
            .line_starts
            .get(end as usize)
            .copied()
            .unwrap_or(fact.bytes);
        return Ok(vec![Anchor {
            repo: repo.repo.clone(),
            commit: repo.commit.clone(),
            file: path.to_string(),
            span: Span {
                start_byte,
                end_byte: end_byte.max(start_byte),
                start_line: start,
                end_line: end,
            },
            kind: AnchorKind::File,
            symbol: path.rsplit('/').next().unwrap_or(path).to_string(),
            confidence: 95,
        }]);
    }

    if let Some(name) = &citation.symbol {
        let found = symbols_in_file(repo, path, name, citation.qualifier.as_deref());
        if found.is_empty() {
            return Err(Abstention {
                citation: Some(citation.raw.clone()),
                reason: AbstainReason::NotFound,
                detail: format!("no declaration named {name} in {path}"),
            });
        }
        return Ok(found.iter().map(|s| anchor_of(repo, s, 92)).collect());
    }

    Ok(vec![Anchor {
        repo: repo.repo.clone(),
        commit: repo.commit.clone(),
        file: path.to_string(),
        span: Span {
            start_byte: 0,
            end_byte: fact.bytes,
            start_line: 1,
            end_line: fact.line_count.max(1),
        },
        kind: AnchorKind::File,
        symbol: path.rsplit('/').next().unwrap_or(path).to_string(),
        confidence: 90,
    }])
}

fn symbols_in_file<'a>(
    repo: &'a RepoIndex,
    path: &str,
    name: &str,
    qualifier: Option<&str>,
) -> Vec<&'a SymbolInfo> {
    let mut out: Vec<&SymbolInfo> = Vec::new();
    for symbol in repo
        .contracts
        .iter()
        .chain(repo.functions.iter())
        .chain(repo.state_variables.iter())
    {
        if symbol.file != path || symbol.name != name {
            continue;
        }
        if let Some(qualifier) = qualifier {
            if symbol.contract.as_deref() != Some(qualifier) {
                continue;
            }
        }
        out.push(symbol);
    }
    out.sort();
    out.dedup();
    out
}

// --------------------------------------------------------------- symbols

fn resolve_symbol(
    citation: &Citation,
    index: &IndexSet,
    resolved_files: &[BTreeSet<String>],
) -> Result<Vec<Anchor>, Abstention> {
    let name = match &citation.symbol {
        Some(name) => name,
        None => {
            return Err(Abstention {
                citation: Some(citation.raw.clone()),
                reason: AbstainReason::NotFound,
                detail: "the citation names neither a file nor a symbol".to_string(),
            })
        }
    };
    let repos = index.candidates(citation.repo_hint.as_deref());

    if let Some(qualifier) = &citation.qualifier {
        let mut out = Vec::new();
        for repo_idx in &repos {
            let repo = &index.repos[*repo_idx];
            let lookup = index.lookup(*repo_idx);
            let Some(contracts) = lookup.contracts_by_name.get(qualifier) else {
                continue;
            };
            for &contract_idx in contracts {
                let contract = &repo.contracts[contract_idx];
                // A qualified member resolves through the contract hierarchy.
                for scope in lookup.hierarchy(&contract.name) {
                    let mut level: Vec<&SymbolInfo> = Vec::new();
                    for symbol in repo.functions.iter() {
                        if symbol.contract.as_deref() == Some(scope.as_str())
                            && symbol.name == *name
                        {
                            level.push(symbol);
                        }
                    }
                    for symbol in repo.state_variables.iter() {
                        if symbol.contract.as_deref() == Some(scope.as_str())
                            && symbol.name == *name
                        {
                            level.push(symbol);
                        }
                    }
                    if !level.is_empty() {
                        for symbol in level {
                            out.push(anchor_of(repo, symbol, 95));
                        }
                        break;
                    }
                }
                // A qualified name may also be the contract itself in prose
                // (`Vault.balanceOf` where `Vault` is the declared contract).
                if name == &contract.name {
                    out.push(anchor_of(repo, contract, 95));
                }
            }
        }
        out.sort();
        out.dedup();
        if out.is_empty() {
            return Err(Abstention {
                citation: Some(citation.raw.clone()),
                reason: AbstainReason::NotFound,
                detail: format!("no declaration named {qualifier}.{name}"),
            });
        }
        return Ok(out);
    }

    // Unqualified: gather every declaration with this name.
    let mut candidates: Vec<(&RepoIndex, &SymbolInfo)> = Vec::new();
    for repo_idx in &repos {
        let repo = &index.repos[*repo_idx];
        for symbol in repo
            .contracts
            .iter()
            .chain(repo.functions.iter())
            .chain(repo.state_variables.iter())
        {
            if symbol.name == *name {
                candidates.push((repo, symbol));
            }
        }
    }
    if candidates.is_empty() {
        return Err(Abstention {
            citation: Some(citation.raw.clone()),
            reason: AbstainReason::NotFound,
            detail: format!("no declaration named {name}"),
        });
    }

    // Prefer the files this text already resolved.
    if candidates.len() > 1 {
        let context: Vec<(&RepoIndex, &SymbolInfo)> = candidates
            .iter()
            .copied()
            .filter(|(repo, symbol)| {
                index
                    .repos
                    .iter()
                    .position(|r| std::ptr::eq(r, *repo))
                    .is_some_and(|i| {
                        resolved_files
                            .get(i)
                            .is_some_and(|files| files.contains(&symbol.file))
                    })
            })
            .collect();
        if !context.is_empty() {
            candidates = context;
        }
    }

    // Several declarations with one name are all real spans of that name.
    // More than one distinct declaration is ambiguous only when they are
    // different contracts: an overload set is still the cited function.
    let mut distinct: Vec<(String, String)> = candidates
        .iter()
        .map(|(repo, symbol)| {
            (
                repo.repo.clone(),
                symbol
                    .contract
                    .clone()
                    .unwrap_or_else(|| symbol.file.clone()),
            )
        })
        .collect();
    distinct.sort();
    distinct.dedup();
    if distinct.len() > 1 {
        return Err(Abstention {
            citation: Some(citation.raw.clone()),
            reason: AbstainReason::Ambiguous,
            detail: format!(
                "{} declarations named {name} in {} containers",
                candidates.len(),
                distinct.len()
            ),
        });
    }
    if candidates.len() == 1 {
        let (repo, symbol) = candidates[0];
        return Ok(vec![anchor_of(repo, symbol, 85)]);
    }
    Ok(candidates
        .iter()
        .map(|(repo, symbol)| anchor_of(repo, symbol, 80))
        .collect())
}

fn anchor_of(repo: &RepoIndex, symbol: &SymbolInfo, confidence: u8) -> Anchor {
    Anchor {
        repo: repo.repo.clone(),
        commit: repo.commit.clone(),
        file: symbol.file.clone(),
        span: symbol.span,
        kind: symbol.kind,
        symbol: match &symbol.contract {
            Some(contract) => format!("{contract}.{}", symbol.name),
            None => symbol.name.clone(),
        },
        confidence,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anchors::index::{parse_repo, IndexOptions};
    use std::path::PathBuf;

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

    #[test]
    fn a_bare_file_path_is_a_file_anchor() {
        let set = resolve_text(
            "See `contracts/Vault.sol` for the deposit path.",
            &fixture(),
        );
        assert!(set.cites_code);
        assert_eq!(set.anchors.len(), 1, "{set:#?}");
        assert_eq!(set.anchors[0].kind, AnchorKind::File);
        assert_eq!(set.anchors[0].file, "contracts/Vault.sol");
        assert_eq!(set.anchors[0].span.start_line, 1);
    }

    #[test]
    fn a_qualified_function_resolves_to_its_declaration() {
        let set = resolve_text("`Vault.compound` never checks the result.", &fixture());
        assert_eq!(set.anchors.len(), 1, "{set:#?}");
        assert_eq!(set.anchors[0].kind, AnchorKind::Function);
        assert_eq!(set.anchors[0].symbol, "Vault.compound");
        assert!(set.abstentions.is_empty(), "{set:#?}");
    }

    #[test]
    fn a_state_variable_resolves_to_its_declaration() {
        let set = resolve_text("`totalAssets` is updated before the transfer.", &fixture());
        assert_eq!(set.anchors.len(), 1, "{set:#?}");
        assert_eq!(set.anchors[0].kind, AnchorKind::StateVar);
        assert_eq!(set.anchors[0].symbol, "Vault.totalAssets");
    }

    #[test]
    fn an_inherited_state_variable_resolves_through_the_base() {
        let set = resolve_text("`baseValue` is written by the base contract.", &fixture());
        assert_eq!(set.anchors.len(), 1, "{set:#?}");
        assert_eq!(set.anchors[0].symbol, "Base.baseValue");
    }

    #[test]
    fn a_contract_name_resolves_to_the_contract() {
        let set = resolve_text("`Vault` inherits `Base`.", &fixture());
        let names: Vec<_> = set.anchors.iter().map(|a| a.symbol.as_str()).collect();
        assert!(names.contains(&"Vault"), "{set:#?}");
        assert!(names.contains(&"Base"), "{set:#?}");
        assert!(set.anchors.iter().all(|a| a.kind == AnchorKind::Contract));
    }

    #[test]
    fn a_github_url_with_lines_resolves_to_the_function_span() {
        // Lines 12-16 are exactly the `deposit` declaration in the fixture.
        let set = resolve_text(
            "https://github.com/example/anchor-fixture/blob/main/contracts/Vault.sol#L12-L16",
            &fixture(),
        );
        assert_eq!(set.anchors.len(), 1, "{set:#?}");
        assert_eq!(set.anchors[0].kind, AnchorKind::Function);
        assert_eq!(set.anchors[0].symbol, "Vault.deposit");
        assert_eq!(set.anchors[0].span.start_line, 12);
        assert_eq!(set.anchors[0].span.end_line, 16);
    }

    #[test]
    fn a_missing_symbol_abstains_with_not_found() {
        let set = resolve_text("`doesNotExist()` is never declared.", &fixture());
        assert!(set.anchors.is_empty(), "{set:#?}");
        assert!(set
            .abstentions
            .iter()
            .any(|a| a.reason == AbstainReason::NotFound));
    }

    #[test]
    fn a_missing_file_abstains_with_file_missing() {
        let set = resolve_text("`contracts/Missing.sol` holds the check.", &fixture());
        assert!(set.anchors.is_empty(), "{set:#?}");
        assert!(set
            .abstentions
            .iter()
            .any(|a| a.reason == AbstainReason::FileMissing));
    }

    #[test]
    fn an_out_of_range_line_abstains_and_never_clamps() {
        let set = resolve_text(
            "https://github.com/example/anchor-fixture/blob/main/contracts/Vault.sol#L9000-L9001",
            &fixture(),
        );
        assert!(set.anchors.is_empty(), "{set:#?}");
        assert!(set
            .abstentions
            .iter()
            .any(|a| a.reason == AbstainReason::LineOutOfRange));
    }

    #[test]
    fn an_unparseable_file_abstains_with_parse_failure() {
        let set = resolve_text("`contracts/Broken.sol` has the bug.", &fixture());
        assert!(set.anchors.is_empty(), "{set:#?}");
        assert!(set
            .abstentions
            .iter()
            .any(|a| a.reason == AbstainReason::ParseFailure));
    }

    #[test]
    fn an_ambiguous_unqualified_name_abstains() {
        let set = resolve_text("`peek()` is wrong.", &fixture());
        assert!(set.anchors.is_empty(), "{set:#?}");
        assert!(set
            .abstentions
            .iter()
            .any(|a| a.reason == AbstainReason::Ambiguous));
    }

    #[test]
    fn prose_without_code_has_no_citation() {
        let set = resolve_text("The protocol loses money when the price moves.", &fixture());
        assert!(!set.cites_code);
        assert!(set.anchors.is_empty());
        assert_eq!(set.abstentions.len(), 1);
        assert_eq!(set.abstentions[0].reason, AbstainReason::NoCitation);
    }

    #[test]
    fn resolution_is_deterministic() {
        let text = "`Vault.deposit` at contracts/Vault.sol:12-16 and `totalAssets`";
        let a = resolve_text(text, &fixture());
        let b = resolve_text(text, &fixture());
        assert_eq!(
            serde_json::to_vec(&a).expect("a"),
            serde_json::to_vec(&b).expect("b")
        );
    }
}
