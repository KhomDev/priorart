//! One error type, one exit code.
//!
//! Every failure that is not "analysis produced" is a usage, IO or parse
//! error, and all of them exit 2. Exit 1 is reserved for the two opt-in flags
//! that turn evidence into a failure: `--fail-on-excluded` and
//! `--fail-on-unpayable`. That keeps the contract narrow enough to script
//! against.

/// A recoverable failure: usage, IO or parse. Rendered to stderr and exits 2.
pub type Error = String;

pub type Result<T> = std::result::Result<T, Error>;

/// The exit code for a usage, IO or parse failure.
pub const USAGE_EXIT: u8 = 2;

/// The exit code for `--fail-on-excluded` / `--fail-on-unpayable`.
pub const EVIDENCE_EXIT: u8 = 1;
