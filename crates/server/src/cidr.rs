use serde::Deserialize;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::str::FromStr;

/// IPv4 CIDR block using bitmask for fast containment checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cidr4 {
    network: u32,
    mask: u32,
}

impl Cidr4 {
    fn new(addr: Ipv4Addr, prefix_len: u8) -> Self {
        let mask = if prefix_len == 0 {
            0
        } else {
            !0u32 << (32 - prefix_len)
        };
        Self {
            network: u32::from(addr) & mask,
            mask,
        }
    }

    #[inline]
    fn contains_u32(self, addr: u32) -> bool {
        (addr & self.mask) == self.network
    }
}

/// IPv6 CIDR block using bitmask for fast containment checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cidr6 {
    network: u128,
    mask: u128,
}

impl Cidr6 {
    fn new(addr: Ipv6Addr, prefix_len: u8) -> Self {
        let mask = if prefix_len == 0 {
            0
        } else {
            !0u128 << (128 - prefix_len)
        };
        Self {
            network: u128::from(addr) & mask,
            mask,
        }
    }

    #[inline]
    fn contains_u128(&self, addr: u128) -> bool {
        (addr & self.mask) == self.network
    }
}

/// A CIDR block (IPv4 or IPv6) that can be parsed from strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(try_from = "String")]
pub enum Cidr {
    V4(Cidr4),
    V6(Cidr6),
}

impl Cidr {}

impl FromStr for Cidr {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (addr_str, prefix_str) = s
            .split_once('/')
            .ok_or_else(|| format!("invalid CIDR format: {s}"))?;

        let addr: IpAddr = addr_str
            .parse()
            .map_err(|_| format!("invalid IP address '{addr_str}'"))?;

        let prefix_len: u8 = prefix_str
            .parse()
            .map_err(|e| format!("invalid prefix '{prefix_str}': {e}"))?;

        match addr {
            IpAddr::V4(v4) => {
                if prefix_len > 32 {
                    return Err(format!("prefix {prefix_len} exceeds maximum 32 for IPv4"));
                }
                Ok(Cidr::V4(Cidr4::new(v4, prefix_len)))
            }
            IpAddr::V6(v6) => {
                if prefix_len > 128 {
                    return Err(format!("prefix {prefix_len} exceeds maximum 128 for IPv6"));
                }
                Ok(Cidr::V6(Cidr6::new(v6, prefix_len)))
            }
        }
    }
}

impl TryFrom<String> for Cidr {
    type Error = String;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        s.parse()
    }
}

/// A set of CIDR blocks using bitmask for fast containment checks.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(from = "Vec<Cidr>")]
pub struct CidrSet {
    v4: Vec<Cidr4>,
    v6: Vec<Cidr6>,
}

impl From<Vec<Cidr>> for CidrSet {
    fn from(cidrs: Vec<Cidr>) -> Self {
        Self::from_cidrs(cidrs)
    }
}

impl CidrSet {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a `CidrSet` from an iterator of Cidrs
    pub fn from_cidrs<I: IntoIterator<Item = Cidr>>(cidrs: I) -> Self {
        let mut set = Self::new();
        for cidr in cidrs {
            set.insert(cidr);
        }
        set
    }

    /// Insert a CIDR block into the set
    pub fn insert(&mut self, cidr: Cidr) {
        match cidr {
            Cidr::V4(c) => self.v4.push(c),
            Cidr::V6(c) => self.v6.push(c),
        }
    }

    /// Check if an IP address is contained in any CIDR in the set.
    #[must_use]
    #[inline]
    pub fn contains(&self, ip: IpAddr) -> bool {
        match ip {
            IpAddr::V4(addr) => self.contains_u32(u32::from(addr)),
            IpAddr::V6(addr) => {
                if let Some(v4) = addr.to_ipv4_mapped() {
                    self.contains_u32(u32::from(v4)) || self.contains_u128(u128::from(addr))
                } else {
                    self.contains_u128(u128::from(addr))
                }
            }
        }
    }

    /// Check if an IPv4 address (as u32) is contained in any CIDR in the set.
    #[must_use]
    #[inline]
    pub fn contains_u32(&self, ip: u32) -> bool {
        self.v4.iter().any(|cidr| cidr.contains_u32(ip))
    }

    /// Check if an IPv6 address (as u128) is contained in any CIDR in the set.
    #[must_use]
    #[inline]
    pub fn contains_u128(&self, ip: u128) -> bool {
        self.v6.iter().any(|cidr| cidr.contains_u128(ip))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_cidr_v4() {
        let cidr: Cidr = "192.168.1.0/24".parse().unwrap();
        assert!(matches!(cidr, Cidr::V4(_)));
    }

    #[test]
    fn test_parse_cidr_v6() {
        let cidr: Cidr = "2001:db8::/32".parse().unwrap();
        assert!(matches!(cidr, Cidr::V6(_)));
    }

    #[test]
    fn test_cidr_set_contains_v4() {
        let cidrs = vec![
            "192.168.1.0/24".parse().unwrap(),
            "10.0.0.0/8".parse().unwrap(),
        ];
        let set = CidrSet::from_cidrs(cidrs);

        assert!(set.contains("192.168.1.0".parse().unwrap()));
        assert!(set.contains("192.168.1.1".parse().unwrap()));
        assert!(set.contains("192.168.1.255".parse().unwrap()));
        assert!(!set.contains("192.168.2.0".parse().unwrap()));
        assert!(set.contains("10.0.0.1".parse().unwrap()));
        assert!(set.contains("10.255.255.255".parse().unwrap()));
        assert!(!set.contains("11.0.0.1".parse().unwrap()));
    }

    #[test]
    fn test_cidr_set_contains_v4_single_host() {
        let set = CidrSet::from_cidrs(vec!["10.0.0.1/32".parse().unwrap()]);

        assert!(set.contains("10.0.0.1".parse().unwrap()));
        assert!(!set.contains("10.0.0.2".parse().unwrap()));
    }

    #[test]
    fn test_cidr_set_contains_v6() {
        let set = CidrSet::from_cidrs(vec!["2001:db8::/32".parse().unwrap()]);

        assert!(set.contains("2001:db8::1".parse().unwrap()));
        assert!(set.contains("2001:db8:ffff:ffff:ffff:ffff:ffff:ffff".parse().unwrap()));
        assert!(!set.contains("2001:db9::1".parse().unwrap()));
    }

    #[test]
    fn test_cidr_set_ipv4_ipv6_mismatch() {
        let set_v4 = CidrSet::from_cidrs(vec!["192.168.1.0/24".parse().unwrap()]);
        let set_v6 = CidrSet::from_cidrs(vec!["2001:db8::/32".parse().unwrap()]);

        assert!(!set_v4.contains("2001:db8::1".parse().unwrap()));
        assert!(!set_v6.contains("192.168.1.1".parse().unwrap()));
    }

    #[test]
    fn test_invalid_prefix() {
        assert!("192.168.1.0/33".parse::<Cidr>().is_err());
        assert!("2001:db8::/129".parse::<Cidr>().is_err());
    }

    #[test]
    fn test_invalid_format() {
        assert!("192.168.1.0".parse::<Cidr>().is_err());
        assert!("not-an-ip/24".parse::<Cidr>().is_err());
    }

    #[test]
    fn test_cidr_set_empty() {
        let set = CidrSet::new();
        assert!(!set.contains("192.168.1.1".parse().unwrap()));
    }

    #[test]
    fn test_cidr_set_ipv4_mapped_ipv6() {
        let set = CidrSet::from_cidrs(vec!["172.22.0.0/16".parse().unwrap()]);

        // IPv4-mapped IPv6 address should match IPv4 CIDR
        let mapped: IpAddr = "::ffff:172.22.0.1".parse().unwrap();
        assert!(set.contains(mapped));

        let mapped_outside: IpAddr = "::ffff:10.0.0.1".parse().unwrap();
        assert!(!set.contains(mapped_outside));
    }

    #[test]
    fn test_cidr_set_mixed_v4_v6() {
        let cidrs = vec![
            "192.168.1.0/24".parse().unwrap(),
            "2001:db8::/32".parse().unwrap(),
        ];
        let set = CidrSet::from_cidrs(cidrs);

        assert!(set.contains("192.168.1.1".parse().unwrap()));
        assert!(set.contains("2001:db8::1".parse().unwrap()));
        assert!(!set.contains("10.0.0.1".parse().unwrap()));
        assert!(!set.contains("2001:db9::1".parse().unwrap()));
    }
}
