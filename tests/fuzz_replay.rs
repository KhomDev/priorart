//! Replays the fuzz seeds through every fuzz target, on every `cargo test`.
//!
//! The targets are the same functions the cargo-fuzz binaries run
//! (fuzz/src/targets.rs), compiled here by path, so this needs no nightly and
//! no libfuzzer. It proves the seeds and a few hostile edge inputs do not
//! panic. It does not explore new inputs: that is what the weekly fuzz job is for.

#[allow(dead_code)]
#[path = "../fuzz/src/targets.rs"]
mod targets;

use std::path::PathBuf;

type Target = fn(&[u8]);

const TARGETS: [(&str, Target); 7] = [
    ("markdown_import", targets::markdown_import),
    ("pdf_scrape", targets::pdf_scrape),
    ("program_yaml", targets::program_yaml),
    ("known_issues_yaml", targets::known_issues_yaml),
    ("ledger_yaml", targets::ledger_yaml),
    ("dossier_json", targets::dossier_json),
    ("finding_profile", targets::finding_profile),
];

fn corpus_dir(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fuzz")
        .join("corpus")
        .join(name)
}

#[test]
fn every_seed_replays_through_its_target_without_panicking() {
    for (name, target) in TARGETS {
        let dir = corpus_dir(name);
        let mut count = 0;
        for entry in std::fs::read_dir(&dir)
            .unwrap_or_else(|e| panic!("cannot read seeds in {}: {e}", dir.display()))
        {
            let path = entry.expect("seed entry").path();
            if !path.is_file() {
                continue;
            }
            let bytes = std::fs::read(&path).expect("read seed");
            target(&bytes);
            count += 1;
        }
        assert!(count > 0, "{name}: no seeds in {}", dir.display());
    }
}

#[test]
fn every_seed_target_pair_survives_hostile_edge_inputs() {
    let edges: [&[u8]; 10] = [
        b"",
        b"\xef\xbb\xbf",
        b"\xff\xfe\x00\x00 not utf-8 \xc3\x28",
        b"---\n---\n---\n",
        b"[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[",
        b"{\"schema\": \"priorart.dossier/v1\", \"items\": [",
        b"%PDF-1.4 (\\(\\\\\\)) Tj stream endstream",
        "# Finding\n\n`` ` ` ` \u{202e}LoanBook.close\u{202c} \x1b[2J\n".as_bytes(),
        b"*a: &a [*a, *a]\nb: *a\n",
        &[0u8; 4096],
    ];
    for (name, target) in TARGETS {
        for (i, edge) in edges.iter().enumerate() {
            let caught = std::panic::catch_unwind(|| target(edge));
            assert!(
                caught.is_ok(),
                "{name} panicked on edge input {i}: {:?}",
                String::from_utf8_lossy(edge)
            );
        }
    }
}
