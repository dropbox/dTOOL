//! Client IP extraction for rate limiting and logging.
//!
//! This module handles extracting the real client IP address from incoming
//! connections, including support for reverse proxy headers.

use axum::http::HeaderMap;
use std::collections::HashSet;
use std::net::{IpAddr, SocketAddr};

/// Extract the client IP address from headers and connection info.
///
/// M-702: Prevent rate-limit bypass by trusting x-forwarded-for only from known proxies.
/// M-737: Parse from right-to-left to prevent spoofing in multi-proxy chains.
///
/// In multi-proxy scenarios:
///   - Client connects to Proxy 1 -> Proxy 2 -> Server
///   - X-Forwarded-For: "client-ip, proxy1-ip"
///   - Attacker can prepend fake IPs to the header before hitting Proxy 1
///   - By parsing right-to-left and skipping trusted proxies, we find the actual client
pub fn extract_client_ip(
    headers: &HeaderMap,
    addr: SocketAddr,
    trusted_proxy_ips: &HashSet<IpAddr>,
) -> String {
    let peer_ip = addr.ip();

    if trusted_proxy_ips.contains(&peer_ip) {
        if let Some(forwarded) = headers.get("x-forwarded-for") {
            if let Ok(forwarded_str) = forwarded.to_str() {
                // Parse right-to-left: skip trusted proxies, return first untrusted IP
                for ip_str in forwarded_str.rsplit(',') {
                    let ip = ip_str.trim();
                    if let Ok(parsed) = ip.parse::<IpAddr>() {
                        // Skip IPs that are also trusted proxies (they added earlier hops)
                        if !trusted_proxy_ips.contains(&parsed) {
                            return parsed.to_string();
                        }
                    }
                }
                // All IPs in XFF are trusted proxies - shouldn't happen normally
                // Fall through to return peer_ip
            }
        }
    }

    peer_ip.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[test]
    fn extract_client_ip_ignores_forwarded_without_trusted_proxy() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", HeaderValue::from_static("1.2.3.4"));

        let trusted = HashSet::new();
        let addr: SocketAddr = "10.0.0.1:1234".parse().unwrap();

        assert_eq!(extract_client_ip(&headers, addr, &trusted), "10.0.0.1");
    }

    #[test]
    fn extract_client_ip_uses_rightmost_non_trusted_ip() {
        // M-737: Parse right-to-left to prevent spoofing.
        // XFF: "1.2.3.4, 5.6.7.8" - only 10.0.0.1 (peer) is trusted
        // 5.6.7.8 is rightmost and not trusted, so return it
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-forwarded-for",
            HeaderValue::from_static("1.2.3.4, 5.6.7.8"),
        );

        let mut trusted = HashSet::new();
        trusted.insert("10.0.0.1".parse::<IpAddr>().unwrap());
        let addr: SocketAddr = "10.0.0.1:1234".parse().unwrap();

        assert_eq!(extract_client_ip(&headers, addr, &trusted), "5.6.7.8");
    }

    #[test]
    fn extract_client_ip_skips_trusted_proxies_in_chain() {
        // M-737: With multi-hop trusted proxies, skip all trusted to find client.
        // XFF: "1.2.3.4, 5.6.7.8" where both 10.0.0.1 (peer) and 5.6.7.8 are trusted
        // Should return 1.2.3.4 (the actual client)
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-forwarded-for",
            HeaderValue::from_static("1.2.3.4, 5.6.7.8"),
        );

        let mut trusted = HashSet::new();
        trusted.insert("10.0.0.1".parse::<IpAddr>().unwrap());
        trusted.insert("5.6.7.8".parse::<IpAddr>().unwrap());
        let addr: SocketAddr = "10.0.0.1:1234".parse().unwrap();

        assert_eq!(extract_client_ip(&headers, addr, &trusted), "1.2.3.4");
    }

    #[test]
    fn extract_client_ip_single_ip_from_trusted_proxy() {
        // Simple case: single IP in XFF from trusted proxy
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", HeaderValue::from_static("1.2.3.4"));

        let mut trusted = HashSet::new();
        trusted.insert("10.0.0.1".parse::<IpAddr>().unwrap());
        let addr: SocketAddr = "10.0.0.1:1234".parse().unwrap();

        assert_eq!(extract_client_ip(&headers, addr, &trusted), "1.2.3.4");
    }

    #[test]
    fn extract_client_ip_falls_back_when_forwarded_is_invalid() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", HeaderValue::from_static("not-an-ip"));

        let mut trusted = HashSet::new();
        trusted.insert("10.0.0.1".parse::<IpAddr>().unwrap());
        let addr: SocketAddr = "10.0.0.1:1234".parse().unwrap();

        assert_eq!(extract_client_ip(&headers, addr, &trusted), "10.0.0.1");
    }

    #[test]
    fn extract_client_ip_falls_back_when_all_xff_are_trusted() {
        // Edge case: all IPs in XFF are trusted (shouldn't happen in practice)
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-forwarded-for",
            HeaderValue::from_static("5.6.7.8, 9.10.11.12"),
        );

        let mut trusted = HashSet::new();
        trusted.insert("10.0.0.1".parse::<IpAddr>().unwrap());
        trusted.insert("5.6.7.8".parse::<IpAddr>().unwrap());
        trusted.insert("9.10.11.12".parse::<IpAddr>().unwrap());
        let addr: SocketAddr = "10.0.0.1:1234".parse().unwrap();

        // All XFF IPs are trusted, fall back to peer IP
        assert_eq!(extract_client_ip(&headers, addr, &trusted), "10.0.0.1");
    }
}
