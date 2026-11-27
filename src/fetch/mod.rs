//! `priorart fetch`: one explicit HTTPS GET per URL, written into the program
//! folder `build` already reads.
//!
//! This module does not open a socket. The HTTP client lives in
//! [`ureq_client`] and is compiled only with the `fetch` feature. Tests pass a
//! [`Transport`] so they never touch the network.

use crate::dossier::sha256;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(feature = "fetch")]
mod ureq_client;

pub mod url;

use self::url::{MAX_BODY_BYTES, MAX_REDIRECTS};

#[cfg(feature = "fetch")]
pub use ureq_client::UreqTransport;

/// Schema id of `fetch.json`. It records what was saved. `build` does not
/// index it; it only reads a page's final URL as the base for relative links.
pub const FETCH_SCHEMA: &str = "priorart.fetch/v1";

/// One GET, as the policy loop sees it. The implementor does not follow
/// redirects and does not send cookies or an Authorization header.
pub trait Transport {
    fn get(&self, url: &str) -> Result<HttpResponse, String>;
}

/// A single response. The policy loop refuses a body over the cap.
pub struct HttpResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

/// What one successful fetch saved.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct PageRecord {
    pub url: String,
    pub final_url: String,
    pub status: u16,
    pub retrieved: String,
    pub bytes: u64,
    pub sha256: String,
    pub path: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
struct FetchFile {
    schema: String,
    tool: String,
    version: String,
    pages: Vec<PageRecord>,
}

/// The lines `priorart fetch` prints.
#[derive(Debug)]
pub struct FetchReport {
    pub dir: PathBuf,
    pub pages: Vec<PageRecord>,
}

impl FetchReport {
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "priorart {} - fetched {} page(s) into {}\n",
            env!("CARGO_PKG_VERSION"),
            self.pages.len(),
            self.dir.display()
        ));
        for page in &self.pages {
            let redirect = if page.final_url != page.url {
                format!("  {}", page.final_url)
            } else {
                String::new()
            };
            out.push_str(&format!(
                "  {}  {}  {} bytes  sha256 {}  {}{redirect}\n",
                page.path,
                page.status,
                page.bytes,
                &page.sha256[..12.min(page.sha256.len())],
                page.url
            ));
        }
        out.push_str("  wrote fetch.json\n");
        out.push_str(&format!("Next: priorart build {}\n", self.dir.display()));
        crate::render::text::sanitize(&out)
    }
}

/// Fetch `urls` into `dir/pages/` and write `fetch.json`.
///
/// `scope.md` is never written. An existing page file is left untouched unless
/// `force` is set. Nothing is written if any URL fails.
pub fn run(
    dir: &Path,
    urls: &[String],
    force: bool,
    transport: &dyn Transport,
) -> Result<FetchReport, String> {
    run_capped(dir, urls, force, transport, MAX_BODY_BYTES)
}

fn run_capped(
    dir: &Path,
    urls: &[String],
    force: bool,
    transport: &dyn Transport,
    max_body: usize,
) -> Result<FetchReport, String> {
    if urls.is_empty() {
        return Err("pass at least one HTTPS URL. priorart fetch does not crawl.".into());
    }
    if !dir.exists() {
        fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    }
    if !dir.is_dir() {
        return Err(format!(
            "{} is not a directory. priorart fetch writes pages into a program folder.",
            dir.display()
        ));
    }
    let mut file = read_fetch_file(dir)?;
    let mut planned: Vec<(String, PathBuf, String)> = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for url in urls {
        if !seen.insert(url.as_str()) {
            return Err(format!(
                "{url} is listed more than once. priorart fetch requests each URL once."
            ));
        }
        url::validate_fetch_url(url)?;
        let rel = path_for_url(&file, url, &planned);
        check_page_path(&rel)?;
        let dest = dir.join(&rel);
        refuse_scope_md(&dest)?;
        if dest.exists() && !force {
            return Err(format!(
                "{} already exists. priorart fetch does not overwrite a saved page without --force, and it never overwrites scope.md.",
                dest.display()
            ));
        }
        planned.push((url.clone(), dest, rel));
    }

    let mut saved: Vec<(PathBuf, Vec<u8>, PageRecord)> = Vec::new();
    for (url, dest, rel) in &planned {
        let got = fetch_one(url, transport, max_body)?;
        let record = PageRecord {
            url: url.clone(),
            final_url: got.final_url,
            status: got.status,
            retrieved: utc_now(),
            bytes: got.body.len() as u64,
            sha256: sha256::hex(&got.body),
            path: rel.clone(),
        };
        saved.push((dest.clone(), got.body, record));
    }

    let pages_dir = dir.join("pages");
    fs::create_dir_all(&pages_dir)
        .map_err(|e| format!("cannot create {}: {e}", pages_dir.display()))?;
    for (dest, body, record) in &saved {
        refuse_scope_md(dest)?;
        write_atomic(dest, body)?;
        upsert_page(&mut file, record.clone());
    }
    file.schema = FETCH_SCHEMA.to_string();
    file.tool = crate::TOOL.to_string();
    file.version = env!("CARGO_PKG_VERSION").to_string();
    file.pages.sort_by(|a, b| a.path.cmp(&b.path));
    let mut pretty = serde_json::to_vec_pretty(&file)
        .map_err(|e| format!("cannot serialise fetch.json: {e}"))?;
    pretty.push(b'\n');
    write_atomic(&dir.join("fetch.json"), &pretty)?;

    Ok(FetchReport {
        dir: dir.to_path_buf(),
        pages: saved.into_iter().map(|(_, _, record)| record).collect(),
    })
}

struct Got {
    final_url: String,
    status: u16,
    body: Vec<u8>,
}

fn fetch_one(start: &str, transport: &dyn Transport, max_body: usize) -> Result<Got, String> {
    let mut current = start.to_string();
    for hop in 0..=MAX_REDIRECTS {
        url::validate_fetch_url(&current)?;
        let response = transport
            .get(&current)
            .map_err(|e| format!("cannot fetch {current}: {e}"))?;
        if is_redirect(response.status) {
            if hop == MAX_REDIRECTS {
                return Err(format!(
                    "{start} redirected more than {MAX_REDIRECTS} times. priorart fetch stops there and saves nothing."
                ));
            }
            let loc = header(&response.headers, "location").ok_or_else(|| {
                format!(
                    "{current} returned {}, which has no Location",
                    response.status
                )
            })?;
            let next = url::resolve_link(Some(&current), loc).ok_or_else(|| {
                format!(
                    "{current} redirected to {loc:?}, which is not a URL priorart fetch follows"
                )
            })?;
            url::validate_fetch_url(&next).map_err(|e| {
                format!(
                    "{current} redirected to {next}, which priorart fetch will not request: {e}"
                )
            })?;
            current = next;
            continue;
        }
        if !(200..300).contains(&response.status) {
            return Err(format!(
                "{current} returned HTTP {}. priorart fetch saves a 2xx body only.",
                response.status
            ));
        }
        if response.body.len() > max_body {
            return Err(format!(
                "{current} is {} bytes, over the {} byte cap. priorart fetch saves nothing.",
                response.body.len(),
                max_body
            ));
        }
        return Ok(Got {
            final_url: current,
            status: response.status,
            body: response.body,
        });
    }
    Err(format!(
        "{start} redirected more than {MAX_REDIRECTS} times."
    ))
}

fn is_redirect(status: u16) -> bool {
    matches!(status, 301 | 302 | 303 | 307 | 308)
}

fn header<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.as_str())
}

fn path_for_url(file: &FetchFile, url: &str, planned: &[(String, PathBuf, String)]) -> String {
    if let Some(existing) = file.pages.iter().find(|p| p.url == url) {
        return existing.path.clone();
    }
    let mut n = 1u32;
    loop {
        let rel = format!("pages/{n:03}.html");
        let taken = file.pages.iter().any(|p| p.path == rel)
            || planned.iter().any(|(_, _, path)| path == &rel);
        if !taken {
            return rel;
        }
        n += 1;
    }
}

fn upsert_page(file: &mut FetchFile, record: PageRecord) {
    if let Some(slot) = file.pages.iter_mut().find(|p| p.url == record.url) {
        *slot = record;
    } else {
        file.pages.push(record);
    }
}

fn read_fetch_file(dir: &Path) -> Result<FetchFile, String> {
    let path = dir.join("fetch.json");
    if !path.exists() {
        return Ok(FetchFile {
            schema: FETCH_SCHEMA.to_string(),
            tool: crate::TOOL.to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            pages: Vec::new(),
        });
    }
    let bytes = fs::read(&path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let file: FetchFile = serde_json::from_slice(&bytes).map_err(|e| {
        format!(
            "{} is not a {} file: {e}. Move it aside or fix it; priorart fetch will not overwrite a file it cannot read.",
            path.display(),
            FETCH_SCHEMA
        )
    })?;
    if file.schema != FETCH_SCHEMA {
        return Err(format!(
            "{} has schema {:?}, not {FETCH_SCHEMA}. priorart fetch will not overwrite it.",
            path.display(),
            file.schema
        ));
    }
    Ok(file)
}

/// A page path from `fetch.json` must stay inside `pages/`. A hand-edited or
/// poisoned `fetch.json` could otherwise point a URL at `../x` or an absolute
/// path, and `fetch` would write there.
fn check_page_path(rel: &str) -> Result<(), String> {
    let ok = rel.starts_with("pages/")
        && rel.len() > "pages/".len()
        && !rel.contains('\\')
        && !rel.contains(':')
        && rel
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..");
    if ok {
        Ok(())
    } else {
        Err(format!(
            "fetch.json maps a URL to {rel:?}, which is not a file under pages/. priorart fetch writes nowhere else; fix or move fetch.json aside."
        ))
    }
}

fn refuse_scope_md(path: &Path) -> Result<(), String> {
    let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
    if name.eq_ignore_ascii_case("scope.md") {
        return Err(
            "priorart fetch never overwrites scope.md. That file is yours; fetch writes pages/*.html."
                .into(),
        );
    }
    Ok(())
}

fn write_atomic(path: &Path, body: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
    }
    let tmp = path.with_extension("tmp");
    {
        let mut file =
            fs::File::create(&tmp).map_err(|e| format!("cannot write {}: {e}", tmp.display()))?;
        file.write_all(body)
            .map_err(|e| format!("cannot write {}: {e}", tmp.display()))?;
    }
    fs::rename(&tmp, path).map_err(|e| format!("cannot rename into {}: {e}", path.display()))?;
    Ok(())
}

fn utc_now() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format_utc(secs)
}

fn format_utc(secs: u64) -> String {
    let days = secs / 86_400;
    let tod = secs % 86_400;
    let hour = tod / 3600;
    let min = (tod % 3600) / 60;
    let sec = tod % 60;
    let (y, m, d) = civil_from_days(days as i64);
    format!("{y:04}-{m:02}-{d:02}T{hour:02}:{min:02}:{sec:02}Z")
}

/// Howard Hinnant's `civil_from_days`, counted from the Unix epoch.
fn civil_from_days(days: i64) -> (i32, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y as i32, m as u32, d as u32)
}

#[cfg(feature = "fetch")]
pub fn ureq_transport() -> Result<UreqTransport, String> {
    Ok(UreqTransport::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::sync::Mutex;

    struct Script {
        hits: Mutex<Vec<String>>,
        responses: BTreeMap<String, HttpResponse>,
    }

    impl Transport for Script {
        fn get(&self, url: &str) -> Result<HttpResponse, String> {
            self.hits.lock().unwrap().push(url.to_string());
            self.responses
                .get(url)
                .map(|r| HttpResponse {
                    status: r.status,
                    headers: r.headers.clone(),
                    body: r.body.clone(),
                })
                .ok_or_else(|| format!("no scripted response for {url}"))
        }
    }

    fn scratch(label: &str) -> PathBuf {
        static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let dir =
            std::env::temp_dir().join(format!("priorart-fetch-{label}-{}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn http_file_ip_and_localhost_never_call_the_transport() {
        let dir = scratch("policy");
        let script = Script {
            hits: Mutex::new(Vec::new()),
            responses: BTreeMap::new(),
        };
        for bad in [
            "http://example.com/a",
            "file:///tmp/x",
            "https://127.0.0.1/a",
            "https://[::1]/a",
            "https://localhost/a",
        ] {
            let err = run(&dir, &[bad.into()], false, &script).expect_err(bad);
            assert!(
                err.contains("HTTPS") || err.contains("IP-literal") || err.contains("localhost"),
                "{bad}: {err}"
            );
        }
        assert!(
            script.hits.lock().unwrap().is_empty(),
            "a rejected URL was requested"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_redirect_to_http_is_refused_and_five_https_hops_are_the_limit() {
        let dir = scratch("redir");
        let mut responses = BTreeMap::new();
        responses.insert(
            "https://example.com/a".into(),
            HttpResponse {
                status: 302,
                headers: vec![("Location".into(), "http://example.com/b".into())],
                body: Vec::new(),
            },
        );
        let script = Script {
            hits: Mutex::new(Vec::new()),
            responses,
        };
        let err = run(&dir, &["https://example.com/a".into()], false, &script).unwrap_err();
        assert!(err.contains("http://example.com/b"), "{err}");
        assert!(!dir.join("fetch.json").exists());

        let mut responses = BTreeMap::new();
        for i in 0..=MAX_REDIRECTS {
            let from = if i == 0 {
                "https://example.com/start".to_string()
            } else {
                format!("https://example.com/r{i}")
            };
            let to = format!("https://example.com/r{}", i + 1);
            responses.insert(
                from,
                HttpResponse {
                    status: 302,
                    headers: vec![("Location".into(), to)],
                    body: Vec::new(),
                },
            );
        }
        let script = Script {
            hits: Mutex::new(Vec::new()),
            responses,
        };
        let err = run(&dir, &["https://example.com/start".into()], false, &script).unwrap_err();
        assert!(err.contains("more than 5"), "{err}");
        assert_eq!(script.hits.lock().unwrap().len(), MAX_REDIRECTS + 1);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn one_https_redirect_is_saved_and_scope_md_is_never_overwritten() {
        let dir = scratch("save");
        fs::write(dir.join("scope.md"), "user edited\n").unwrap();
        let mut responses = BTreeMap::new();
        responses.insert(
            "https://example.com/info".into(),
            HttpResponse {
                status: 302,
                headers: vec![("Location".into(), "https://example.com/info/".into())],
                body: Vec::new(),
            },
        );
        responses.insert(
            "https://example.com/info/".into(),
            HttpResponse {
                status: 200,
                headers: Vec::new(),
                body: b"<h1>Program</h1>".to_vec(),
            },
        );
        let script = Script {
            hits: Mutex::new(Vec::new()),
            responses,
        };
        let report = run(&dir, &["https://example.com/info".into()], false, &script).unwrap();
        assert_eq!(report.pages[0].path, "pages/001.html");
        assert_eq!(report.pages[0].final_url, "https://example.com/info/");
        assert_eq!(report.pages[0].status, 200);
        assert_eq!(
            fs::read(dir.join("pages/001.html")).unwrap(),
            b"<h1>Program</h1>"
        );
        assert_eq!(
            fs::read_to_string(dir.join("scope.md")).unwrap(),
            "user edited\n"
        );
        let meta: serde_json::Value =
            serde_json::from_slice(&fs::read(dir.join("fetch.json")).unwrap()).unwrap();
        assert_eq!(meta["schema"], FETCH_SCHEMA);
        assert_eq!(meta["pages"][0]["sha256"], report.pages[0].sha256);
        let again = run(&dir, &["https://example.com/info".into()], false, &script).unwrap_err();
        assert!(again.contains("--force"), "{again}");
        let forced = run(&dir, &["https://example.com/info".into()], true, &script).unwrap();
        assert_eq!(forced.pages[0].path, "pages/001.html");
        assert_eq!(
            fs::read_to_string(dir.join("scope.md")).unwrap(),
            "user edited\n"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_body_over_the_cap_saves_nothing() {
        let dir = scratch("cap");
        let mut responses = BTreeMap::new();
        responses.insert(
            "https://example.com/big".into(),
            HttpResponse {
                status: 200,
                headers: Vec::new(),
                body: vec![b'x'; 32],
            },
        );
        let script = Script {
            hits: Mutex::new(Vec::new()),
            responses,
        };
        let err = run_capped(
            &dir,
            &["https://example.com/big".into()],
            false,
            &script,
            16,
        )
        .unwrap_err();
        assert!(err.contains("over the 16 byte cap"), "{err}");
        assert!(!dir.join("fetch.json").exists());
        assert!(!dir.join("pages/001.html").exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn utc_format_is_a_zulu_timestamp() {
        assert_eq!(format_utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(MAX_BODY_BYTES, 16 * 1024 * 1024);
        assert_eq!(MAX_REDIRECTS, 5);
        assert_eq!(url::TIMEOUT_SECS, 30);
    }

    fn script(pairs: Vec<(&str, u16, Vec<(&str, &str)>, &[u8])>) -> Script {
        let mut responses = BTreeMap::new();
        for (url, status, headers, body) in pairs {
            responses.insert(
                url.to_string(),
                HttpResponse {
                    status,
                    headers: headers
                        .into_iter()
                        .map(|(k, v)| (k.to_string(), v.to_string()))
                        .collect(),
                    body: body.to_vec(),
                },
            );
        }
        Script {
            hits: Mutex::new(Vec::new()),
            responses,
        }
    }

    fn page(url: &str, final_url: &str, path: &str) -> PageRecord {
        PageRecord {
            url: url.into(),
            final_url: final_url.into(),
            status: 200,
            retrieved: "1970-01-01T00:00:00Z".into(),
            bytes: 3,
            sha256: "0123456789abcdef0123".into(),
            path: path.into(),
        }
    }

    fn write_fetch_json(dir: &Path, pages: Vec<PageRecord>) {
        let file = FetchFile {
            schema: FETCH_SCHEMA.into(),
            tool: crate::TOOL.into(),
            version: "0".into(),
            pages,
        };
        fs::write(dir.join("fetch.json"), serde_json::to_vec(&file).unwrap()).unwrap();
    }

    #[test]
    fn render_lists_each_page_and_the_redirect_target_and_strips_control_bytes() {
        let report = FetchReport {
            dir: PathBuf::from("prog"),
            pages: vec![
                page(
                    "https://a.example/x",
                    "https://a.example/x",
                    "pages/001.html",
                ),
                page(
                    "https://a.example/y",
                    "https://a.example/y/",
                    "pages/002.html",
                ),
                PageRecord {
                    sha256: "abc".into(),
                    ..page(
                        "https://a.example/\u{1b}[2J",
                        "https://a.example/\u{1b}[2J",
                        "pages/003.html",
                    )
                },
            ],
        };
        let out = report.render();
        assert!(out.contains("fetched 3 page(s) into prog"), "{out}");
        assert!(
            out.contains(
                "pages/001.html  200  3 bytes  sha256 0123456789ab  https://a.example/x\n"
            ),
            "{out}"
        );
        assert!(
            out.contains("https://a.example/y  https://a.example/y/\n"),
            "{out}"
        );
        assert!(
            out.contains("sha256 abc "),
            "a short hash is printed whole: {out}"
        );
        assert!(
            !out.contains('\u{1b}'),
            "an escape byte reached the terminal: {out:?}"
        );
        assert!(out.ends_with("Next: priorart build prog\n"), "{out}");
    }

    #[test]
    fn empty_duplicate_and_non_directory_inputs_are_refused_before_any_request() {
        let dir = scratch("inputs");
        let s = script(vec![]);
        let err = run(&dir, &[], false, &s).unwrap_err();
        assert!(err.contains("at least one HTTPS URL"), "{err}");
        let u = "https://example.com/a".to_string();
        let err = run(&dir, &[u.clone(), u.clone()], false, &s).unwrap_err();
        assert!(err.contains("more than once"), "{err}");
        let file = dir.join("not-a-dir");
        fs::write(&file, "x").unwrap();
        let err = run(&file, &[u], false, &s).unwrap_err();
        assert!(err.contains("is not a directory"), "{err}");
        assert!(s.hits.lock().unwrap().is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_new_folder_is_created_and_a_relative_redirect_is_followed() {
        let dir = scratch("create").join("new").join("prog");
        let s = script(vec![
            (
                "https://example.com/a",
                301,
                vec![("location", "/b")],
                &b""[..],
            ),
            ("https://example.com/b", 200, vec![], &b"ok"[..]),
        ]);
        let report = run(&dir, &["https://example.com/a".into()], false, &s).unwrap();
        assert_eq!(report.pages[0].final_url, "https://example.com/b");
        assert_eq!(fs::read(dir.join("pages/001.html")).unwrap(), &b"ok"[..]);
        assert!(
            !dir.join("pages/001.tmp").exists(),
            "the temp file was left behind"
        );
        let _ = fs::remove_dir_all(dir.parent().unwrap().parent().unwrap());
    }

    #[test]
    fn a_redirect_without_location_or_to_an_unfetchable_target_saves_nothing() {
        let dir = scratch("badloc");
        for (loc, want) in [
            (None, "has no Location"),
            (Some("https://127.0.0.1/x"), "will not request"),
            (Some("file:///etc/passwd"), "priorart fetch"),
        ] {
            let headers = loc.map(|l| vec![("Location", l)]).unwrap_or_default();
            let s = script(vec![("https://example.com/r", 307, headers, &b""[..])]);
            let err = run(&dir, &["https://example.com/r".into()], false, &s).unwrap_err();
            assert!(err.contains(want), "{loc:?}: {err}");
            assert_eq!(
                s.hits.lock().unwrap().len(),
                1,
                "{loc:?}: a second request was made"
            );
            assert!(!dir.join("fetch.json").exists());
            assert!(!dir.join("pages").exists());
        }
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn non_2xx_and_transport_errors_save_nothing() {
        let dir = scratch("status");
        for status in [100u16, 204 + 100, 404, 500] {
            let s = script(vec![(
                "https://example.com/s",
                status,
                vec![],
                &b"body"[..],
            )]);
            let err = run(&dir, &["https://example.com/s".into()], false, &s).unwrap_err();
            assert!(err.contains(&format!("HTTP {status}")), "{status}: {err}");
        }
        let s = script(vec![]);
        let err = run(&dir, &["https://example.com/timeout".into()], false, &s).unwrap_err();
        assert!(
            err.starts_with("cannot fetch https://example.com/timeout"),
            "{err}"
        );
        assert!(!dir.join("fetch.json").exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_second_url_gets_the_next_free_page_and_the_first_keeps_its_path() {
        let dir = scratch("next");
        fs::create_dir_all(dir.join("pages")).unwrap();
        write_fetch_json(
            &dir,
            vec![page(
                "https://example.com/old",
                "https://example.com/old",
                "pages/001.html",
            )],
        );
        let s = script(vec![("https://example.com/new", 200, vec![], &b"n"[..])]);
        let report = run(&dir, &["https://example.com/new".into()], false, &s).unwrap();
        assert_eq!(report.pages[0].path, "pages/002.html");
        let meta: FetchFile =
            serde_json::from_slice(&fs::read(dir.join("fetch.json")).unwrap()).unwrap();
        let paths: Vec<_> = meta.pages.iter().map(|p| p.path.as_str()).collect();
        assert_eq!(paths, ["pages/001.html", "pages/002.html"]);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_corrupt_or_foreign_fetch_json_is_never_overwritten() {
        let dir = scratch("corrupt");
        let s = script(vec![("https://example.com/a", 200, vec![], &b"a"[..])]);
        for (body, want) in [
            (&b"{not json"[..], "is not a priorart.fetch/v1 file"),
            (
                &br#"{"schema":"other/v9","tool":"x","version":"0","pages":[]}"#[..],
                "has schema \"other/v9\"",
            ),
        ] {
            fs::write(dir.join("fetch.json"), body).unwrap();
            let err = run(&dir, &["https://example.com/a".into()], false, &s).unwrap_err();
            assert!(err.contains(want), "{err}");
            assert_eq!(fs::read(dir.join("fetch.json")).unwrap(), body);
        }
        assert!(s.hits.lock().unwrap().is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_poisoned_fetch_json_cannot_send_a_page_outside_pages_or_onto_scope_md() {
        let dir = scratch("poison");
        let u = "https://example.com/p";
        for bad in [
            "../escape.html",
            "/abs.html",
            "C:/x.html",
            "pages/../../x.html",
            "pages\\..\\x.html",
            "pages/",
            "scope.md",
        ] {
            write_fetch_json(&dir, vec![page(u, u, bad)]);
            let s = script(vec![(u, 200, vec![], &b"evil"[..])]);
            let err = run(&dir, &[u.into()], true, &s).unwrap_err();
            assert!(err.contains("not a file under pages/"), "{bad}: {err}");
            assert!(s.hits.lock().unwrap().is_empty(), "{bad}: requested anyway");
        }
        assert!(!dir.parent().unwrap().join("escape.html").exists());
        write_fetch_json(&dir, vec![page(u, u, "pages/scope.md")]);
        let s = script(vec![(u, 200, vec![], &b"evil"[..])]);
        let err = run(&dir, &[u.into()], true, &s).unwrap_err();
        assert!(err.contains("never overwrites scope.md"), "{err}");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn header_lookup_ignores_case_and_only_listed_codes_redirect() {
        let h = vec![("LOCATION".to_string(), "x".to_string())];
        assert_eq!(header(&h, "location"), Some("x"));
        assert_eq!(header(&h, "content-type"), None);
        for code in [301, 302, 303, 307, 308] {
            assert!(is_redirect(code), "{code}");
        }
        for code in [200, 300, 304, 305, 306, 309, 399] {
            assert!(!is_redirect(code), "{code}");
        }
    }

    #[test]
    fn utc_dates_cross_leap_days_and_centuries() {
        assert_eq!(format_utc(951_782_400), "2000-02-29T00:00:00Z");
        assert_eq!(format_utc(4_107_542_399), "2100-02-28T23:59:59Z");
        assert_eq!(format_utc(1_709_164_800 + 3_661), "2024-02-29T01:01:01Z");
        assert!(utc_now().ends_with('Z'));
        assert_eq!(utc_now().len(), 20);
    }

    proptest::proptest! {
        #[test]
        fn civil_from_days_is_monotone_and_in_range(days in 0i64..200_000) {
            let (y, m, d) = civil_from_days(days);
            let (y2, m2, d2) = civil_from_days(days + 1);
            proptest::prop_assert!((1..=12).contains(&m) && (1..=31).contains(&d));
            proptest::prop_assert!((y2, m2, d2) > (y, m, d));
        }

        #[test]
        fn check_page_path_accepts_only_plain_files_under_pages(s in "\\PC{0,40}") {
            if check_page_path(&s).is_ok() {
                proptest::prop_assert!(s.starts_with("pages/"));
                proptest::prop_assert!(!s.split('/').any(|p| p == ".." || p == "." || p.is_empty()));
                proptest::prop_assert!(!s.contains('\\') && !s.contains(':'));
            }
            let numbered = format!("pages/{:03}.html", s.len());
            proptest::prop_assert!(check_page_path(&numbered).is_ok());
        }
    }
}
