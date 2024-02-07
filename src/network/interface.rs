use super::address::Address;
use anyhow::anyhow;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct MacAddr([u8; 6]);

impl std::str::FromStr for MacAddr {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut buf = [0u8; 6];

        let mut last = 0;
        for (x, octet) in s.split(':').enumerate() {
            if octet.len() != 2 {
                return Err(anyhow!("invalid octet in macaddr {} at position {}", s, x));
            }

            let val = u8::from_str_radix(octet, 16)?;
            buf[x] = val;
            last = x;
        }

        if last != 5 {
            return Err(anyhow!(
                "Invalid number of octets in macaddr {} at position {}",
                s,
                last
            ));
        }

        Ok(Self(buf))
    }
}

impl std::fmt::Display for MacAddr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&format!(
            "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            self.0[0], self.0[1], self.0[2], self.0[3], self.0[4], self.0[5]
        ))
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Interface {
    pub(crate) name: String,
    pub(crate) macaddr: Option<MacAddr>,
    pub(crate) mtu: u16,
    pub(crate) up: bool,
    pub(crate) addresses: Vec<Address>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;

    #[test]
    fn test_macaddr_convert() -> Result<()> {
        assert_eq!(
            "AB:CD:EF:00:01:02".parse::<MacAddr>()?.to_string(),
            "ab:cd:ef:00:01:02",
            "case normalization",
        );

        let good = vec![
            "ab:cd:ef:00:01:02",
            "01:02:03:04:05:06",
            "00:00:00:00:00:00",
        ];

        for item in good {
            assert_eq!(item.parse::<MacAddr>()?.to_string(), item, "{}", item);
        }

        let errors = vec![
            "ab:cd:ef:gh:ij:kl",
            "ab:cd:ef",
            "ff000:ab:cd:ef:01:02",
            "0:1:2:3:4:5",
        ];

        for error in errors {
            assert!(error.parse::<MacAddr>().is_err(), "{}", error);
        }

        Ok(())
    }
}
