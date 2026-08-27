use axum::http::{HeaderMap, Request};
use std::net::{IpAddr, SocketAddr};
use tower_governor::{key_extractor::KeyExtractor, GovernorError};

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct RateLimitKeyExtractor;

impl KeyExtractor for RateLimitKeyExtractor {
    type Key = String;

    fn extract<B>(&self, req: &Request<B>) -> Result<Self::Key, GovernorError> {
        // 1. Priority: API Key (Bearer sk_...)
        if let Some(key) = extract_bearer_token(req) {
            return Ok(format!("key:{}", key));
        }

        // 2. Priority: Real IP (X-Forwarded-For -> PeerAddr)
        // Hardening for K8s/Docker environments where PeerAddr is often the gateway
        let peer_addr = req
            .extensions()
            .get::<axum::extract::ConnectInfo<SocketAddr>>()
            .map(|connect_info| connect_info.0);
        let ip =
            extract_client_ip(req.headers(), peer_addr).ok_or(GovernorError::UnableToExtractKey)?;
        Ok(format!("ip:{}", ip))
    }
}

/// Extract Bearer token specifically strictly looking for "sk_" prefix
/// This isolates merchant usage by their unique key.
fn extract_bearer_token<B>(req: &Request<B>) -> Option<String> {
    req.headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|val| val.strip_prefix("Bearer "))
        .map(|t| t.trim())
        .filter(|t| t.starts_with("sk_")) // Only accept valid-looking secret keys
        .map(|t| t.to_string())
}

/// Extract and normalize the client IP used by abuse controls.
///
/// The production edge must overwrite `CF-Connecting-IP` / `X-Forwarded-For`
/// rather than forwarding client-supplied values unchanged.
pub(crate) fn extract_client_ip(
    headers: &HeaderMap,
    peer_addr: Option<SocketAddr>,
) -> Option<IpAddr> {
    // Strategy A: Cloudflare Connecting IP (High Priority, Trusted by Deployment)
    if let Some(ip) = parse_ip_header(headers, "cf-connecting-ip") {
        return Some(ip);
    }

    // Strategy B: X-Forwarded-For (Standard for Proxies/LB/Ingress)
    if let Some(xff) = headers.get("x-forwarded-for") {
        if let Ok(xff_str) = xff.to_str() {
            if let Some(ip) = xff_str
                .split(',')
                .next()
                .and_then(|value| value.trim().parse::<IpAddr>().ok())
            {
                return Some(ip);
            }
        }
    }

    // Strategy C: Direct Connection IP (Fallback)
    // Requires app to be run with `into_make_service_with_connect_info` in main.rs
    peer_addr.map(|addr| addr.ip())
}

fn parse_ip_header(headers: &HeaderMap, name: &'static str) -> Option<IpAddr> {
    headers
        .get(name)
        .and_then(|header| header.to_str().ok())
        .and_then(|value| value.trim().parse::<IpAddr>().ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::ConnectInfo;
    use axum::http::Request;

    #[test]
    fn test_extract_key_priority() {
        // Bearer token should take precedence over IP
        let req = Request::builder()
            .header("Authorization", "Bearer sk_test_123")
            .header("X-Forwarded-For", "1.2.3.4")
            .extension(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 8080))))
            .body(())
            .unwrap();

        let extractor = RateLimitKeyExtractor;
        let key = extractor.extract(&req).unwrap();
        assert_eq!(key, "key:sk_test_123");
    }

    #[test]
    fn test_extract_key_fallback_to_xff() {
        // No Bearer token, should use X-Forwarded-For
        let req = Request::builder()
            .header("X-Forwarded-For", "10.0.0.1, 192.168.1.1")
            .extension(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 8080))))
            .body(())
            .unwrap();

        let extractor = RateLimitKeyExtractor;
        let key = extractor.extract(&req).unwrap();
        assert_eq!(key, "ip:10.0.0.1");
    }

    #[test]
    fn test_extract_key_fallback_to_peer_addr() {
        // No Bearer token, no XFF, should use PeerAddr (ConnectInfo)
        let req = Request::builder()
            .extension(ConnectInfo(SocketAddr::from(([192, 168, 0, 1], 12345))))
            .body(())
            .unwrap();

        let extractor = RateLimitKeyExtractor;
        let key = extractor.extract(&req).unwrap();
        assert_eq!(key, "ip:192.168.0.1");
    }

    #[test]
    fn test_extract_key_priority_cf_over_xff() {
        // CF-Connecting-IP should take precedence over X-Forwarded-For
        let req = Request::builder()
            .header("CF-Connecting-IP", "203.0.113.1")
            .header("X-Forwarded-For", "10.0.0.1")
            .extension(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 8080))))
            .body(())
            .unwrap();

        let extractor = RateLimitKeyExtractor;
        let key = extractor.extract(&req).unwrap();
        assert_eq!(key, "ip:203.0.113.1");
    }

    #[test]
    fn test_extract_ignores_malformed_forwarded_headers() {
        let req = Request::builder()
            .header("CF-Connecting-IP", "attacker-controlled-value")
            .header("X-Forwarded-For", "also-not-an-ip")
            .extension(ConnectInfo(SocketAddr::from(([192, 0, 2, 10], 8080))))
            .body(())
            .unwrap();

        let extractor = RateLimitKeyExtractor;
        let key = extractor.extract(&req).unwrap();
        assert_eq!(key, "ip:192.0.2.10");
    }

    #[test]
    fn test_extract_ignores_non_sk_bearer() {
        // Bearer token without sk_ prefix should be ignored, falling back to IP
        let req = Request::builder()
            .header("Authorization", "Bearer user_token_123")
            .header("X-Forwarded-For", "8.8.8.8")
            .body(())
            .unwrap();

        let extractor = RateLimitKeyExtractor;
        let key = extractor.extract(&req).unwrap();
        assert_eq!(key, "ip:8.8.8.8");
    }

    #[test]
    fn test_extract_fails_without_ip_or_key() {
        // No nothing
        let req = Request::builder().body(()).unwrap();
        let extractor = RateLimitKeyExtractor;
        // Should rely on GovernorError
        assert!(matches!(
            extractor.extract(&req),
            Err(GovernorError::UnableToExtractKey)
        ));
    }
}
