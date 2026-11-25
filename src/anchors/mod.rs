//! Deterministic code anchors.
//!
//! A finding cites code with a file path, a GitHub URL, a backticked
//! identifier, or a function name. This module extracts those citations from
//! the text and resolves them to spans in an audited Solidity repository at a
//! pinned commit. When a citation cannot be resolved it abstains with a reason;
//! it never guesses a span.
//!
//! The repository index is built with [`solar_parse`]: one pass per `.sol`
//! file yields contracts, functions and state variables with byte and line
//! spans, a call graph over internal calls, and per-function state-variable
//! read and write sets. The index is re-implemented here rather than shared
//! with any other tool, so the two repositories stay independent.
//!
//! Everything is offline and deterministic: files are visited in sorted order,
//! every collection is sorted or a `BTreeMap`, and no clock or environment
//! value reaches the output.

#![forbid(unsafe_code)]

mod benchmark;
mod cite;
mod index;
mod resolve;

pub use benchmark::{bench, BenchOptions, BenchReport, CorpusReport, CorpusStatus};
pub use cite::{citation_tokens, citations, cites_code, Citation, CitationKind};
pub use index::{
    parse_repo, FileFact, IndexOptions, IndexSet, ParseFailure, RepoIndex, SymbolInfo,
};
pub use resolve::resolve_text;

use serde::{Deserialize, Serialize};

/// Schema id for one text's anchor set (`priorart anchors resolve --json`).
pub const ANCHORS_SCHEMA: &str = "priorart.anchors/v1";
/// Schema id for the resolution measurement (`priorart anchors bench --json`).
pub const ANCHORS_BENCH_SCHEMA: &str = "priorart.anchors-bench/v1";

/// What a span points at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AnchorKind {
    /// A contract, abstract contract, interface or library declaration.
    Contract,
    /// A function, constructor, fallback, receive or modifier declaration.
    Function,
    /// A state-variable (including constant and immutable) declaration.
    StateVar,
    /// A file, either whole or a cited line range inside it.
    File,
}

/// A resolved span in the audited repository.
///
/// Byte offsets are 0-based and the end is exclusive. Lines are 1-based and
/// both ends are inclusive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Span {
    pub start_byte: u32,
    pub end_byte: u32,
    pub start_line: u32,
    pub end_line: u32,
}

impl Span {
    /// The whole file, for a file that has no discoverable text length.
    pub const ZERO: Span = Span {
        start_byte: 0,
        end_byte: 0,
        start_line: 1,
        end_line: 1,
    };
}

/// One resolved citation.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Anchor {
    /// Repository id, `owner/name` when known, else the directory name given.
    pub repo: String,
    /// The pinned commit the index was built at, or an empty string when the
    /// caller did not pin one.
    pub commit: String,
    /// Forward-slash path relative to the repository root.
    pub file: String,
    pub span: Span,
    pub kind: AnchorKind,
    /// `Contract.member`, `Contract`, or the function name for a free function.
    pub symbol: String,
    /// Resolution confidence in percent (0-100). Fixed values per rule; never a
    /// statistical claim.
    pub confidence: u8,
}

/// Why a citation, or a whole text, did not resolve.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AbstainReason {
    /// The text contains no code citation at all. Not a failure to resolve.
    NoCitation,
    /// No audited source was supplied for the corpus (a blocked corpus).
    RepoUnavailable,
    /// A cited file path matches no file in the index.
    FileMissing,
    /// A cited symbol has no declaration in the index.
    NotFound,
    /// A cited name has more than one candidate and no context selects one.
    Ambiguous,
    /// The cited line range falls outside the indexed file.
    LineOutOfRange,
    /// The cited file exists but `solar` could not parse it.
    ParseFailure,
    /// The checkout is not at the pinned commit.
    CommitMismatch,
    /// The finding file could not be read.
    IoError,
}

impl AbstainReason {
    /// Stable kebab-case id used in JSON and in the report tables.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NoCitation => "no-citation",
            Self::RepoUnavailable => "repo-unavailable",
            Self::FileMissing => "file-missing",
            Self::NotFound => "not-found",
            Self::Ambiguous => "ambiguous",
            Self::LineOutOfRange => "line-out-of-range",
            Self::ParseFailure => "parse-failure",
            Self::CommitMismatch => "commit-mismatch",
            Self::IoError => "io-error",
        }
    }

    /// Priority when one finding has several reasons; the first found wins.
    pub const fn priority(self) -> u8 {
        match self {
            Self::NoCitation => 0,
            Self::IoError => 10,
            Self::RepoUnavailable => 20,
            Self::CommitMismatch => 25,
            Self::ParseFailure => 30,
            Self::FileMissing => 40,
            Self::LineOutOfRange => 50,
            Self::Ambiguous => 60,
            Self::NotFound => 70,
        }
    }
}

/// One abstention: a citation that did not resolve, with the reason.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Abstention {
    /// The raw citation token, or `null` for a whole-text abstention.
    pub citation: Option<String>,
    pub reason: AbstainReason,
    /// Short, deterministic explanation. Never contains a host path.
    pub detail: String,
}

/// The anchor result for one finding or prior-item text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnchorSet {
    pub schema: String,
    /// Whether the text cites code under the definition this module implements.
    pub cites_code: bool,
    /// Every citation found, including unresolved ones.
    pub citations: Vec<Citation>,
    /// Every resolved span, sorted and deduplicated.
    pub anchors: Vec<Anchor>,
    /// Why unresolved citations abstained, sorted and deduplicated.
    pub abstentions: Vec<Abstention>,
}

impl AnchorSet {
    /// The finding resolves when at least one citation becomes an anchor.
    pub fn resolved(&self) -> bool {
        !self.anchors.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn span_and_reason_round_trip_through_json() {
        let span = Span {
            start_byte: 10,
            end_byte: 42,
            start_line: 2,
            end_line: 4,
        };
        let text = serde_json::to_string(&span).expect("serialise span");
        assert_eq!(
            serde_json::from_str::<Span>(&text).expect("parse span"),
            span
        );

        for reason in [
            AbstainReason::NoCitation,
            AbstainReason::RepoUnavailable,
            AbstainReason::FileMissing,
            AbstainReason::NotFound,
            AbstainReason::Ambiguous,
            AbstainReason::LineOutOfRange,
            AbstainReason::ParseFailure,
            AbstainReason::CommitMismatch,
            AbstainReason::IoError,
        ] {
            let text = serde_json::to_string(&reason).expect("serialise reason");
            let back: AbstainReason = serde_json::from_str(&text).expect("parse reason");
            assert_eq!(back, reason);
            assert!(!reason.as_str().is_empty());
        }
        for kind in [
            AnchorKind::Contract,
            AnchorKind::Function,
            AnchorKind::StateVar,
            AnchorKind::File,
        ] {
            let text = serde_json::to_string(&kind).expect("serialise kind");
            assert!(!text.is_empty());
            let back: AnchorKind = serde_json::from_str(&text).expect("parse kind");
            assert_eq!(back, kind);
        }
    }
}
