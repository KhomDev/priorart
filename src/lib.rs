//! viability-gate - offline, deterministic pre-submission checking for audit
//! findings.
//!
//! The crate is a library plus two identical binaries. Both exist because `vg`
//! is also the name of an unrelated bioinformatics tool: a researcher whose PATH
//! already has that one can install and call `viability-gate` instead.

pub mod bench;
pub mod claims;
pub mod cli;
pub mod engine;
pub mod import;
pub mod index;
pub mod init;
pub mod model;
pub mod render;
