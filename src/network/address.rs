use serde::{de::Visitor, Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Address {
    pub ip: IpAddr,
    pub mask: u8,
}

impl std::hash::Hash for Address {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.ip.hash(state)
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
