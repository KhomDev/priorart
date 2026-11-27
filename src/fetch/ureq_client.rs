//! The HTTP client behind `priorart fetch`.
//!
//! Compiled only with `--features fetch` (on by default). `ureq` 2 with its
//! `tls` feature speaks HTTPS through rustls. There is no cookie store, no
//! `Authorization` header and no proxy taken from the environment: the
//! `proxy-from-env` feature is off, and the agent is built with
//! `try_proxy_from_env(false)`. `http_proxy`, `https_proxy`, `all_proxy` and
//! their uppercase forms are ignored. A redirect is not followed here; the
//! policy loop in the parent module does that, so each hop is checked.

use crate::fetch::url::{MAX_BODY_BYTES, TIMEOUT_SECS};
use crate::fetch::{HttpResponse, Transport};
use std::io::Read;
use std::time::Duration;

pub struct UreqTransport {
    agent: ureq::Agent,
}

impl Default for UreqTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl UreqTransport {
    pub fn new() -> Self {
        let timeout = Duration::from_secs(TIMEOUT_SECS);
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(timeout)
            .timeout_read(timeout)
            .timeout_write(timeout)
            .timeout(timeout)
            .redirects(0)
            .https_only(true)
            .try_proxy_from_env(false)
            .user_agent(&format!("priorart/{}", env!("CARGO_PKG_VERSION")))
            .build();
        Self { agent }
    }
}

impl Transport for UreqTransport {
    fn get(&self, url: &str) -> Result<HttpResponse, String> {
        let response = self.agent.get(url).call().map_err(|e| e.to_string())?;
        let status = response.status();
        let mut headers = Vec::new();
        for name in response.headers_names() {
            if let Some(value) = response.header(&name) {
                headers.push((name, value.to_string()));
            }
        }
        let mut reader = response.into_reader();
        let body = read_capped(&mut reader, MAX_BODY_BYTES)?;
        Ok(HttpResponse {
            status,
            headers,
            body,
        })
    }
}

fn read_capped(reader: &mut dyn Read, max: usize) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = reader
            .read(&mut buf)
            .map_err(|e| format!("reading the response body: {e}"))?;
        if n == 0 {
            return Ok(out);
        }
        if out.len() + n > max {
            return Err(format!(
                "response body exceeds {max} bytes. priorart fetch saves nothing."
            ));
        }
        out.extend_from_slice(&buf[..n]);
    }
}
