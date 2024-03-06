use serde::{de::Visitor, Deserialize, Serialize};
use std::hash::{Hash, Hasher};
use std::net::{IpAddr, Ipv4Addr};

#[derive(Debug, Clone, Eq, PartialOrd, Ord)]
pub struct Address {
    pub ip: IpAddr,
    pub mask: u8,
}

impl PartialEq for Address {
    fn eq(&self, other: &Self) -> bool {
        self.ip == other.ip
    }
}

impl Hash for Address {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.ip.hash(state);
        // NOTE: we don't want to consider the mask here; we don't want collisions on ips over a
        // mask difference.
    }
}

impl Default for Address {
    fn default() -> Self {
        Self {
            ip: IpAddr::V4(Ipv4Addr::from(0)),
            mask: 0,
        }
    }
}

impl Serialize for Address {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&format!("{}/{}", self.ip, self.mask))
    }
}

struct AddressVisitor;

impl Visitor<'_> for AddressVisitor {
    type Value = Address;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("expecting a CIDR-formatted address")
    }

    fn visit_str<E>(self, v: &str) -> std::result::Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        let parts = v.split('/').collect::<Vec<&str>>();
        if parts.len() != 2 {
            return Err(E::custom("invalid CIDR"));
        }

        let ip: IpAddr = parts[0].parse().map_err(|e| E::custom(e))?;
        let mask: u8 = parts[1].parse().map_err(|e| E::custom(e))?;

        if (ip.is_ipv4() && mask > 32) || (ip.is_ipv6() && mask > 128) {
            return Err(E::custom("Mask is larger than possible for this IP class"));
        }

        Ok(Address { ip, mask })
    }
}

impl<'de> Deserialize<'de> for Address {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_str(AddressVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use std::str::FromStr;

    #[test]
    fn test_serde() -> Result<()> {
        let a: Vec<Address> = serde_json::from_str(r#"["192.168.1.1/32"]"#)?;
        assert_eq!(
            a[0],
            Address {
                ip: IpAddr::from_str("192.168.1.1")?,
                mask: 32
            }
        );

        let a: Vec<Address> = serde_json::from_str(r#"["fe80::/128"]"#)?;
        assert_eq!(
            a[0],
            Address {
                ip: IpAddr::from_str("fe80::")?,
                mask: 128
            }
        );

        let errors = vec![
            "192.168.1.1",
            "192.168.1.1/64",
            "fe80::/129",
            "fe80::",
            "hijk::",
        ];

        for error in errors {
            assert!(serde_json::from_str::<Vec<Address>>(&format!(r#"["{}"]"#, error)).is_err());
        }

        Ok(())
    }

    #[test]
    fn test_hash() -> Result<()> {
        use std::collections::{hash_map::DefaultHasher, HashMap};

        let mut addresses = vec![
            Address {
                ip: IpAddr::from_str("192.168.1.1")?,
                mask: 16,
            },
            Address {
                ip: IpAddr::from_str("192.168.1.1")?,
                mask: 32,
            },
            Address {
                ip: IpAddr::from_str("192.168.1.2")?,
                mask: 16,
            },
            Address {
                ip: IpAddr::from_str("192.168.1.3")?,
                mask: 16,
            },
        ];

        let mut hasher = DefaultHasher::new();

        assert_eq!(
            addresses[0].hash(&mut hasher),
            addresses[1].hash(&mut hasher)
        );

        let orig = addresses.clone();
        addresses.sort();
        assert_eq!(addresses, orig);

        let mut map: HashMap<Address, bool> = HashMap::default();
        for address in addresses {
            map.insert(address, true);
        }

        assert_eq!(map.len(), 3);

        for address in vec!["192.168.1.1", "192.168.1.2", "192.168.1.3"] {
            assert!(
                map.get(&Address {
                    ip: IpAddr::from_str(address)?,
                    mask: 32,
                })
                .is_some(),
                "{}",
                address
            )
        }

        Ok(())
    }
}
