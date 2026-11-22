//! Input trust boundaries: only regular files are read, and a finding, a
//! dossier or a program folder cannot be used to make the tool read without
//! bound.
//!
//! OS-specific cases skip cleanly: creating a symlink needs a privilege on
//! Windows, and a FIFO needs a Unix system with `mkfifo`.

#[allow(dead_code)]
mod support;

#[cfg(unix)]
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
#[cfg(unix)]
use std::process::Stdio;
#[cfg(unix)]
use std::time::{Duration, Instant};
use support::{run, TempDir};

fn program_with_scope(tmp: &TempDir) -> PathBuf {
    tmp.write(
        "program/scope.md",
        "# Scope\n\n## In scope\n\n- src/Vault.sol\n\n## Out of scope\n\n- Attacks requiring a privileged role.\n",
    );
    tmp.write(
        "program/known-issues.md",
        "# Known issues\n\n- Nothing acknowledged yet.\n",
    );
    tmp.join("program")
}

fn build_dossier(tmp: &TempDir) -> PathBuf {
    let dir = program_with_scope(tmp);
    let out = tmp.join("program.dossier");
    let r = run(&[
        "build",
        dir.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
    ]);
    assert_eq!(r.code, 0, "build failed: {} {}", r.stdout, r.stderr);
    out
}

/// Create a file symlink, or report that this account cannot. Tests skip when
/// the host refuses the call (Windows without the symlink privilege).
fn try_symlink_file(src: &Path, dst: &Path) -> bool {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(src, dst).is_ok()
    }
    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_file(src, dst).is_ok()
    }
}

fn try_symlink_dir(src: &Path, dst: &Path) -> bool {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(src, dst).is_ok()
    }
    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_dir(src, dst).is_ok()
    }
}

fn sparse_file(path: &Path, len: u64) {
    let file = std::fs::File::create(path).expect("create sparse file");
    file.set_len(len).expect("set the sparse file length");
}

#[cfg(unix)]
fn run_with_deadline(args: &[&str], limit: Duration) -> (support::Run, Duration) {
    let start = Instant::now();
    let mut child = Command::new(env!("CARGO_BIN_EXE_priorart"))
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn priorart");
    loop {
        match child.try_wait().expect("poll child") {
            Some(status) => {
                let mut stdout = String::new();
                let mut stderr = String::new();
                if let Some(mut s) = child.stdout.take() {
                    let _ = s.read_to_string(&mut stdout);
                }
                if let Some(mut s) = child.stderr.take() {
                    let _ = s.read_to_string(&mut stderr);
                }
                return (
                    Run {
                        code: status.code().unwrap_or(-1),
                        stdout,
                        stderr,
                    },
                    start.elapsed(),
                );
            }
            None => {
                if start.elapsed() > limit {
                    let _ = child.kill();
                    let _ = child.wait();
                    panic!("priorart {args:?} blocked for {limit:?}; it may have opened a FIFO");
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    }
}

// ------------------------------------------------------- regular files only

#[test]
fn a_symlinked_program_page_is_named_skipped_and_never_read() {
    let tmp = TempDir::new("symlink-scope");
    tmp.write(
        "outside.md",
        "# Outside\n\n## Rewards\n\n- Critical: 999,999 USD\n",
    );
    tmp.write(
        "program/audits/review.md",
        "# Review\n\n## Finding 1\n\nThe `LoanBook.close` path leaves `FeeLedger.distributeFees` unsettled.\n",
    );
    if !try_symlink_file(&tmp.join("outside.md"), &tmp.join("program/scope.md")) {
        eprintln!("skipping: this account cannot create a file symlink");
        return;
    }
    let out = tmp.join("out.dossier");
    let r = run(&[
        "build",
        tmp.join("program").to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
    ]);
    assert_eq!(r.code, 0, "{} {}", r.stdout, r.stderr);
    let combined = format!("{}{}", r.stdout, r.stderr);
    assert!(
        combined.contains("scope.md is a symlink"),
        "the skipped symlink must be named: {combined}"
    );
    assert!(combined.contains("skipped"), "{combined}");
    let dossier = std::fs::read_to_string(&out).unwrap();
    assert!(
        !dossier.contains("999,999"),
        "bytes behind a symlink reached the dossier"
    );
    assert!(
        dossier.contains("audits/review.md"),
        "the regular audit should still be indexed"
    );
}

#[test]
fn a_folder_whose_only_page_is_a_symlink_is_a_named_usage_error() {
    let tmp = TempDir::new("symlink-only");
    tmp.write("outside.md", "# Outside\n\n## In scope\n\n- src/Evil.sol\n");
    if !try_symlink_file(&tmp.join("outside.md"), &tmp.join("program/scope.md")) {
        eprintln!("skipping: this account cannot create a file symlink");
        return;
    }
    let r = run(&["build", tmp.join("program").to_str().unwrap()]);
    assert_eq!(r.code, 2, "{} {}", r.stdout, r.stderr);
    assert!(r.stderr.contains("no inputs found"), "{}", r.stderr);
    assert!(
        r.stderr.contains("scope.md is a symlink"),
        "the refusal must name the symlink: {}",
        r.stderr
    );
}

#[test]
fn a_symlinked_program_yaml_is_refused_by_name() {
    let tmp = TempDir::new("symlink-yaml");
    tmp.write(
        "program/scope.md",
        "# Scope\n\n## In scope\n\n- src/Vault.sol\n",
    );
    tmp.write("outside.yaml", "name: Outside\n");
    if !try_symlink_file(&tmp.join("outside.yaml"), &tmp.join("program/program.yaml")) {
        eprintln!("skipping: this account cannot create a file symlink");
        return;
    }
    let r = run(&["build", tmp.join("program").to_str().unwrap()]);
    assert_eq!(r.code, 2, "{} {}", r.stdout, r.stderr);
    assert!(r.stderr.contains("program.yaml"), "{}", r.stderr);
    assert!(r.stderr.contains("symlink"), "{}", r.stderr);
    assert!(r.stderr.contains("regular file"), "{}", r.stderr);
}

#[test]
fn a_symlinked_audits_directory_is_named_and_not_followed() {
    let tmp = TempDir::new("symlink-audits");
    tmp.write(
        "program/scope.md",
        "# Scope\n\n## In scope\n\n- src/Vault.sol\n",
    );
    tmp.write(
        "outside/review.md",
        "# Review\n\n## Finding 1\n\nThe `OutsideContract.close` path leaves `OutsideLedger.distribute` unsettled.\n",
    );
    if !try_symlink_dir(&tmp.join("outside"), &tmp.join("program/audits")) {
        eprintln!("skipping: this account cannot create a directory symlink");
        return;
    }
    let out = tmp.join("out.dossier");
    let r = run(&[
        "build",
        tmp.join("program").to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
    ]);
    assert_eq!(r.code, 0, "{} {}", r.stdout, r.stderr);
    let combined = format!("{}{}", r.stdout, r.stderr);
    assert!(
        combined.contains("audits is a symlink"),
        "the skipped audits directory must be named: {combined}"
    );
    let dossier = std::fs::read_to_string(&out).unwrap();
    assert!(
        !dossier.contains("OutsideContract"),
        "bytes behind a symlinked audits/ reached the dossier"
    );
}

#[test]
fn a_symlinked_finding_is_refused_by_name() {
    let tmp = TempDir::new("symlink-finding");
    let dossier = build_dossier(&tmp);
    tmp.write("real.md", "# Finding\n\n`Vault.close` needs an operator.\n");
    if !try_symlink_file(&tmp.join("real.md"), &tmp.join("finding.md")) {
        eprintln!("skipping: this account cannot create a file symlink");
        return;
    }
    let r = run(&[
        "check",
        tmp.join("finding.md").to_str().unwrap(),
        "--dossier",
        dossier.to_str().unwrap(),
    ]);
    assert_eq!(r.code, 2, "{} {}", r.stdout, r.stderr);
    assert!(r.stderr.contains("finding.md"), "{}", r.stderr);
    assert!(r.stderr.contains("symlink"), "{}", r.stderr);
}

#[test]
fn a_symlinked_dossier_beside_the_finding_is_refused_not_read() {
    let tmp = TempDir::new("symlink-discover");
    tmp.write("finding.md", "# Finding\n\n`Vault.close`.\n");
    // The target is outside the finding's directory, so the only .dossier
    // beside the finding is the symlink itself.
    tmp.write(
        "target/real.dossier",
        "{\"schema\":\"priorart.dossier/v1\"}",
    );
    if !try_symlink_file(&tmp.join("target/real.dossier"), &tmp.join("only.dossier")) {
        eprintln!("skipping: this account cannot create a file symlink");
        return;
    }
    let r = run(&["check", tmp.join("finding.md").to_str().unwrap()]);
    assert_eq!(r.code, 2, "{} {}", r.stdout, r.stderr);
    assert!(
        r.stderr.contains("only.dossier") && r.stderr.contains("not a regular file"),
        "{}",
        r.stderr
    );
}

#[cfg(unix)]
#[test]
fn a_fifo_program_page_is_named_and_never_opened() {
    let tmp = TempDir::new("fifo");
    std::fs::create_dir_all(tmp.join("program")).unwrap();
    let fifo = tmp.join("program/scope.md");
    let made = Command::new("mkfifo")
        .arg(&fifo)
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !made {
        eprintln!("skipping: mkfifo is not available on this system");
        return;
    }
    let (r, elapsed) = run_with_deadline(
        &["build", tmp.join("program").to_str().unwrap()],
        Duration::from_secs(20),
    );
    assert_eq!(r.code, 2, "{} {}", r.stdout, r.stderr);
    assert!(r.stderr.contains("scope.md"), "{}", r.stderr);
    assert!(
        r.stderr.contains("special file"),
        "the refusal must name the file type: {}",
        r.stderr
    );
    assert!(
        elapsed < Duration::from_secs(10),
        "the build blocked on the FIFO for {elapsed:?}"
    );
}

// ------------------------------------------------------------- input caps

#[test]
fn an_oversized_sparse_finding_is_refused_before_it_is_read() {
    let tmp = TempDir::new("huge-finding");
    let dossier = build_dossier(&tmp);
    let huge = tmp.join("huge.md");
    sparse_file(&huge, priorart::check::MAX_FINDING_BYTES + 1);
    let r = run(&[
        "check",
        huge.to_str().unwrap(),
        "--dossier",
        dossier.to_str().unwrap(),
    ]);
    assert_eq!(r.code, 2, "{} {}", r.stdout, r.stderr);
    assert!(r.stderr.contains("huge.md"), "{}", r.stderr);
    assert!(
        r.stderr.contains("up to 16 MiB"),
        "the error must name the finding limit: {}",
        r.stderr
    );
}

#[test]
fn an_oversized_sparse_dossier_is_refused_before_it_is_read() {
    let tmp = TempDir::new("huge-dossier");
    let finding = tmp.write(
        "finding.md",
        "# Finding\n\n`Vault.close` needs an operator.\n",
    );
    let huge = tmp.join("huge.dossier");
    sparse_file(&huge, priorart::dossier::load::MAX_DOSSIER_BYTES + 1);
    let r = run(&[
        "check",
        finding.to_str().unwrap(),
        "--dossier",
        huge.to_str().unwrap(),
    ]);
    assert_eq!(r.code, 2, "{} {}", r.stdout, r.stderr);
    assert!(r.stderr.contains("huge.dossier"), "{}", r.stderr);
    assert!(
        r.stderr.contains("up to 64 MiB"),
        "the error must name the dossier limit: {}",
        r.stderr
    );
}

// ------------------------------------------------------------- output paths

#[test]
fn the_build_summary_prints_the_dossier_path_with_forward_slashes() {
    let tmp = TempDir::new("slashes");
    let dir = program_with_scope(&tmp);
    std::fs::create_dir_all(tmp.join("out")).unwrap();
    let out = tmp.join("out/one.dossier");
    let r = run(&[
        "build",
        dir.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
    ]);
    assert_eq!(r.code, 0, "{} {}", r.stdout, r.stderr);
    assert!(
        r.stdout.contains("out/one.dossier"),
        "the summary must use forward slashes: {}",
        r.stdout
    );
    assert!(
        !r.stdout.contains("out\\one.dossier"),
        "a Windows backslash reached the summary: {}",
        r.stdout
    );
}

// --------------------------------------------------------- pdftotext policy

/// A compressed-stream PDF: the in-process scrape finds no text, so only a
/// pdftotext call can read it.
fn compressed_pdf() -> Vec<u8> {
    let mut pdf = b"%PDF-1.4\n1 0 obj\n<< /Length 8 /Filter /FlateDecode >>\nstream\n".to_vec();
    pdf.extend_from_slice(&[0x78, 0x9c, 0x01, 0x02, 0x00, 0xff, 0xfe, 0x01, 0x02]);
    pdf.extend_from_slice(b"\nendstream\nendobj\ntrailer\n<<>>\n%%EOF\n");
    pdf
}

fn run_build_with_path(
    program: &Path,
    out: &Path,
    bin_dir: &Path,
    extra: &[&str],
) -> std::process::Output {
    let mut args: Vec<&str> = vec![
        "build",
        program.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
    ];
    args.extend_from_slice(extra);
    Command::new(env!("CARGO_BIN_EXE_priorart"))
        .args(&args)
        .env("PATH", support::path_with_first(bin_dir))
        .output()
        .expect("run priorart")
}

#[test]
fn no_pdftotext_never_runs_poppler_and_the_control_proves_it() {
    let tmp = TempDir::new("no-pdftotext");
    tmp.write(
        "program/scope.md",
        "# Scope\n\n## In scope\n\n- src/Vault.sol\n",
    );
    tmp.write_bytes("program/audits/prior.pdf", &compressed_pdf());
    let bin_dir = tmp.join("bin");
    let marker = tmp.join("pdftotext-ran");
    let marker_for_shell = marker.to_string_lossy().replace('\\', "/");
    support::fake_pdftotext(
        &bin_dir,
        &format!("printf 'text from the fake pdftotext\\n'; : > '{marker_for_shell}'"),
    );
    let program = tmp.join("program");

    // Control: without the flag, the fallback runs. Without this run the
    // refusal below would prove nothing.
    let control_out = tmp.join("control.dossier");
    let control = run_build_with_path(&program, &control_out, &bin_dir, &[]);
    assert_eq!(
        control.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&control.stderr)
    );
    assert!(
        marker.is_file(),
        "the control must run the fake pdftotext; the flag test would be vacuous"
    );
    std::fs::remove_file(&marker).unwrap();

    let guarded_out = tmp.join("guarded.dossier");
    let guarded = run_build_with_path(&program, &guarded_out, &bin_dir, &["--no-pdftotext"]);
    let stdout = String::from_utf8_lossy(&guarded.stdout);
    let stderr = String::from_utf8_lossy(&guarded.stderr);
    let combined = format!("{stdout}{stderr}");
    assert_eq!(guarded.status.code(), Some(0), "{combined}");
    assert!(
        combined.contains("--no-pdftotext"),
        "the skipped PDF must say why: {combined}"
    );
    assert!(
        !marker.is_file(),
        "--no-pdftotext still ran the fake pdftotext: {combined}"
    );
    // The build still succeeds and the regular page is indexed.
    assert!(combined.contains("source(s) indexed"), "{combined}");
}
