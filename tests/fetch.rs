//! `priorart fetch` policy, without the network.
//!
//! URL rejection is checked in the binary. The redirect, cap and file-writing
//! cases live in `src/fetch` and use an injected transport.

#[allow(dead_code)]
mod support;

use support::run;

#[cfg(feature = "fetch")]
#[test]
fn fetch_rejects_http_file_and_localhost_before_any_request() {
    let tmp = support::TempDir::new("fetch-cli");
    let dir = tmp.join("program");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("scope.md"), "keep me\n").unwrap();
    for bad in [
        "http://example.com/program",
        "file:///tmp/page.html",
        "https://127.0.0.1/program",
        "https://localhost/program",
    ] {
        let r = run(&["fetch", bad, "--dir", dir.to_str().unwrap()]);
        assert_eq!(r.code, 2, "{bad}: {}", r.stderr);
        assert!(
            r.stderr.contains("HTTPS")
                || r.stderr.contains("IP-literal")
                || r.stderr.contains("localhost"),
            "{bad}: {}",
            r.stderr
        );
    }
    assert_eq!(
        std::fs::read_to_string(dir.join("scope.md")).unwrap(),
        "keep me\n"
    );
    assert!(!dir.join("fetch.json").exists());
}

#[cfg(not(feature = "fetch"))]
#[test]
fn fetch_reports_that_the_binary_has_no_http_client() {
    let r = run(&["fetch", "https://example.com/program"]);
    assert_eq!(r.code, 2, "{}", r.stderr);
    assert!(
        r.stderr.contains("no HTTP client") || r.stderr.contains("fetch feature"),
        "{}",
        r.stderr
    );
}
