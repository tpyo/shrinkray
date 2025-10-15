use axum::http::{HeaderMap, header};
use ipnet::IpNet;
use std::{net::IpAddr, str::FromStr};

/// Extension trait for `HeaderMap`.
pub trait HeaderMapExt {
    /// Returns the 'user-agent' header if present
    fn get_user_agent(&self) -> Option<String>;
    /// Returns the 'referer' header if present
    fn get_referrer(&self) -> Option<String>;
    /// Return the client IP address from the 'x-forwarded-for' header if present
    fn get_x_forwarded_for(&self, trusted_proxies: &[IpNet]) -> Option<String>;
}

impl HeaderMapExt for HeaderMap {
    fn get_user_agent(&self) -> Option<String> {
        Some(self.get(header::USER_AGENT)?.to_str().ok()?.to_string())
    }

    fn get_referrer(&self) -> Option<String> {
        Some(self.get(header::REFERER)?.to_str().ok()?.to_string())
    }

    fn get_x_forwarded_for(&self, trusted_proxies: &[IpNet]) -> Option<String> {
        let x_forwarded_for = self
            .get("x-forwarded-for")?
            .to_str()
            .ok()?
            .parse::<XForwardedForHeader>()
            .ok()?;

        // Find the first untrusted IP by iterating in reverse
        x_forwarded_for
            .0
            .iter()
            .rev()
            .find(|&ip| !trusted_proxies.iter().any(|subnet| subnet.contains(ip)))
            .or(x_forwarded_for.0.first())
            .map(std::string::ToString::to_string)
    }
}

#[derive(Debug)]
pub struct XForwardedForHeader(pub Vec<IpAddr>);

#[derive(Debug, PartialEq, Eq)]
pub struct XForwardedForParseError;

impl FromStr for XForwardedForHeader {
    type Err = XForwardedForParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let header = s
            .split(',')
            .filter_map(|s| s.trim().parse::<IpAddr>().ok())
            .collect();

        Ok(Self(header))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_x_forwarded_for_correct_ip() {
        let trusted_proxies = vec![IpNet::from_str("192.168.1.0/16").unwrap()];
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-forwarded-for",
            "203.0.113.195,2001:db8:85a3:8d3:1319:8a2e:370:7348,198.51.100.178,192.168.4.23"
                .parse()
                .unwrap(),
        );

        assert_eq!(
            headers.get_x_forwarded_for(&trusted_proxies),
            Some("198.51.100.178".to_string())
        );
    }

    #[test]
    fn test_x_forwarded_for_untrusted_ip() {
        let trusted_proxies = vec![IpNet::from_str("192.168.1.0/16").unwrap()];
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-forwarded-for",
            "198.51.100.178,64.123.123.123,192.168.4.23"
                .parse()
                .unwrap(),
        );

        assert_eq!(
            headers.get_x_forwarded_for(&trusted_proxies),
            Some("64.123.123.123".to_string())
        );
    }

    #[test]
    fn test_x_forwarded_for_single_ip() {
        let trusted_proxies = vec![];
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", "198.51.100.178".parse().unwrap());

        assert_eq!(
            headers.get_x_forwarded_for(&trusted_proxies),
            Some("198.51.100.178".to_string())
        );
    }

    #[test]
    fn test_x_forwarded_only_trusted_ips() {
        let trusted_proxies = vec![IpNet::from_str("192.168.1.0/16").unwrap()];
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-forwarded-for",
            "192.168.1.23,192.168.4.23".parse().unwrap(),
        );
        assert_eq!(
            headers.get_x_forwarded_for(&trusted_proxies),
            Some("192.168.1.23".to_string())
        );
    }

    #[test]
    fn test_x_forwarded_for_no_header() {
        let trusted_proxies = vec![];
        let headers = HeaderMap::new();
        assert_eq!(headers.get_x_forwarded_for(&trusted_proxies), None);
    }
}
