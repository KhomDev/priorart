//! The audited-repository index: contracts, functions, state variables,
//! call graph and state-variable read/write sets, parsed with `solar`.
//!
//! One `Session` and one `Arena` are created per file, so the lifetime of the
//! AST never escapes the file being parsed and a parse failure is attributed to
//! exactly that file. The facts extracted from each file are owned values with
//! byte and line spans.
//!
//! The model is deliberately lexical. It resolves a call by name through the
//! current contract, then its bases, then free functions; a call through a
//! variable or an unknown interface stays unresolved rather than being guessed.
//! Inline assembly is not modelled. Overloaded functions share one id, so their
//! call-graph entry and read/write set are the union of the overloads; every
//! declaration still keeps its own span.

use crate::anchors::{AnchorKind, Span};
use serde::{Deserialize, Serialize};
use solar_parse::ast::{self as ast, ExprKind, FunctionKind, ItemKind, StmtKind, UnOpKind};
use solar_parse::interface::source_map::{FileName, SourceFile};
use solar_parse::interface::{Session, SpannedOption};
use solar_parse::Parser;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// Directories that never hold the audited source and are skipped by default.
pub const DEFAULT_SKIP_DIRS: [&str; 9] = [
    ".git",
    "node_modules",
    "lib",
    "out",
    "cache",
    "artifacts",
    "target",
    "broadcast",
    "coverage",
];

/// Safety caps, so one pathological checkout cannot hang a run.
#[derive(Debug, Clone)]
pub struct IndexOptions {
    pub skip_dirs: Vec<String>,
    pub max_files: usize,
    pub max_file_bytes: usize,
}

impl Default for IndexOptions {
    fn default() -> Self {
        Self {
            skip_dirs: DEFAULT_SKIP_DIRS.iter().map(|s| s.to_string()).collect(),
            max_files: 5_000,
            max_file_bytes: 2 * 1024 * 1024,
        }
    }
}

/// A contract, function or state-variable declaration.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SymbolInfo {
    /// Stable id: `Contract::name` for a function, `Contract.var` for a state
    /// variable, `Contract` for a contract, `file::name` for free functions.
    pub id: String,
    pub name: String,
    pub kind: AnchorKind,
    pub file: String,
    pub span: Span,
    /// Declaring contract, or `None` for a file-level item.
    pub contract: Option<String>,
    /// For a contract: direct base-contract names. Empty otherwise.
    pub bases: Vec<String>,
    /// For a function: `function`, `constructor`, `fallback`, `receive` or
    /// `modifier`. `None` for contracts and state variables.
    pub function_kind: Option<String>,
    /// For a state variable: the last segment of a user-defined type
    /// (`BalanceSheet` in `BalanceSheet internal sheet`). `None` otherwise.
    /// Used to resolve `sheet.addDebt(...)` to `BalanceSheet.addDebt`.
    pub var_type: Option<String>,
}

/// What was learned from one file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileFact {
    pub path: String,
    pub bytes: u32,
    pub line_count: u32,
    /// Byte offset of each 1-based line start (`line_starts[0]` is line 1).
    pub line_starts: Vec<u32>,
    pub parsed: bool,
    pub parse_error: Option<String>,
    pub contracts: Vec<SymbolInfo>,
    pub functions: Vec<SymbolInfo>,
    pub state_variables: Vec<SymbolInfo>,
    /// One entry per function id: the state variable names its body reads and
    /// writes. Names are qualified during index assembly.
    pub read_write: Vec<(String, ReadWrite)>,
    pub calls: Vec<RawCall>,
}

/// A state-variable access set. Names are `Contract.var` after assembly.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadWrite {
    pub reads: Vec<String>,
    pub writes: Vec<String>,
}

/// A call before name resolution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawCall {
    pub caller: String,
    pub name: String,
    pub receiver: Option<String>,
    pub kind: RawCallKind,
    pub span: Span,
    /// Declaring contract of the caller, so resolution does not parse ids.
    pub contract: Option<String>,
    pub file: String,
}

/// Where a raw call came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RawCallKind {
    /// A call expression in a function body.
    Expr,
    /// A modifier invocation in a function header.
    Modifier,
    /// A base-constructor invocation in a constructor header.
    BaseConstructor,
}

/// A resolved or unresolved call edge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CallEdge {
    pub caller: String,
    pub callee: Option<String>,
    pub name: String,
    pub receiver: Option<String>,
    pub file: String,
    pub span: Span,
}

/// A file that exists but did not parse.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParseFailure {
    pub file: String,
    pub reason: String,
}

/// One audited repository at one commit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoIndex {
    pub repo: String,
    pub commit: String,
    /// Forward-slash display path; never a host path when produced by the
    /// measurement command (it uses the repository id there).
    pub root: String,
    pub commit_verified: bool,
    pub files: BTreeMap<String, FileFact>,
    pub contracts: Vec<SymbolInfo>,
    pub functions: Vec<SymbolInfo>,
    pub state_variables: Vec<SymbolInfo>,
    /// Function id -> resolved callee ids, sorted and deduplicated.
    pub call_graph: BTreeMap<String, Vec<String>>,
    /// Every call site, resolved or not.
    pub calls: Vec<CallEdge>,
    /// Function id -> qualified state-variable read/write set.
    pub read_write: BTreeMap<String, ReadWrite>,
    pub parse_failures: Vec<ParseFailure>,
}

/// One or more repositories searched together. A finding may cite two
/// checkouts (for example a contest snapshot plus a protocol repository).
pub struct IndexSet {
    pub repos: Vec<RepoIndex>,
    lookups: Vec<RepoLookup>,
}

impl IndexSet {
    pub fn new(repos: Vec<RepoIndex>) -> Self {
        let lookups = repos.iter().map(RepoLookup::build).collect();
        Self { repos, lookups }
    }

    pub fn single(index: RepoIndex) -> Self {
        Self::new(vec![index])
    }

    /// Repositories that match a URL hint (`owner/repo`, `repo`), or all when
    /// there is no hint.
    pub fn candidates(&self, hint: Option<&str>) -> Vec<usize> {
        match hint {
            None => (0..self.repos.len()).collect(),
            Some(hint) => {
                let hint = hint.trim_end_matches(".git");
                let stripped = hint.rsplit('/').next().unwrap_or(hint);
                let mut out: Vec<usize> = self
                    .repos
                    .iter()
                    .enumerate()
                    .filter(|(_, r)| {
                        let repo = r.repo.trim_end_matches(".git");
                        repo == hint
                            || repo.rsplit('/').next() == Some(stripped)
                            || repo.ends_with(&format!("/{hint}"))
                    })
                    .map(|(i, _)| i)
                    .collect();
                if out.is_empty() {
                    out = (0..self.repos.len()).collect();
                }
                out
            }
        }
    }

    pub(crate) fn lookup(&self, idx: usize) -> &RepoLookup {
        &self.lookups[idx]
    }
}

/// Precomputed name tables for one repository.
pub(crate) struct RepoLookup {
    pub(crate) files: Vec<String>,
    pub(crate) contracts_by_name: BTreeMap<String, Vec<usize>>,
    pub(crate) functions_by_name: BTreeMap<String, Vec<usize>>,
    pub(crate) functions_by_contract: BTreeMap<String, Vec<usize>>,
    pub(crate) vars_by_contract: BTreeMap<String, BTreeSet<String>>,
    pub(crate) bases: BTreeMap<String, Vec<String>>,
    /// `Contract.var` -> the last segment of its user-defined type.
    pub(crate) var_types: BTreeMap<String, String>,
    /// A copy of the function declarations, so call resolution does not need
    /// a borrow of the repository that owns the lookup.
    functions: Vec<SymbolInfo>,
}

impl RepoLookup {
    fn build(repo: &RepoIndex) -> Self {
        let mut contracts_by_name: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        for (i, c) in repo.contracts.iter().enumerate() {
            contracts_by_name.entry(c.name.clone()).or_default().push(i);
        }
        let mut functions_by_name: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        let mut functions_by_contract: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        for (i, f) in repo.functions.iter().enumerate() {
            functions_by_name.entry(f.name.clone()).or_default().push(i);
            if let Some(c) = &f.contract {
                functions_by_contract.entry(c.clone()).or_default().push(i);
            }
        }
        let mut vars_by_contract: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        let mut var_types: BTreeMap<String, String> = BTreeMap::new();
        for v in repo.state_variables.iter() {
            if let Some(c) = &v.contract {
                vars_by_contract
                    .entry(c.clone())
                    .or_default()
                    .insert(v.name.clone());
                if let Some(ty) = &v.var_type {
                    var_types.insert(format!("{c}.{}", v.name), ty.clone());
                }
            }
        }
        let mut bases: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for c in &repo.contracts {
            bases.insert(c.name.clone(), c.bases.clone());
        }
        Self {
            files: repo.files.keys().cloned().collect(),
            contracts_by_name,
            functions_by_name,
            functions_by_contract,
            vars_by_contract,
            bases,
            var_types,
            functions: repo.functions.clone(),
        }
    }

    /// Contracts reachable from `contract` (itself first, then bases in
    /// declaration order), without repeating a name.
    pub(crate) fn hierarchy(&self, contract: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut seen = BTreeSet::new();
        let mut stack = vec![contract.to_string()];
        while let Some(name) = stack.pop() {
            if !seen.insert(name.clone()) {
                continue;
            }
            out.push(name.clone());
            if let Some(bases) = self.bases.get(&name) {
                for base in bases.iter().rev() {
                    stack.push(base.clone());
                }
            }
        }
        out
    }
}

/// Parse every Solidity file under `root` into one index.
pub fn parse_repo(
    root: &Path,
    repo: &str,
    commit: &str,
    options: &IndexOptions,
) -> Result<RepoIndex, String> {
    if !root.is_dir() {
        return Err(format!("repository directory does not exist: {root:?}"));
    }
    let mut paths = Vec::new();
    collect_sol_files(root, root, options, &mut paths)?;
    paths.sort();
    paths.truncate(options.max_files);

    let mut files: BTreeMap<String, FileFact> = BTreeMap::new();
    let mut parse_failures = Vec::new();
    for rel in &paths {
        let abs = root.join(rel);
        match parse_one(&abs, rel, options) {
            Ok(fact) => {
                files.insert(rel.clone(), fact);
            }
            Err(reason) => {
                files.insert(
                    rel.clone(),
                    FileFact {
                        path: rel.clone(),
                        bytes: 0,
                        line_count: 0,
                        line_starts: Vec::new(),
                        parsed: false,
                        parse_error: Some(reason.clone()),
                        contracts: Vec::new(),
                        functions: Vec::new(),
                        state_variables: Vec::new(),
                        read_write: Vec::new(),
                        calls: Vec::new(),
                    },
                );
                parse_failures.push(ParseFailure {
                    file: rel.clone(),
                    reason,
                });
            }
        }
    }

    let mut contracts = Vec::new();
    let mut functions = Vec::new();
    let mut state_variables = Vec::new();
    let mut raw_calls = Vec::new();
    let mut raw_rw: Vec<(String, ReadWrite)> = Vec::new();
    for fact in files.values() {
        contracts.extend(fact.contracts.iter().cloned());
        functions.extend(fact.functions.iter().cloned());
        state_variables.extend(fact.state_variables.iter().cloned());
        raw_calls.extend(fact.calls.iter().cloned());
        for (id, rw) in &fact.read_write {
            raw_rw.push((id.clone(), rw.clone()));
        }
    }
    contracts.sort();
    functions.sort();
    state_variables.sort();
    raw_calls.sort_by(|a, b| {
        (a.file.as_str(), a.span, a.caller.as_str(), a.name.as_str()).cmp(&(
            b.file.as_str(),
            b.span,
            b.caller.as_str(),
            b.name.as_str(),
        ))
    });

    let mut index = RepoIndex {
        repo: repo.to_string(),
        commit: commit.to_string(),
        root: display_path(root),
        commit_verified: false,
        files,
        contracts,
        functions,
        state_variables,
        call_graph: BTreeMap::new(),
        calls: Vec::new(),
        read_write: BTreeMap::new(),
        parse_failures,
    };
    let lookup = RepoLookup::build(&index);

    // Resolve calls and qualify read/write names.
    let mut graph: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut edges = Vec::new();
    for raw in raw_calls {
        let callees = lookup.resolve_raw_call(&raw);
        for callee in &callees {
            graph
                .entry(raw.caller.clone())
                .or_default()
                .insert(callee.clone());
        }
        edges.push(CallEdge {
            caller: raw.caller,
            callee: match callees.as_slice() {
                [one] => Some(one.clone()),
                _ => None,
            },
            name: raw.name,
            receiver: raw.receiver,
            file: raw.file,
            span: raw.span,
        });
    }
    index.call_graph = graph
        .into_iter()
        .map(|(k, v)| (k, v.into_iter().collect()))
        .collect();
    edges.sort_by(|a, b| {
        (a.file.as_str(), a.span, a.caller.as_str(), a.name.as_str()).cmp(&(
            b.file.as_str(),
            b.span,
            b.caller.as_str(),
            b.name.as_str(),
        ))
    });
    index.calls = edges;

    let mut function_contract: BTreeMap<String, Option<String>> = BTreeMap::new();
    for f in &index.functions {
        function_contract.insert(f.id.clone(), f.contract.clone());
    }
    let mut rw_map: BTreeMap<String, ReadWrite> = BTreeMap::new();
    for (id, rw) in raw_rw {
        let contract = function_contract.get(&id).cloned().flatten();
        // The walker collected every identifier it saw. Only names that are a
        // state variable visible from this function's contract survive, so
        // locals, parameters and language builtins drop out here.
        let qualify = |names: Vec<String>| -> Vec<String> {
            let mut out: BTreeSet<String> = BTreeSet::new();
            let Some(contract) = &contract else {
                return Vec::new();
            };
            for name in names {
                if let Some(owner) = lookup.var_owner(contract, &name) {
                    out.insert(format!("{owner}.{name}"));
                }
            }
            out.into_iter().collect()
        };
        let entry = rw_map.entry(id).or_default();
        for read in qualify(rw.reads) {
            if !entry.reads.contains(&read) {
                entry.reads.push(read);
            }
        }
        for write in qualify(rw.writes) {
            if !entry.writes.contains(&write) {
                entry.writes.push(write);
            }
        }
    }
    for rw in rw_map.values_mut() {
        rw.reads.sort();
        rw.writes.sort();
    }
    index.read_write = rw_map;
    Ok(index)
}

impl RepoLookup {
    /// The contract that declares a visible state variable named `name`.
    fn var_owner(&self, contract: &str, name: &str) -> Option<String> {
        self.hierarchy(contract).into_iter().find(|c| {
            self.vars_by_contract
                .get(c)
                .is_some_and(|vars| vars.contains(name))
        })
    }

    /// The declared user-defined type of a visible state variable, if any.
    fn var_type(&self, contract: &str, name: &str) -> Option<String> {
        for c in self.hierarchy(contract) {
            if let Some(ty) = self.var_types.get(&format!("{c}.{name}")) {
                return Some(ty.clone());
            }
        }
        None
    }

    /// Resolve a raw call to zero or more function ids.
    fn resolve_raw_call(&self, raw: &RawCall) -> Vec<String> {
        match raw.kind {
            RawCallKind::BaseConstructor => {
                if self.contracts_by_name.contains_key(&raw.name) {
                    vec![format!("{}::constructor", raw.name)]
                } else {
                    Vec::new()
                }
            }
            RawCallKind::Modifier => {
                self.find_in_hierarchy(raw.contract.as_deref(), &raw.name, true)
            }
            RawCallKind::Expr => match raw.receiver.as_deref() {
                Some("this") => self.find_in_hierarchy(raw.contract.as_deref(), &raw.name, false),
                Some("super") => {
                    let mut out = Vec::new();
                    if let Some(contract) = &raw.contract {
                        for base in self
                            .bases
                            .get(contract)
                            .map(|b| b.as_slice())
                            .unwrap_or(&[])
                        {
                            out.extend(self.find_in_hierarchy(Some(base), &raw.name, false));
                        }
                    }
                    dedup(out)
                }
                Some(receiver) => {
                    if self.contracts_by_name.contains_key(receiver) {
                        self.find_in_hierarchy(Some(receiver), &raw.name, false)
                    } else {
                        // A call through a state variable whose declared type
                        // is an indexed contract: `sheet.addDebt(...)` with
                        // `BalanceSheet internal sheet`.
                        let target = raw
                            .contract
                            .as_deref()
                            .and_then(|caller| self.var_type(caller, receiver));
                        match target {
                            Some(ty) if self.contracts_by_name.contains_key(&ty) => {
                                self.find_in_hierarchy(Some(&ty), &raw.name, false)
                            }
                            _ => Vec::new(),
                        }
                    }
                }
                None => {
                    let mut out = self.find_in_hierarchy(raw.contract.as_deref(), &raw.name, false);
                    if out.is_empty() {
                        out = self.find_free(&raw.name);
                    }
                    out
                }
            },
        }
    }

    fn find_in_hierarchy(&self, contract: Option<&str>, name: &str, modifier: bool) -> Vec<String> {
        let Some(contract) = contract else {
            return Vec::new();
        };
        for c in self.hierarchy(contract) {
            let ids = self.functions_in(&c, name, modifier);
            if !ids.is_empty() {
                return ids;
            }
        }
        Vec::new()
    }

    fn functions_in(&self, contract: &str, name: &str, modifier: bool) -> Vec<String> {
        let Some(indices) = self.functions_by_contract.get(contract) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for &i in indices {
            let f = &self.function_at(i);
            let is_modifier = f.function_kind.as_deref() == Some("modifier");
            if is_modifier != modifier {
                continue;
            }
            if f.name == name {
                out.push(f.id.clone());
            }
        }
        dedup(out)
    }

    fn find_free(&self, name: &str) -> Vec<String> {
        let Some(indices) = self.functions_by_name.get(name) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for &i in indices {
            let f = self.function_at(i);
            if f.contract.is_none() && f.function_kind.as_deref() != Some("modifier") {
                out.push(f.id.clone());
            }
        }
        dedup(out)
    }

    fn function_at(&self, index: usize) -> &SymbolInfo {
        &self.functions[index]
    }
}

fn dedup(mut ids: Vec<String>) -> Vec<String> {
    ids.sort();
    ids.dedup();
    ids
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn collect_sol_files(
    root: &Path,
    dir: &Path,
    options: &IndexOptions,
    out: &mut Vec<String>,
) -> Result<(), String> {
    let entries = std::fs::read_dir(dir)
        .map_err(|e| format!("cannot read directory {}: {e}", dir.display()))?;
    let mut names: Vec<PathBuf> = entries.filter_map(|e| e.ok()).map(|e| e.path()).collect();
    names.sort();
    for path in names {
        if path.is_dir() {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            if options.skip_dirs.iter().any(|skip| skip == &name) {
                continue;
            }
            collect_sol_files(root, &path, options, out)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("sol") {
            if let Ok(rel) = path.strip_prefix(root) {
                out.push(rel.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    Ok(())
}

/// Parse one file and extract owned facts.
fn parse_one(abs: &Path, rel: &str, options: &IndexOptions) -> Result<FileFact, String> {
    let meta = std::fs::metadata(abs).map_err(|e| format!("cannot stat: {e}"))?;
    let bytes = meta.len();
    if bytes > options.max_file_bytes as u64 {
        return Err(format!(
            "file is {bytes} bytes, above the {} byte cap",
            options.max_file_bytes
        ));
    }
    let src = std::fs::read_to_string(abs).map_err(|e| format!("cannot read as UTF-8: {e}"))?;

    let sess = Session::builder()
        .with_buffer_emitter(solar_parse::interface::ColorChoice::Never)
        .single_threaded()
        .build();
    let file_name = FileName::Real(abs.to_path_buf());
    let result = sess.enter_sequential(|| -> Result<FileFact, String> {
        let arena = ast::Arena::new();
        let mut parser = Parser::from_source_code(&sess, &arena, file_name.clone(), src)
            .map_err(|_| "solar could not read the source into its map".to_string())?;
        let ast = match parser.parse_file() {
            Ok(ast) => ast,
            Err(err) => {
                err.cancel();
                return Err("solar reported a syntax error".to_string());
            }
        };
        if let Some(Err(_)) = sess.emitted_errors() {
            return Err("solar emitted a diagnostic error".to_string());
        }
        let source: std::sync::Arc<SourceFile> = sess
            .source_map()
            .get_file(file_name.clone())
            .ok_or_else(|| "parsed file missing from solar's source map".to_string())?;
        Ok(extract_file(&source, rel, &ast))
    });
    result
}

fn extract_file(source: &SourceFile, rel: &str, ast: &ast::SourceUnit<'_>) -> FileFact {
    let line_starts: Vec<u32> = source.lines().iter().map(|p| p.to_u32()).collect();
    let ctx = SpanCtx {
        line_starts,
        src_len: source.source_len.to_u32(),
    }
    .normalised();
    let file = rel.to_string();
    let mut contracts = Vec::new();
    let mut functions = Vec::new();
    let mut state_variables = Vec::new();
    let mut read_write = Vec::new();
    let mut calls = Vec::new();

    // File-level constants are visible in every contract of the file. They
    // are not storage, so they are not part of a read/write set and the index
    // does not need their names during the walk.

    for item in ast.items.iter() {
        match &item.kind {
            ItemKind::Contract(c) => {
                let bases: Vec<String> = c
                    .bases
                    .iter()
                    .map(|m| m.name.last().as_str().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                contracts.push(SymbolInfo {
                    id: c.name.as_str().to_string(),
                    name: c.name.as_str().to_string(),
                    kind: AnchorKind::Contract,
                    file: file.clone(),
                    span: ctx.span(item.span),
                    contract: None,
                    bases: bases.clone(),
                    function_kind: None,
                    var_type: None,
                });
                // Own state variables are collected first; inheritance is
                // filled in during index assembly by name, not here.
                for sub in c.body.iter() {
                    match &sub.kind {
                        ItemKind::Variable(var) => {
                            if let Some(name) = &var.name {
                                state_variables.push(SymbolInfo {
                                    id: format!("{}.{}", c.name.as_str(), name.as_str()),
                                    name: name.as_str().to_string(),
                                    kind: AnchorKind::StateVar,
                                    file: file.clone(),
                                    span: ctx.span(sub.span),
                                    contract: Some(c.name.as_str().to_string()),
                                    bases: Vec::new(),
                                    function_kind: None,
                                    var_type: custom_type_name(&var.ty),
                                });
                            }
                        }
                        ItemKind::Function(func) => {
                            let (info, rw, raw_calls) = extract_function(
                                &ctx,
                                &file,
                                Some(c.name.as_str()),
                                sub.span,
                                func,
                            );
                            functions.push(info);
                            read_write
                                .push((functions.last().expect("just pushed").id.clone(), rw));
                            calls.extend(raw_calls);
                            // Modifier invocations live in the header.
                            for modifier in func.header.modifiers.iter() {
                                let name = modifier.name.last().as_str().to_string();
                                if name.is_empty() {
                                    continue;
                                }
                                let is_base = bases.iter().any(|b| b == &name);
                                calls.push(RawCall {
                                    caller: functions.last().expect("just pushed").id.clone(),
                                    name,
                                    receiver: None,
                                    kind: if is_base {
                                        RawCallKind::BaseConstructor
                                    } else {
                                        RawCallKind::Modifier
                                    },
                                    span: ctx.span(modifier.span()),
                                    contract: Some(c.name.as_str().to_string()),
                                    file: file.clone(),
                                });
                            }
                        }
                        _ => {}
                    }
                }
            }
            ItemKind::Function(func) => {
                let (info, rw, raw_calls) = extract_function(&ctx, &file, None, item.span, func);
                functions.push(info);
                read_write.push((functions.last().expect("just pushed").id.clone(), rw));
                calls.extend(raw_calls);
            }
            ItemKind::Variable(var) => {
                if let Some(name) = &var.name {
                    state_variables.push(SymbolInfo {
                        id: name.as_str().to_string(),
                        name: name.as_str().to_string(),
                        kind: AnchorKind::StateVar,
                        file: file.clone(),
                        span: ctx.span(item.span),
                        contract: None,
                        bases: Vec::new(),
                        function_kind: None,
                        var_type: custom_type_name(&var.ty),
                    });
                }
            }
            _ => {}
        }
    }

    contracts.sort();
    functions.sort();
    state_variables.sort();
    FileFact {
        path: file,
        bytes: ctx.src_len,
        line_count: ctx.line_starts.len() as u32,
        line_starts: ctx.line_starts,
        parsed: true,
        parse_error: None,
        contracts,
        functions,
        state_variables,
        read_write,
        calls,
    }
}

struct SpanCtx {
    line_starts: Vec<u32>,
    src_len: u32,
}

impl SpanCtx {
    fn normalised(mut self) -> Self {
        if self.line_starts.is_empty() {
            self.line_starts.push(0);
        }
        self
    }

    fn line_at(&self, pos: u32) -> u32 {
        let idx = self.line_starts.partition_point(|&start| start <= pos);
        (idx.max(1) as u32).min(self.line_starts.len() as u32)
    }

    fn span(&self, span: solar_parse::interface::Span) -> Span {
        // Spans from this file's session are absolute; subtract the file's
        // start through the line table's first position (0 for the first file,
        // which a one-file session always is).
        let start = span.lo().to_u32().min(self.src_len);
        let end = span.hi().to_u32().min(self.src_len).max(start);
        let start_line = self.line_at(start);
        let end_line = self.line_at(end.saturating_sub(1).max(start));
        Span {
            start_byte: start,
            end_byte: end,
            start_line,
            end_line,
        }
    }
}

fn extract_function(
    ctx: &SpanCtx,
    file: &str,
    contract: Option<&str>,
    item_span: solar_parse::interface::Span,
    func: &ast::ItemFunction<'_>,
) -> (SymbolInfo, ReadWrite, Vec<RawCall>) {
    let name = func
        .header
        .name
        .map(|i| i.as_str().to_string())
        .unwrap_or_else(|| match func.kind {
            FunctionKind::Constructor => "constructor".to_string(),
            FunctionKind::Fallback => "fallback".to_string(),
            FunctionKind::Receive => "receive".to_string(),
            _ => "function".to_string(),
        });
    let kind_name = match func.kind {
        FunctionKind::Constructor => "constructor",
        FunctionKind::Fallback => "fallback",
        FunctionKind::Receive => "receive",
        FunctionKind::Modifier => "modifier",
        FunctionKind::Function => "function",
    };
    let id = function_id(contract, func.kind, &name, file);
    let symbol = SymbolInfo {
        id: id.clone(),
        name: name.clone(),
        kind: AnchorKind::Function,
        file: file.to_string(),
        span: ctx.span(item_span),
        contract: contract.map(|c| c.to_string()),
        bases: Vec::new(),
        function_kind: Some(kind_name.to_string()),
        var_type: None,
    };

    let mut walker = Walker {
        ctx,
        contract: contract.map(|c| c.to_string()),
        file: file.to_string(),
        id: id.clone(),
        frames: vec![BTreeSet::new()],
        reads: BTreeSet::new(),
        writes: BTreeSet::new(),
        calls: Vec::new(),
    };
    for var in func
        .header
        .parameters
        .vars
        .iter()
        .chain(func.header.returns.iter().flat_map(|p| p.vars.iter()))
    {
        if let Some(n) = &var.name {
            walker.declare(n.as_str());
        }
    }
    if let Some(body) = &func.body {
        walker.walk_block(body);
    }
    let rw = ReadWrite {
        reads: walker.reads.into_iter().collect(),
        writes: walker.writes.into_iter().collect(),
    };
    (symbol, rw, walker.calls)
}

/// The last path segment of a user-defined type, so `BalanceSheet internal
/// sheet` yields `BalanceSheet`. Elementary, array and mapping types yield
/// `None`.
fn custom_type_name(ty: &ast::Type<'_>) -> Option<String> {
    match &ty.kind {
        ast::TypeKind::Custom(path) => Some(path.last().as_str().to_string()),
        _ => None,
    }
}

fn function_id(contract: Option<&str>, kind: FunctionKind, name: &str, file: &str) -> String {
    match contract {
        Some(c) => match kind {
            FunctionKind::Constructor => format!("{c}::constructor"),
            FunctionKind::Fallback => format!("{c}::fallback"),
            FunctionKind::Receive => format!("{c}::receive"),
            FunctionKind::Modifier => format!("{c}::modifier:{name}"),
            FunctionKind::Function => format!("{c}::{name}"),
        },
        None => format!("{file}::{name}"),
    }
}

/// Walks one function body, collecting calls and every identifier it sees.
/// Names are filtered to actual state variables during index assembly, where
/// the full inheritance hierarchy is known.
struct Walker<'a> {
    ctx: &'a SpanCtx,
    contract: Option<String>,
    file: String,
    id: String,
    frames: Vec<BTreeSet<String>>,
    reads: BTreeSet<String>,
    writes: BTreeSet<String>,
    calls: Vec<RawCall>,
}

impl Walker<'_> {
    fn declare(&mut self, name: &str) {
        if let Some(frame) = self.frames.last_mut() {
            frame.insert(name.to_string());
        }
    }

    fn shadowed(&self, name: &str) -> bool {
        self.frames.iter().rev().any(|f| f.contains(name))
    }

    /// Record an identifier in a read position. Locals and parameters declared
    /// in an enclosing scope shadow a state variable of the same name.
    fn record_read(&mut self, name: &str) {
        if !self.shadowed(name) && !is_builtin_name(name) {
            self.reads.insert(name.to_string());
        }
    }

    fn push_scope(&mut self) {
        self.frames.push(BTreeSet::new());
    }

    fn pop_scope(&mut self) {
        self.frames.pop();
    }

    fn walk_block(&mut self, block: &ast::Block<'_>) {
        self.push_scope();
        for stmt in block.stmts.iter() {
            self.walk_stmt(stmt);
        }
        self.pop_scope();
    }

    fn walk_stmt(&mut self, stmt: &ast::Stmt<'_>) {
        match &stmt.kind {
            StmtKind::Assembly(_) => {
                // Inline assembly can read and write storage only through
                // slot arithmetic; the lexical model cannot attribute it.
            }
            StmtKind::DeclSingle(var) => {
                if let Some(init) = &var.initializer {
                    self.walk_expr(init);
                }
                if let Some(n) = &var.name {
                    self.declare(n.as_str());
                }
            }
            StmtKind::DeclMulti(vars, rhs) => {
                self.walk_expr(rhs);
                for var in vars.iter() {
                    if let SpannedOption::Some(var) = var {
                        if let Some(n) = &var.name {
                            self.declare(n.as_str());
                        }
                    }
                }
            }
            StmtKind::Block(block) => self.walk_block(block),
            StmtKind::Break | StmtKind::Continue | StmtKind::Placeholder => {}
            StmtKind::DoWhile(body, cond) => {
                self.walk_scoped(body);
                self.walk_expr(cond);
            }
            StmtKind::Emit(_, args) | StmtKind::Revert(_, args) => self.walk_args(args),
            StmtKind::Expr(expr) => self.walk_expr(expr),
            StmtKind::For {
                init,
                cond,
                next,
                body,
            } => {
                self.push_scope();
                if let Some(init) = init {
                    self.walk_stmt(init);
                }
                if let Some(cond) = cond {
                    self.walk_expr(cond);
                }
                if let Some(next) = next {
                    self.walk_expr(next);
                }
                self.walk_scoped(body);
                self.pop_scope();
            }
            StmtKind::If(cond, then, otherwise) => {
                self.walk_expr(cond);
                self.walk_scoped(then);
                if let Some(otherwise) = otherwise {
                    self.walk_scoped(otherwise);
                }
            }
            StmtKind::Return(expr) => {
                if let Some(expr) = expr {
                    self.walk_expr(expr);
                }
            }
            StmtKind::Try(try_stmt) => {
                self.walk_expr(try_stmt.expr);
                for clause in try_stmt.clauses.iter() {
                    self.push_scope();
                    for var in clause.args.vars.iter() {
                        if let Some(n) = &var.name {
                            self.declare(n.as_str());
                        }
                    }
                    self.walk_block(&clause.block);
                    self.pop_scope();
                }
            }
            StmtKind::UncheckedBlock(block) => self.walk_block(block),
            StmtKind::While(cond, body) => {
                self.walk_expr(cond);
                self.walk_scoped(body);
            }
        }
    }

    /// Walk a branch or loop body. A block already opens its own scope; a
    /// single-statement body with a declaration gets one here, so the
    /// declaration does not leak into the rest of the function.
    fn walk_scoped(&mut self, stmt: &ast::Stmt<'_>) {
        if matches!(&stmt.kind, StmtKind::Block(_)) {
            self.walk_stmt(stmt);
        } else {
            self.push_scope();
            self.walk_stmt(stmt);
            self.pop_scope();
        }
    }

    fn walk_args(&mut self, args: &ast::CallArgs<'_>) {
        for expr in args.exprs() {
            self.walk_expr(expr);
        }
    }

    fn walk_expr(&mut self, expr: &ast::Expr<'_>) {
        match &expr.kind {
            ExprKind::Array(items) => {
                for item in items.iter() {
                    self.walk_expr(item);
                }
            }
            ExprKind::Assign(lhs, op, rhs) => {
                self.walk_lvalue(lhs, op.is_some());
                self.walk_expr(rhs);
            }
            ExprKind::Binary(lhs, _, rhs) => {
                self.walk_expr(lhs);
                self.walk_expr(rhs);
            }
            ExprKind::Call(callee, args) => {
                self.record_call(callee, expr.span);
                self.walk_callee(callee);
                self.walk_args(args);
            }
            ExprKind::CallOptions(inner, named) => {
                self.walk_expr(inner);
                for arg in named.iter() {
                    self.walk_expr(arg.value);
                }
            }
            ExprKind::Delete(inner) => self.walk_lvalue(inner, false),
            ExprKind::Ident(ident) => self.record_read(ident.as_str()),
            ExprKind::Index(base, kind) => {
                self.walk_expr(base);
                match kind {
                    ast::IndexKind::Index(index) => {
                        if let Some(index) = index {
                            self.walk_expr(index);
                        }
                    }
                    ast::IndexKind::Range(start, end) => {
                        if let Some(start) = start {
                            self.walk_expr(start);
                        }
                        if let Some(end) = end {
                            self.walk_expr(end);
                        }
                    }
                }
            }
            ExprKind::Lit(_, _) => {}
            ExprKind::Member(base, _) => {
                if let ExprKind::Ident(ident) = &base.kind {
                    self.record_read(ident.as_str());
                } else {
                    self.walk_expr(base);
                }
            }
            ExprKind::New(_) | ExprKind::Type(_) | ExprKind::TypeCall(_) => {}
            ExprKind::Payable(args) => self.walk_args(args),
            ExprKind::Ternary(a, b, c) => {
                self.walk_expr(a);
                self.walk_expr(b);
                self.walk_expr(c);
            }
            ExprKind::Tuple(items) => {
                for item in items.iter() {
                    if let SpannedOption::Some(inner) = item {
                        self.walk_expr(inner);
                    }
                }
            }
            ExprKind::Unary(op, inner) => {
                if matches!(
                    op.kind,
                    UnOpKind::PreInc | UnOpKind::PreDec | UnOpKind::PostInc | UnOpKind::PostDec
                ) {
                    self.walk_lvalue(inner, true);
                } else {
                    self.walk_expr(inner);
                }
            }
            ExprKind::Err(_) => {}
        }
    }

    /// Walk a write target. `also_read` is true for compound assignment and
    /// increment/decrement, which read the old value too.
    fn walk_lvalue(&mut self, expr: &ast::Expr<'_>, also_read: bool) {
        match &expr.kind {
            ExprKind::Ident(ident) => {
                let name = ident.as_str();
                if !self.shadowed(name) && !is_builtin_name(name) {
                    self.writes.insert(name.to_string());
                    if also_read {
                        self.reads.insert(name.to_string());
                    }
                }
            }
            ExprKind::Member(base, _) => self.walk_lvalue(base, also_read),
            ExprKind::Index(base, kind) => {
                self.walk_lvalue(base, also_read);
                match kind {
                    ast::IndexKind::Index(index) => {
                        if let Some(index) = index {
                            self.walk_expr(index);
                        }
                    }
                    ast::IndexKind::Range(start, end) => {
                        if let Some(start) = start {
                            self.walk_expr(start);
                        }
                        if let Some(end) = end {
                            self.walk_expr(end);
                        }
                    }
                }
            }
            ExprKind::Tuple(items) => {
                for item in items.iter() {
                    if let SpannedOption::Some(inner) = item {
                        self.walk_lvalue(inner, also_read);
                    }
                }
            }
            _ => self.walk_expr(expr),
        }
    }

    /// The callee expression may itself contain calls or reads.
    fn walk_callee(&mut self, callee: &ast::Expr<'_>) {
        match &callee.kind {
            ExprKind::Ident(_) => {}
            ExprKind::Member(base, _) => match &base.kind {
                ExprKind::Ident(ident) => self.record_read(ident.as_str()),
                _ => self.walk_expr(base),
            },
            _ => self.walk_expr(callee),
        }
    }

    fn record_call(&mut self, callee: &ast::Expr<'_>, span: solar_parse::interface::Span) {
        let (name, receiver): (Option<String>, Option<String>) = match &callee.kind {
            ExprKind::Ident(ident) => (Some(ident.as_str().to_string()), None),
            ExprKind::Member(base, field) => match &base.kind {
                ExprKind::Ident(ident) => (
                    Some(field.as_str().to_string()),
                    Some(ident.as_str().to_string()),
                ),
                _ => (Some(field.as_str().to_string()), None),
            },
            _ => (None, None),
        };
        let Some(name) = name else {
            return;
        };
        if is_builtin_call(&name) || is_elementary_type(&name) {
            return;
        }
        self.calls.push(RawCall {
            caller: self.id.clone(),
            name,
            receiver,
            kind: RawCallKind::Expr,
            span: self.ctx.span(span),
            contract: self.contract.clone(),
            file: self.file.clone(),
        });
    }
}

fn is_builtin_call(name: &str) -> bool {
    matches!(
        name,
        "require"
            | "assert"
            | "revert"
            | "keccak256"
            | "sha256"
            | "ripemd160"
            | "ecrecover"
            | "addmod"
            | "mulmod"
            | "selfdestruct"
            | "blockhash"
            | "gasleft"
            | "type"
            | "payable"
            | "abi"
            | "encode"
            | "encodePacked"
            | "encodeWithSelector"
            | "encodeWithSignature"
            | "decode"
    )
}

/// Language names and builtins that are never state variables.
fn is_builtin_name(name: &str) -> bool {
    if is_elementary_type(name) {
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
            | "require"
            | "assert"
            | "revert"
            | "emit"
            | "return"
            | "delete"
            | "new"
            | "payable"
            | "keccak256"
            | "sha256"
            | "ripemd160"
            | "ecrecover"
            | "addmod"
            | "mulmod"
            | "selfdestruct"
            | "blockhash"
            | "gasleft"
    )
}

fn is_elementary_type(name: &str) -> bool {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/anchors-repo")
    }

    fn index() -> RepoIndex {
        parse_repo(
            &fixture_root(),
            "example/anchor-fixture",
            "0000000000000000000000000000000000000000",
            &IndexOptions::default(),
        )
        .expect("fixture repo parses")
    }

    #[test]
    fn contracts_functions_and_vars_are_found() {
        let idx = index();
        let contract_names: Vec<_> = idx.contracts.iter().map(|c| c.name.as_str()).collect();
        assert!(contract_names.contains(&"Vault"), "{contract_names:?}");
        assert!(contract_names.contains(&"Base"), "{contract_names:?}");
        let fns: Vec<_> = idx.functions.iter().map(|f| f.name.as_str()).collect();
        for want in [
            "deposit",
            "compound",
            "_helper",
            "useHelper",
            "baseIncrement",
        ] {
            assert!(fns.contains(&want), "missing {want} in {fns:?}");
        }
        let vars: Vec<_> = idx
            .state_variables
            .iter()
            .map(|v| v.name.as_str())
            .collect();
        for want in ["totalAssets", "balances", "baseValue", "baseRef"] {
            assert!(vars.contains(&want), "missing {want} in {vars:?}");
        }
    }

    #[test]
    fn call_graph_resolves_internal_and_inherited_calls() {
        let idx = index();
        let compound = idx
            .call_graph
            .get("Vault::compound")
            .unwrap_or_else(|| panic!("no compound entry: {:#?}", idx.call_graph));
        assert_eq!(compound, &vec!["Base::baseIncrement".to_string()]);
        let use_helper = idx
            .call_graph
            .get("Vault::useHelper")
            .unwrap_or_else(|| panic!("no useHelper entry"));
        assert_eq!(use_helper, &vec!["Vault::_helper".to_string()]);
        let peek_base = idx
            .call_graph
            .get("Vault::peekBase")
            .unwrap_or_else(|| panic!("no peekBase entry"));
        assert_eq!(peek_base, &vec!["Base::peek".to_string()]);
        assert!(!idx.call_graph.contains_key("Vault::deposit"));
    }

    #[test]
    fn read_write_sets_are_state_only_and_qualified() {
        let idx = index();
        let deposit = &idx.read_write["Vault::deposit"];
        assert_eq!(
            deposit.reads,
            vec![
                "Vault.balances".to_string(),
                "Vault.totalAssets".to_string()
            ]
        );
        assert_eq!(
            deposit.writes,
            vec![
                "Vault.balances".to_string(),
                "Vault.totalAssets".to_string()
            ]
        );
        let base = &idx.read_write["Base::baseIncrement"];
        assert_eq!(base.reads, vec!["Base.baseValue".to_string()]);
        assert_eq!(base.writes, vec!["Base.baseValue".to_string()]);
        let helper = &idx.read_write["Vault::_helper"];
        assert!(helper.reads.is_empty(), "{helper:?}");
        assert!(helper.writes.is_empty(), "{helper:?}");
        let peek_base = &idx.read_write["Vault::peekBase"];
        assert_eq!(peek_base.reads, vec!["Vault.baseRef".to_string()]);
        assert!(peek_base.writes.is_empty(), "{peek_base:?}");
    }

    #[test]
    fn a_syntax_error_is_a_parse_failure_not_a_silent_skip() {
        let idx = index();
        assert!(
            idx.parse_failures
                .iter()
                .any(|f| f.file == "contracts/Broken.sol"),
            "{:#?}",
            idx.parse_failures
        );
        assert!(!idx.files["contracts/Broken.sol"].parsed);
    }

    #[test]
    fn indexing_is_deterministic() {
        let a = index();
        let b = index();
        assert_eq!(
            serde_json::to_vec(&a).expect("serialise a"),
            serde_json::to_vec(&b).expect("serialise b")
        );
    }
}
