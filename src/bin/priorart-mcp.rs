#![forbid(unsafe_code)]
//! MCP stdio server (`io.github.KhomDev/priorart`). See `src/mcp.rs` for the
//! tools and the transport.
//!
//! priorart is `PRIORART_BIN` when set, else a sibling of this
//! executable, else `priorart` on `PATH`. The operator, not the client,
//! sets two limits: `PRIORART_MCP_ROOT` (every path argument must resolve
//! inside it) and `PRIORART_MCP_TIMEOUT_SECS` (default 120, 1 to 3600).

use priorart::mcp::{self, Policy, Ran};
use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

const HELP: &str =
    "priorart-mcp: the priorart MCP server (MCP Registry: io.github.KhomDev/priorart)

Speaks MCP over stdio: one JSON-RPC message per line on stdin and stdout.
Content-Length framed requests are also read and answered in kind.
Tools (read-only): check, candidates, anchors_resolve, anchors_index,
taxonomy_classify, ledger_validate, vocab_list, vocab_show, explain, rules,
rejections_summary. fetch, build, init and anchors bench are not offered.

Environment:
  PRIORART_BIN               the priorart binary (default: next to this binary, then PATH)
  PRIORART_MCP_ROOT          refuse any path argument that resolves outside this directory
  PRIORART_MCP_TIMEOUT_SECS  kill a priorart run after this many seconds (default 120)

Configure it in your MCP client with the command `priorart-mcp`.";

fn main() {
    if let Some(arg) = std::env::args().nth(1) {
        match arg.as_str() {
            "--version" | "-V" => {
                println!("priorart-mcp {}", env!("CARGO_PKG_VERSION"));
                return;
            }
            "--help" | "-h" => {
                println!("{HELP}");
                return;
            }
            other => {
                eprintln!("priorart-mcp: unknown argument `{other}`; see --help");
                std::process::exit(2);
            }
        }
    }
    let policy = match std::env::var_os("PRIORART_MCP_ROOT") {
        Some(root) if !root.is_empty() => match Policy::with_root(&PathBuf::from(root)) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("priorart-mcp: {e}");
                std::process::exit(2);
            }
        },
        _ => Policy::default(),
    };
    let timeout = match timeout_from_env() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("priorart-mcp: {e}");
            std::process::exit(2);
        }
    };
    let mut stdin = std::io::stdin().lock();
    let mut stdout = std::io::stdout().lock();
    let mut run = |args: &[String]| -> Result<Ran, String> {
        let bin = resolve();
        mcp::run_bounded(
            Command::new(&bin).args(args),
            timeout,
            mcp::MAX_TOOL_OUTPUT_BYTES,
        )
    };
    // Every framing error stops the server. An oversized frame is answered
    // with a JSON-RPC error first, because the client gets a reason for the
    // stop; the unread body is not trusted enough to resynchronise past it.
    if let Err(e) = mcp::serve(
        &mut stdin as &mut dyn BufRead,
        &mut stdout as &mut dyn Write,
        &policy,
        &mut run,
    ) {
        let _ = writeln!(std::io::stderr(), "priorart-mcp: {e}");
        std::process::exit(2);
    }
}

fn timeout_from_env() -> Result<Duration, String> {
    let Some(raw) = std::env::var_os("PRIORART_MCP_TIMEOUT_SECS") else {
        return Ok(mcp::DEFAULT_TIMEOUT);
    };
    let raw = raw.to_string_lossy();
    match raw.trim().parse::<u64>() {
        Ok(s) if (1..=3600).contains(&s) => Ok(Duration::from_secs(s)),
        _ => Err(format!(
            "PRIORART_MCP_TIMEOUT_SECS must be a whole number of seconds from 1 to 3600, not `{raw}`"
        )),
    }
}

fn resolve() -> PathBuf {
    if let Some(p) = std::env::var_os("PRIORART_BIN") {
        return PathBuf::from(p);
    }
    if let Some(dir) = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(PathBuf::from))
    {
        for name in ["priorart", "priorart.exe"] {
            let sibling = dir.join(name);
            if sibling.is_file() {
                return sibling;
            }
        }
    }
    PathBuf::from("priorart")
}
