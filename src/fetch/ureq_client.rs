//! The HTTP client behind `priorart fetch`.
//!
//! Compiled only with `--features fetch` (on by default). `ureq` 2 with its
//! `tls` feature speaks HTTPS through rustls. There is no cookie store, no
//! `Authorization` header and no proxy taken from the environment: the
//! `proxy-from-env` feature is off, and the agent is built with
//! `try_proxy_from_env(false)`. `http_proxy`, `https_proxy`, `all_proxy` and
//! their uppercase forms are ignored. A redirect is not followed here; the
//! policy loop in the parent module does that, so each hop is checked.
//!
//! Every host name is resolved here, before connecting. If any address it
//! resolves to is not a public unicast address (loopback, private, link-local,
//! carrier-grade NAT, unique-local, multicast, documentation, reserved, or an
//! IPv4-mapped or NAT64 form of one), the request is refused and nothing is
//! sent. Checking after DNS closes the gap a name check alone leaves: a public
//! name that resolves to `127.0.0.1` or `169.254.169.254`. This is the only
//! source file that names `std::net::`; the no-network CI scan allows it here.

use crate::fetch::url::{MAX_BODY_BYTES, TIMEOUT_SECS};
use crate::fetch::{HttpResponse, Transport};
use std::io::{self, Read};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, ToSocketAddrs};
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
            .resolver(resolve_public)
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

/// Resolve `netloc` (`host:port`) and refuse it unless every address is public.
fn resolve_public(netloc: &str) -> io::Result<Vec<SocketAddr>> {
    let addrs: Vec<SocketAddr> = netloc.to_socket_addrs()?.collect();
    check_resolved(netloc, addrs).map_err(|e| io::Error::new(io::ErrorKind::PermissionDenied, e))
}

/// Refuse an empty answer or one that holds any non-public address. All
/// addresses are checked, not only the first, so a mixed answer cannot be
/// used to reach a private address on a later connection attempt.
fn check_resolved(netloc: &str, addrs: Vec<SocketAddr>) -> Result<Vec<SocketAddr>, String> {
    if addrs.is_empty() {
        return Err(format!(
            "{netloc} resolved to no address. priorart fetch sends nothing."
        ));
    }
    if let Some(bad) = addrs.iter().find(|a| !is_public_ip(a.ip())) {
        return Err(format!(
            "{netloc} resolves to {}, which is not a public address. priorart fetch does not request private, loopback, link-local or reserved addresses.",
            bad.ip()
        ));
    }
    Ok(addrs)
}

/// True only for a public unicast address.
fn is_public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_public_v4(v4),
        IpAddr::V6(v6) => is_public_v6(v6),
    }
}

fn is_public_v4(ip: Ipv4Addr) -> bool {
    let [a, b, c, _] = ip.octets();
    !(a == 0                                   // 0.0.0.0/8 "this network"
        || a == 10                             // 10.0.0.0/8 private
        || a == 127                            // 127.0.0.0/8 loopback
        || (a == 100 && (64..=127).contains(&b)) // 100.64.0.0/10 carrier-grade NAT
        || (a == 169 && b == 254)              // 169.254.0.0/16 link-local, cloud metadata
        || (a == 172 && (16..=31).contains(&b)) // 172.16.0.0/12 private
        || (a == 192 && b == 0 && c == 0)      // 192.0.0.0/24 protocol assignments
        || (a == 192 && b == 0 && c == 2)      // 192.0.2.0/24 documentation
        || (a == 192 && b == 88 && c == 99)    // 192.88.99.0/24 6to4 relay
        || (a == 192 && b == 168)              // 192.168.0.0/16 private
        || (a == 198 && (18..=19).contains(&b)) // 198.18.0.0/15 benchmarking
        || (a == 198 && b == 51 && c == 100)   // 198.51.100.0/24 documentation
        || (a == 203 && b == 0 && c == 113)    // 203.0.113.0/24 documentation
        || a >= 224) // 224.0.0.0/4 multicast, 240.0.0.0/4 reserved, broadcast
}

fn is_public_v6(ip: Ipv6Addr) -> bool {
    let seg = ip.segments();
    if let Some(v4) = embedded_v4(&seg) {
        return is_public_v4(v4);
    }
    !(ip.is_unspecified()
        || ip.is_loopback()
        || seg[0] & 0xff00 == 0xff00 // ff00::/8 multicast
        || seg[0] & 0xfe00 == 0xfc00 // fc00::/7 unique-local
        || seg[0] & 0xffc0 == 0xfe80 // fe80::/10 link-local
        || seg[0] & 0xffc0 == 0xfec0 // fec0::/10 site-local (deprecated)
        || (seg[0] == 0x2001 && seg[1] == 0x0db8) // 2001:db8::/32 documentation
        || (seg[0] == 0x0100 && seg[1..4] == [0, 0, 0]) // 100::/64 discard
        || seg[0] & 0xe000 != 0x2000) // outside 2000::/3 global unicast
}

/// The IPv4 address inside an IPv4-mapped (`::ffff:a.b.c.d`), IPv4-compatible
/// (`::a.b.c.d`) or NAT64 (`64:ff9b::a.b.c.d`) address.
fn embedded_v4(seg: &[u16; 8]) -> Option<Ipv4Addr> {
    let tail = Ipv4Addr::new(
        (seg[6] >> 8) as u8,
        seg[6] as u8,
        (seg[7] >> 8) as u8,
        seg[7] as u8,
    );
    let mapped = seg[..5] == [0, 0, 0, 0, 0] && seg[5] == 0xffff;
    let compatible = seg[..6] == [0, 0, 0, 0, 0, 0] && (seg[6] != 0 || seg[7] > 1);
    let nat64 = seg[0] == 0x0064 && seg[1] == 0xff9b && seg[2..6] == [0, 0, 0, 0];
    (mapped || compatible || nat64).then_some(tail)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(s: &str) -> IpAddr {
        s.parse().expect("test address parses")
    }

    #[test]
    fn non_public_addresses_are_refused() {
        for a in [
            "0.0.0.0",
            "10.1.2.3",
            "127.0.0.1",
            "100.64.0.1",
            "100.127.255.255",
            "169.254.169.254",
            "172.16.0.1",
            "172.31.255.255",
            "192.0.0.8",
            "192.0.2.1",
            "192.88.99.1",
            "192.168.1.1",
            "198.18.0.1",
            "198.19.255.255",
            "198.51.100.7",
            "203.0.113.9",
            "224.0.0.1",
            "240.0.0.1",
            "255.255.255.255",
            "::",
            "::1",
            "ff02::1",
            "fc00::1",
            "fd12:3456::1",
            "fe80::1",
            "febf::1",
            "fec0::1",
            "2001:db8::1",
            "100::1",
            "::ffff:127.0.0.1",
            "::ffff:169.254.169.254",
            "::ffff:10.0.0.1",
            "::10.0.0.1",
            "64:ff9b::192.168.0.1",
            "64:ff9b::7f00:1",
            "4000::1",
        ] {
            assert!(!is_public_ip(ip(a)), "{a} must be refused");
        }
    }

    #[test]
    fn public_addresses_are_accepted() {
        for a in [
            "1.1.1.1",
            "8.8.8.8",
            "9.255.255.255",
            "11.0.0.1",
            "100.63.255.255",
            "100.128.0.1",
            "126.255.255.255",
            "128.0.0.1",
            "169.253.0.1",
            "169.255.0.1",
            "172.15.255.255",
            "172.32.0.1",
            "192.0.1.1",
            "192.0.3.1",
            "192.88.98.1",
            "192.167.0.1",
            "192.169.0.1",
            "198.17.0.1",
            "198.20.0.1",
            "198.51.99.1",
            "203.0.112.1",
            "223.255.255.254",
            "2606:4700::1111",
            "2a00:1450::1",
            "2001:db9::1",
            "::ffff:1.1.1.1",
            "64:ff9b::808:808",
            "2000::1",
            "3fff::1",
        ] {
            assert!(is_public_ip(ip(a)), "{a} must be accepted");
        }
    }

    #[test]
    fn one_private_address_in_an_answer_refuses_the_whole_answer() {
        let good: SocketAddr = "1.1.1.1:443".parse().unwrap();
        let bad: SocketAddr = "127.0.0.1:443".parse().unwrap();
        assert_eq!(check_resolved("x:443", vec![good]).unwrap(), vec![good]);
        let err = check_resolved("x:443", vec![good, bad]).unwrap_err();
        assert!(err.contains("127.0.0.1"), "{err}");
        assert!(check_resolved("x:443", vec![bad, good]).is_err());
        assert!(check_resolved("x:443", Vec::new()).is_err());
    }

    #[test]
    fn the_resolver_refuses_a_name_that_resolves_to_loopback() {
        let err = resolve_public("127.0.0.1:443").unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::PermissionDenied);
    }
}
