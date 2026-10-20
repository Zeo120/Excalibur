//! Resilient DNS resolver and connection engine with in-memory caching,
//! exponential backoff retries for transient errors (such as Windows os error 11002 / WSATRY_AGAIN),
//! public DNS fallback (Cloudflare 1.1.1.1 / Google 8.8.8.8), and dual-stack connection handling.

use crate::error::{CoreError, Result};
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::{LazyLock, RwLock};
use std::time::{Duration, Instant};
use tokio::net::{TcpStream, UdpSocket};
use tracing::{debug, warn};

#[derive(Debug, Clone)]
struct DnsCacheEntry {
    addrs: Vec<SocketAddr>,
    expires_at: Instant,
}

static DNS_CACHE: LazyLock<RwLock<HashMap<String, DnsCacheEntry>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

/// Look up cached socket addresses for a given host and port.
pub fn get_cached_addrs(host: &str, port: u16) -> Option<Vec<SocketAddr>> {
    let now = Instant::now();
    let cache = DNS_CACHE.read().ok()?;
    if let Some(entry) = cache.get(host) {
        if now < entry.expires_at {
            let mut result = Vec::new();
            for addr in &entry.addrs {
                let mut remapped = *addr;
                remapped.set_port(port);
                result.push(remapped);
            }
            if !result.is_empty() {
                return Some(result);
            }
        }
    }
    None
}

const MAX_DNS_CACHE_ENTRIES: usize = 1024;

/// Store resolved socket addresses in the in-memory DNS cache.
pub fn store_cached_addrs(host: &str, addrs: &[SocketAddr], ttl: Duration) {
    if addrs.is_empty() {
        return;
    }
    if let Ok(mut cache) = DNS_CACHE.write() {
        let now = Instant::now();
        if cache.len() >= MAX_DNS_CACHE_ENTRIES {
            cache.retain(|_, entry| entry.expires_at > now);
        }
        if cache.len() >= MAX_DNS_CACHE_ENTRIES {
            if let Some(first_key) = cache.keys().next().cloned() {
                cache.remove(&first_key);
            }
        }
        cache.insert(
            host.to_string(),
            DnsCacheEntry {
                addrs: addrs.to_vec(),
                expires_at: now + ttl,
            },
        );
    }
}

/// Checks whether an I/O error looks like a transient DNS resolution failure.
fn is_transient_dns_error(err: &std::io::Error) -> bool {
    // Windows WSAHOST_NOT_FOUND / WSATRY_AGAIN is 11002
    if let Some(code) = err.raw_os_error() {
        if code == 11002 || code == 11001 || code == 10060 || code == 10065 {
            return true;
        }
    }
    let msg = err.to_string().to_lowercase();
    msg.contains("11002")
        || msg.contains("temporary error")
        || msg.contains("authoritative")
        || msg.contains("try again")
}

/// Resolves a hostname using system DNS with automatic exponential backoff retries
/// if a transient error (such as Windows os error 11002) is encountered.
async fn resolve_system_dns(host: &str, port: u16) -> std::io::Result<Vec<SocketAddr>> {
    let mut backoff = Duration::from_millis(150);
    let mut last_err = None;

    for attempt in 0..3 {
        match tokio::net::lookup_host((host, port)).await {
            Ok(iter) => {
                let addrs: Vec<SocketAddr> = iter.collect();
                if !addrs.is_empty() {
                    return Ok(addrs);
                }
            }
            Err(e) => {
                if is_transient_dns_error(&e) && attempt < 2 {
                    debug!(
                        "System DNS lookup for {}:{} had transient error (attempt {}): {}. Retrying in {:?}...",
                        host, port, attempt + 1, e, backoff
                    );
                    tokio::time::sleep(backoff).await;
                    backoff *= 2;
                    last_err = Some(e);
                    continue;
                }
                return Err(e);
            }
        }
    }

    Err(last_err.unwrap_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("No IP addresses found for {}", host),
        )
    }))
}

/// Builds a standard DNS query packet (RFC 1035) for an A record.
fn build_dns_query(host: &str, query_id: u16) -> Option<Vec<u8>> {
    let mut packet = Vec::with_capacity(64);
    // Header (12 bytes)
    packet.extend_from_slice(&query_id.to_be_bytes()); // ID
    packet.extend_from_slice(&[0x01, 0x00]); // Flags: Standard query, Recursion Desired
    packet.extend_from_slice(&[0x00, 0x01]); // QDCOUNT: 1
    packet.extend_from_slice(&[0x00, 0x00]); // ANCOUNT: 0
    packet.extend_from_slice(&[0x00, 0x00]); // NSCOUNT: 0
    packet.extend_from_slice(&[0x00, 0x00]); // ARCOUNT: 0

    // Question Section: QNAME
    for label in host.split('.') {
        if label.is_empty() || label.len() > 63 {
            return None;
        }
        packet.push(label.len() as u8);
        packet.extend_from_slice(label.as_bytes());
    }
    packet.push(0); // Root label null terminator

    packet.extend_from_slice(&[0x00, 0x01]); // QTYPE: A (1)
    packet.extend_from_slice(&[0x00, 0x01]); // QCLASS: IN (1)

    Some(packet)
}

/// Parses an A record response from a standard DNS answer.
fn parse_dns_response(buf: &[u8], expected_id: u16) -> Vec<Ipv4Addr> {
    let mut results = Vec::new();
    if buf.len() < 12 {
        return results;
    }

    let id = u16::from_be_bytes([buf[0], buf[1]]);
    if id != expected_id {
        return results;
    }

    let flags = u16::from_be_bytes([buf[2], buf[3]]);
    // Check if Response bit is set and RCODE is 0 (NoError)
    if (flags & 0x8000) == 0 || (flags & 0x000F) != 0 {
        return results;
    }

    let qdcount = u16::from_be_bytes([buf[4], buf[5]]) as usize;
    let ancount = u16::from_be_bytes([buf[6], buf[7]]) as usize;

    let mut idx = 12;

    // Skip question section
    for _ in 0..qdcount {
        while idx < buf.len() {
            let len = buf[idx];
            if (len & 0xC0) == 0xC0 {
                idx += 2;
                break;
            } else if len == 0 {
                idx += 1;
                break;
            } else {
                idx += 1 + (len as usize);
            }
        }
        // Skip QTYPE (2) + QCLASS (2)
        idx += 4;
        if idx > buf.len() {
            return results;
        }
    }

    // Parse answers
    for _ in 0..ancount {
        if idx >= buf.len() {
            break;
        }
        // Skip NAME
        while idx < buf.len() {
            let len = buf[idx];
            if (len & 0xC0) == 0xC0 {
                idx += 2;
                break;
            } else if len == 0 {
                idx += 1;
                break;
            } else {
                idx += 1 + (len as usize);
            }
        }

        if idx + 10 > buf.len() {
            break;
        }

        let atype = u16::from_be_bytes([buf[idx], buf[idx + 1]]);
        let _aclass = u16::from_be_bytes([buf[idx + 2], buf[idx + 3]]);
        let _ttl = u32::from_be_bytes([buf[idx + 4], buf[idx + 5], buf[idx + 6], buf[idx + 7]]);
        let rdlength = u16::from_be_bytes([buf[idx + 8], buf[idx + 9]]) as usize;
        idx += 10;

        if atype == 1 && rdlength == 4 && idx + 4 <= buf.len() {
            let ip = Ipv4Addr::new(buf[idx], buf[idx + 1], buf[idx + 2], buf[idx + 3]);
            results.push(ip);
        }

        idx += rdlength;
    }

    results
}

/// Fallback DNS resolver using direct public DNS (Cloudflare 1.1.1.1 and Google 8.8.8.8).
async fn resolve_public_dns(host: &str, port: u16) -> Option<Vec<SocketAddr>> {
    // Generate a cryptographically random 16-bit transaction ID to defend against Kaminsky DNS cache poisoning
    let query_id = {
        let u = uuid::Uuid::new_v4();
        let b = u.as_bytes();
        u16::from_be_bytes([b[0], b[1]])
    };
    let query_packet = build_dns_query(host, query_id)?;

    let public_resolvers = [
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)), 53),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8)), 53),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(1, 0, 0, 1)), 53),
    ];

    let socket = UdpSocket::bind("0.0.0.0:0").await.ok()?;

    for resolver in public_resolvers {
        if socket.send_to(&query_packet, resolver).await.is_ok() {
            let mut buf = [0u8; 512];
            let recv_fut = socket.recv_from(&mut buf);
            if let Ok(Ok((n, _))) =
                tokio::time::timeout(Duration::from_millis(1500), recv_fut).await
            {
                let ips = parse_dns_response(&buf[..n], query_id);
                if !ips.is_empty() {
                    debug!(
                        "Resolved {} via public DNS resolver {}: {:?}",
                        host, resolver, ips
                    );
                    let addrs: Vec<SocketAddr> = ips
                        .into_iter()
                        .map(|ip| SocketAddr::new(IpAddr::V4(ip), port))
                        .collect();
                    return Some(addrs);
                }
            }
        }
    }

    None
}

/// Resolves a hostname to a list of SocketAddrs with caching, system DNS retry,
/// and public DNS fallback.
pub async fn resolve_host(host: &str, port: u16) -> Result<Vec<SocketAddr>> {
    // 1. Literal IP address check
    if let Ok(ip) = host.parse::<IpAddr>() {
        return Ok(vec![SocketAddr::new(ip, port)]);
    }

    // 2. In-memory cache check
    if let Some(cached) = get_cached_addrs(host, port) {
        return Ok(cached);
    }

    // 3. System DNS with retry for transient errors
    match resolve_system_dns(host, port).await {
        Ok(addrs) => {
            store_cached_addrs(host, &addrs, Duration::from_secs(300));
            Ok(addrs)
        }
        Err(sys_err) => {
            warn!(
                "System DNS failed for {}:{} ({}). Attempting public DNS fallback...",
                host, port, sys_err
            );
            // 4. Public DNS fallback
            if let Some(fallback_addrs) = resolve_public_dns(host, port).await {
                store_cached_addrs(host, &fallback_addrs, Duration::from_secs(300));
                return Ok(fallback_addrs);
            }
            Err(CoreError::Io(sys_err))
        }
    }
}

/// Connects to a target host and port with multi-IP Happy Eyeballs fallback and timeout.
pub async fn connect_tcp_resilient(
    host: &str,
    port: u16,
    total_timeout: Duration,
) -> Result<TcpStream> {
    let addrs = resolve_host(host, port).await?;
    if addrs.is_empty() {
        return Err(CoreError::Http(format!(
            "No resolved IP addresses available for {}",
            host
        )));
    }

    // Sort to prioritize IPv4 if dual-stack to prevent IPv6 blackhole timeouts on broken routes
    let mut sorted_addrs = addrs.clone();
    sorted_addrs.sort_by_key(|a| if a.is_ipv4() { 0 } else { 1 });

    let per_addr_timeout = (total_timeout / (sorted_addrs.len() as u32).max(1))
        .max(Duration::from_millis(1500))
        .min(Duration::from_secs(5));

    let mut last_error = None;
    for addr in &sorted_addrs {
        debug!("Attempting TCP connection to {} (addr: {})...", host, addr);
        match tokio::time::timeout(per_addr_timeout, TcpStream::connect(addr)).await {
            Ok(Ok(stream)) => {
                let _ = stream.set_nodelay(true);
                return Ok(stream);
            }
            Ok(Err(e)) => {
                debug!("Failed to connect to {}: {}", addr, e);
                last_error = Some(e);
            }
            Err(_) => {
                debug!(
                    "Connection to {} timed out after {:?}",
                    addr, per_addr_timeout
                );
                last_error = Some(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    format!("Connection to {} timed out", addr),
                ));
            }
        }
    }

    Err(CoreError::Io(last_error.unwrap_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::ConnectionRefused,
            format!(
                "Failed to connect to any resolved address for {}:{}",
                host, port
            ),
        )
    })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dns_cache_crud() {
        let host = "test.local";
        let addrs = vec![
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100)), 80),
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 101)), 80),
        ];

        store_cached_addrs(host, &addrs, Duration::from_secs(60));
        let cached = get_cached_addrs(host, 443).expect("Expected cached addresses");
        assert_eq!(cached.len(), 2);
        assert_eq!(cached[0].port(), 443);
    }

    #[test]
    fn test_build_dns_query() {
        let query = build_dns_query("example.com", 0x1234).expect("Valid query packet");
        assert!(query.len() > 12);
        assert_eq!(query[0], 0x12);
        assert_eq!(query[1], 0x34);
        assert_eq!(query[2], 0x01); // RD=1
    }

    #[tokio::test]
    async fn test_literal_ip_resolution() {
        let addrs = resolve_host("127.0.0.1", 8080)
            .await
            .expect("Resolution failed");
        assert_eq!(addrs.len(), 1);
        assert_eq!(
            addrs[0],
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080)
        );
    }
}
