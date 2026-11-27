//! URL checks for `priorart fetch` and for resolving an `<a href>` against a
//! fetched page's final URL.
//!
//! Nothing here opens a socket. `std::net` is not used: an IP literal is
//! recognised from the host text, so the no-socket scan stays empty.

/// How many redirects `fetch` will follow. The next response must not itself
/// be a redirect.
pub const MAX_REDIRECTS: usize = 5;

/// Body cap for one response. One byte over is refused and nothing is saved.
pub const MAX_BODY_BYTES: usize = 16 * 1024 * 1024;

/// Connect, read, write and overall deadline for one GET.
pub const TIMEOUT_SECS: u64 = 30;

/// Accept a URL `priorart fetch` is allowed to request.
///
/// HTTPS only. `http:`, `file:` and every other scheme are rejected, as are
/// userinfo, IP-literal hosts (dotted IPv4, bracketed IPv6, an all-digit
/// host, a `0x` host) and `localhost` (including `*.localhost`). DNS is not
/// consulted: a name that later resolves to a private address is not rejected
/// here.
pub fn validate_fetch_url(raw: &str) -> Result<(), String> {
    let url = raw.trim();
    if url.is_empty() {
        return Err("a fetch URL is empty".into());
    }
    if url
        .chars()
        .any(|c| c.is_whitespace() || c.is_control() || c == '\\')
    {
        return Err(format!(
            "{url:?} is not a fetchable URL (whitespace, controls and backslashes are rejected)"
        ));
    }
    let Some((scheme, rest)) = split_scheme(url) else {
        return Err(format!(
            "{url:?} is not an absolute URL. priorart fetch requests HTTPS URLs only."
        ));
    };
    if !scheme.eq_ignore_ascii_case("https") {
        return Err(format!(
            "{url:?} uses {scheme}:. priorart fetch requests HTTPS URLs only (http:, file: and every other scheme are rejected)."
        ));
    }
    if !rest.starts_with("//") {
        return Err(format!("{url:?} is not a hierarchical HTTPS URL"));
    }
    let authority_and_path = &rest[2..];
    if authority_and_path.is_empty() {
        return Err(format!("{url:?} has no host"));
    }
    let (authority, _) = split_authority(authority_and_path);
    if authority.is_empty() {
        return Err(format!("{url:?} has no host"));
    }
    if authority.contains('@') {
        return Err(format!(
            "{url:?} carries userinfo. priorart fetch sends no credentials and rejects URLs that contain them."
        ));
    }
    let host = host_of(authority)?;
    if host.is_empty() {
        return Err(format!("{url:?} has no host"));
    }
    if is_localhost(&host) {
        return Err(format!(
            "{url:?} names a localhost host. priorart fetch does not request localhost."
        ));
    }
    if is_ip_literal(&host) {
        return Err(format!(
            "{url:?} names an IP-literal host. priorart fetch does not request IP literals."
        ));
    }
    Ok(())
}

/// True when `host` is an IP literal: dotted IPv4, bracketed or bare IPv6, an
/// all-digit host, or a `0x` hex host.
pub fn is_ip_literal(host: &str) -> bool {
    let host = host.trim();
    if host.is_empty() {
        return false;
    }
    if host.starts_with('[') {
        return true;
    }
    if host.contains(':') {
        // A bare IPv6 host. A port is separated before this is called.
        return true;
    }
    if host.len() > 2
        && host[..2].eq_ignore_ascii_case("0x")
        && host[2..].chars().all(|c| c.is_ascii_hexdigit())
    {
        return true;
    }
    if host.chars().all(|c| c.is_ascii_digit()) {
        return true;
    }
    let parts: Vec<&str> = host.split('.').collect();
    if parts.len() == 4
        && parts.iter().all(|p| {
            !p.is_empty()
                && p.len() <= 3
                && p.chars().all(|c| c.is_ascii_digit())
                && p.parse::<u16>().ok().is_some_and(|n| n <= 255)
        })
    {
        return true;
    }
    false
}

fn is_localhost(host: &str) -> bool {
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    host == "localhost" || host.ends_with(".localhost")
}

/// Join `href` onto `base` when `href` is relative. An absolute `href` is
/// returned unchanged apart from tracking parameters. A relative `href` with
/// no base is returned unchanged (still relative).
pub fn resolve_link(base: Option<&str>, href: &str) -> Option<String> {
    let href = href.trim();
    if href.is_empty() {
        return None;
    }
    let lower = href.to_ascii_lowercase();
    if lower.starts_with("javascript:")
        || lower.starts_with("data:")
        || lower.starts_with("mailto:")
        || lower.starts_with("blob:")
        || lower.starts_with("file:")
    {
        return None;
    }
    if split_scheme(href).is_some() {
        return Some(strip_tracking(href));
    }
    let Some(base) = base.map(str::trim).filter(|b| !b.is_empty()) else {
        return Some(strip_tracking(href));
    };
    Some(strip_tracking(&join_url(base, href)))
}

/// Drop query keys whose names start with `utm_`. The fragment is kept.
pub fn strip_tracking(url: &str) -> String {
    let (before_frag, frag) = match url.split_once('#') {
        Some((a, b)) => (a, Some(b)),
        None => (url, None),
    };
    let (path, query) = match before_frag.split_once('?') {
        Some((p, q)) => (p, Some(q)),
        None => (before_frag, None),
    };
    let mut out = path.to_string();
    if let Some(query) = query {
        let kept: Vec<&str> = query
            .split('&')
            .filter(|pair| {
                let name = pair.split_once('=').map(|(n, _)| n).unwrap_or(pair);
                !name.to_ascii_lowercase().starts_with("utm_")
            })
            .filter(|pair| !pair.is_empty())
            .collect();
        if !kept.is_empty() {
            out.push('?');
            out.push_str(&kept.join("&"));
        }
    }
    if let Some(frag) = frag {
        out.push('#');
        out.push_str(frag);
    }
    out
}

fn join_url(base: &str, href: &str) -> String {
    if href.starts_with("//") {
        let scheme = split_scheme(base).map(|(s, _)| s).unwrap_or("https");
        return format!("{scheme}:{href}");
    }
    let Some((scheme, rest)) = split_scheme(base) else {
        return href.to_string();
    };
    if !rest.starts_with("//") {
        return href.to_string();
    }
    let authority_and = &rest[2..];
    let (authority, after_auth) = split_authority(authority_and);
    let origin = format!("{scheme}://{authority}");
    if href.starts_with('/') {
        return format!("{origin}{href}");
    }
    if href.starts_with('?') || href.starts_with('#') {
        let path = after_auth
            .split_once('?')
            .map(|(p, _)| p)
            .unwrap_or(after_auth);
        let path = path.split_once('#').map(|(p, _)| p).unwrap_or(path);
        return format!("{origin}{path}{href}");
    }
    let path = after_auth
        .split_once('?')
        .map(|(p, _)| p)
        .unwrap_or(after_auth);
    let path = path.split_once('#').map(|(p, _)| p).unwrap_or(path);
    let dir = match path.rfind('/') {
        Some(i) => &path[..=i],
        None => "/",
    };
    let combined = format!("{dir}{href}");
    format!("{origin}{}", normalise_path(&combined))
}

fn normalise_path(path: &str) -> String {
    let (path, suffix) = if let Some(i) = path.find(['?', '#']) {
        (&path[..i], &path[i..])
    } else {
        (path, "")
    };
    let mut parts: Vec<&str> = Vec::new();
    for seg in path.split('/') {
        if seg.is_empty() || seg == "." {
            continue;
        }
        if seg == ".." {
            parts.pop();
            continue;
        }
        parts.push(seg);
    }
    let mut out = String::from("/");
    out.push_str(&parts.join("/"));
    out.push_str(suffix);
    out
}

fn split_scheme(url: &str) -> Option<(&str, &str)> {
    let bytes = url.as_bytes();
    if !bytes.first().is_some_and(|c| c.is_ascii_alphabetic()) {
        return None;
    }
    let mut i = 1;
    while i < bytes.len()
        && (bytes[i].is_ascii_alphanumeric() || matches!(bytes[i], b'+' | b'-' | b'.'))
    {
        i += 1;
    }
    if bytes.get(i) != Some(&b':') {
        return None;
    }
    Some((&url[..i], &url[i + 1..]))
}

fn split_authority(authority_and_path: &str) -> (&str, &str) {
    match authority_and_path.find(['/', '?', '#']) {
        Some(i) => (&authority_and_path[..i], &authority_and_path[i..]),
        None => (authority_and_path, ""),
    }
}

fn host_of(authority: &str) -> Result<String, String> {
    if let Some(rest) = authority.strip_prefix('[') {
        let Some(end) = rest.find(']') else {
            return Err(format!("{authority:?} has an unterminated IPv6 host"));
        };
        let host = format!("[{}]", &rest[..end]);
        let after = &rest[end + 1..];
        if !after.is_empty() && !after.starts_with(':') {
            return Err(format!("{authority:?} is not a host"));
        }
        if let Some(port) = after.strip_prefix(':') {
            check_port(port)?;
        }
        return Ok(host);
    }
    let (host, port) = match authority.rsplit_once(':') {
        Some((h, p)) if !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()) => (h, Some(p)),
        _ => (authority, None),
    };
    if let Some(port) = port {
        check_port(port)?;
    }
    if host.is_empty() || host.contains(':') {
        return Err(format!("{authority:?} is not a host this build requests"));
    }
    Ok(host.to_string())
}

fn check_port(port: &str) -> Result<(), String> {
    if port.is_empty() || !port.chars().all(|c| c.is_ascii_digit()) {
        return Err(format!("port {port:?} is not a number"));
    }
    if port.parse::<u32>().ok().is_none_or(|n| n > 65535) {
        return Err(format!("port {port} is outside 0-65535"));
    }
    Ok(())
}

/// A GitHub repository, tree or blob URL. Marketing paths (`/topics`,
/// `/features`, …) are not code.
pub fn is_github_code_url(url: &str) -> bool {
    let Some((_, rest)) = split_scheme(url) else {
        return false;
    };
    if !rest.starts_with("//") {
        return false;
    }
    let (authority, after) = split_authority(&rest[2..]);
    let host = host_of(authority).unwrap_or_default();
    if !host.eq_ignore_ascii_case("github.com") && !host.eq_ignore_ascii_case("www.github.com") {
        return false;
    }
    let path = after.split(['?', '#']).next().unwrap_or("");
    let segs: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    if segs.len() < 2 {
        return false;
    }
    let owner = segs[0];
    let repo = segs[1].trim_end_matches(".git");
    if repo.is_empty() || github_meta_owner(owner) {
        return false;
    }
    segs.len() == 2 || matches!(segs[2], "tree" | "blob" | "commit")
}

fn github_meta_owner(owner: &str) -> bool {
    matches!(
        owner.to_ascii_lowercase().as_str(),
        "topics"
            | "features"
            | "marketplace"
            | "settings"
            | "explore"
            | "collections"
            | "about"
            | "pricing"
            | "login"
            | "signup"
            | "orgs"
            | "sponsors"
            | "apps"
            | "pulls"
            | "issues"
            | "notifications"
            | "new"
            | "codespaces"
            | "account"
            | "customer-stories"
            | "readme"
            | "enterprise"
            | "security"
            | "organizations"
            | "site"
            | "gitignore"
            | "search"
    )
}

/// An explorer URL whose path carries a 20-byte account (`/address/0x`… or
/// `/token/0x`…).
pub fn is_explorer_address_url(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    for marker in ["/address/0x", "/token/0x"] {
        if let Some(at) = lower.find(marker) {
            let hex = lower[at + marker.len()..]
                .chars()
                .take_while(|c| c.is_ascii_hexdigit())
                .count();
            if hex == 40 {
                return true;
            }
        }
    }
    false
}

/// True when `text` contains a GitHub code URL or an explorer address URL.
pub fn contains_scope_target(text: &str) -> bool {
    for url in http_urls(text) {
        let url = strip_tracking(&url);
        if is_github_code_url(&url) || is_explorer_address_url(&url) {
            return true;
        }
    }
    false
}

fn http_urls(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let bytes = text.as_bytes();
    let mut i = 0;
    // Byte-wise and case-insensitive: `i` may sit inside a multi-byte char,
    // so the text is never sliced at `i` (a fuzzed U+FFFD panicked here), and
    // no lowercase copy of the rest is made per byte.
    let starts = |i: usize, scheme: &[u8]| {
        bytes.len() >= i + scheme.len() && bytes[i..i + scheme.len()].eq_ignore_ascii_case(scheme)
    };
    while i + 8 < bytes.len() {
        let scheme_len = if starts(i, b"https://") {
            8
        } else if starts(i, b"http://") {
            7
        } else {
            i += 1;
            continue;
        };
        let start = i;
        i += scheme_len;
        while i < bytes.len()
            && !bytes[i].is_ascii_whitespace()
            && !matches!(bytes[i], b'<' | b'>' | b'"' | b'\'' | b')' | b']')
        {
            i += 1;
        }
        // Trim a trailing sentence mark that is not part of the URL.
        let mut end = i;
        while end > start && matches!(bytes[end - 1], b'.' | b',' | b';') {
            end -= 1;
        }
        out.push(text[start..end].to_string());
    }
    out
}

#[cfg(test)]
mod tests {

    #[test]
    fn urls_next_to_multibyte_text_neither_panic_nor_shift() {
        // Fuzz crash e98545bc (markdown_import): U+FFFD replacement chars made
        // `http_urls` slice inside a char.
        let text = "acknowl\u{fffd}\u{fffd}ged, see HTTPS://github.com/o/r/blob/main/A.sol, and \u{1f600}x";
        assert_eq!(
            http_urls(text),
            vec!["HTTPS://github.com/o/r/blob/main/A.sol".to_string()]
        );
        assert!(!contains_scope_target(
            "\u{fffd}\u{fffd}\u{fffd}\u{fffd}\u{fffd}"
        ));
        for cut in 0..text.len() {
            if text.is_char_boundary(cut) {
                let _ = http_urls(&text[cut..]);
            }
        }
    }

    use super::*;

    #[test]
    fn each_unfetchable_character_class_is_rejected_on_its_own() {
        // Mutation survivors: `||` -> `&&` in the character check let a URL
        // through that has only one of the three.
        for url in [
            "https://a.example/x y",
            "https://a.example/x\u{7f}",
            "https://a.example/x\\y",
            "https://a.example/x\ty",
        ] {
            let e = validate_fetch_url(url).expect_err(url);
            assert!(
                e.contains("whitespace, controls and backslashes"),
                "{url:?}: {e}"
            );
        }
    }

    #[test]
    fn ip_literal_detection_is_exact_at_its_edges() {
        assert!(!is_ip_literal("0x"), "0x alone is not an address");
        assert!(is_ip_literal("0x7f"));
        assert!(!is_ip_literal("cafe"), "hex digits without 0x are a name");
        assert!(!is_ip_literal("deadbeef"));
        assert!(!is_ip_literal("1.2"));
        assert!(!is_ip_literal("10.0.0"));
        assert!(!is_ip_literal("1.2.3.4.5"));
        assert!(is_ip_literal("1.2.3.4"));
        assert!(is_ip_literal("255.255.255.255"));
        assert!(!is_ip_literal("1.2.3.256"));
        assert!(!is_ip_literal("1.2.3.+5"), "a sign is not a digit");
        assert!(!is_ip_literal("1.2.3.0001"), "four digits is not an octet");
    }

    #[test]
    fn non_web_schemes_are_never_followed_whatever_their_case() {
        for href in [
            "mailto:a@b.example",
            "MAILTO:a@b.example",
            "blob:https://e.example/x",
            "file:///etc/passwd",
            "FILE:///etc/passwd",
            "javascript:alert(1)",
            "data:text/html,x",
        ] {
            assert_eq!(
                resolve_link(Some("https://e.example/"), href),
                None,
                "{href}"
            );
        }
    }

    #[test]
    fn a_fragment_or_query_only_href_keeps_the_base_path() {
        let base = "https://e.example/a/b.html?old=1#top";
        assert_eq!(
            resolve_link(Some(base), "#sec").as_deref(),
            Some("https://e.example/a/b.html#sec")
        );
        assert_eq!(
            resolve_link(Some(base), "?page=2").as_deref(),
            Some("https://e.example/a/b.html?page=2")
        );
    }

    #[test]
    fn host_and_port_errors_name_their_own_cause() {
        assert_eq!(host_of("[::1]:443").unwrap(), "[::1]");
        assert!(host_of("[::1]x").unwrap_err().contains("is not a host"));
        assert_eq!(host_of("example.com:8080").unwrap(), "example.com");
        for bad in ["example.com:abc", "example.com:"] {
            let e = host_of(bad).unwrap_err();
            assert!(e.contains("not a host this build requests"), "{bad}: {e}");
        }
        assert!(host_of(":80").is_err(), "an empty host is refused");
        assert!(check_port("").unwrap_err().contains("not a number"));
        assert!(check_port("abc").unwrap_err().contains("not a number"));
        assert!(check_port("0").is_ok());
        assert!(check_port("65535").is_ok());
        assert!(check_port("65536").unwrap_err().contains("outside 0-65535"));
        assert!(check_port("70000").is_err());
    }

    #[test]
    fn github_code_urls_need_a_github_host_with_or_without_www() {
        assert!(is_github_code_url("https://github.com/org/repo"));
        assert!(is_github_code_url("https://www.github.com/org/repo"));
        assert!(!is_github_code_url("https://evil.example/org/repo"));
    }

    #[test]
    fn a_scheme_with_nothing_after_it_is_not_a_url() {
        assert!(http_urls("https://").is_empty());
        assert!(http_urls("see https://").is_empty());
        assert_eq!(
            http_urls("see https://a.example."),
            vec!["https://a.example"]
        );
    }

    #[test]
    fn https_is_accepted_and_other_schemes_are_rejected() {
        assert!(validate_fetch_url("https://example.com/a").is_ok());
        assert!(validate_fetch_url("https://example.com:8443/a").is_ok());
        for bad in [
            "http://example.com/a",
            "file:///etc/passwd",
            "ftp://example.com/a",
            "https://user:pw@example.com/a",
            "https://127.0.0.1/",
            "https://127.0.0.1:443/x",
            "https://[::1]/",
            "https://2130706433/",
            "https://0x7f000001/",
            "https://localhost/a",
            "https://foo.localhost/a",
            "https://example.com\\@evil.com/",
            "",
        ] {
            assert!(validate_fetch_url(bad).is_err(), "{bad} was accepted");
        }
    }

    #[test]
    fn tracking_parameters_are_removed_and_other_queries_stay() {
        assert_eq!(
            strip_tracking("https://github.com/acme/vault?utm_source=x&utm_medium=y"),
            "https://github.com/acme/vault"
        );
        assert_eq!(
            strip_tracking("https://github.com/acme/vault?keep=1&utm_source=x"),
            "https://github.com/acme/vault?keep=1"
        );
    }

    #[test]
    fn relative_links_resolve_only_against_a_base() {
        assert_eq!(
            resolve_link(None, "/src/Vault.sol").as_deref(),
            Some("/src/Vault.sol")
        );
        assert_eq!(
            resolve_link(Some("https://example.com/program/scope"), "asset").as_deref(),
            Some("https://example.com/program/asset")
        );
        assert_eq!(
            resolve_link(Some("https://example.com/program/scope"), "/abs").as_deref(),
            Some("https://example.com/abs")
        );
        assert_eq!(
            resolve_link(Some("https://example.com/"), "javascript:alert(1)"),
            None
        );
    }

    #[test]
    fn github_code_urls_and_explorer_addresses_are_scope_targets() {
        assert!(is_github_code_url("https://github.com/acme/vault"));
        assert!(is_github_code_url(
            "https://github.com/acme/vault/tree/main/src"
        ));
        assert!(is_github_code_url(
            "https://github.com/acme/vault/blob/main/src/Vault.sol"
        ));
        assert!(!is_github_code_url("https://github.com/topics/solidity"));
        assert!(!is_github_code_url("https://github.com/acme"));
        assert!(is_explorer_address_url(
            "https://etherscan.io/address/0x1234567890abcdef1234567890abcdef12345678"
        ));
        assert!(!is_explorer_address_url(
            "https://etherscan.io/address/not-an-address"
        ));
        assert!(contains_scope_target(
            "Target: https://github.com/acme/vault?utm_source=x; Added on: 1 January 2020"
        ));
    }
}
