//! priorart - offline, deterministic prior-art dossiers for audit findings.
//!
//! The tool answers three questions with quoted evidence and page numbers, and
//! never with a verdict:
//!
//! 1. Has this been found?  - ranked prior items sharing at least two strong
//!    code identifiers with the finding.
//! 2. Is it excluded?      - the program's own exclusion clauses, classified
//!    into the 14 clause classes, against the finding's preconditions.
//! 3. Can it be paid?      - the claimed severity against the program's payout
//!    tier shape and floor.
//!
//! Everything is read from files the user saved. The crate has no HTTP client
//! and does not touch the network at runtime.

pub mod check;
pub mod cli;
pub mod dossier;
pub mod error;
pub mod gates;
pub mod init;
pub mod ledger;
#[path = "match/mod.rs"]
pub mod matching;
pub mod model;
pub mod rejections;
pub mod render;
pub mod vocab;

/// The tool name every output and error prefix uses.
pub const TOOL: &str = "priorart";
