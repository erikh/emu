use serde::{Deserialize, Serialize};
use std::{collections::HashMap, io::Write, path::PathBuf};

use anyhow::{anyhow, Result};

const DEFAULT_CPU_TYPE: &str = "host";
const DEFAULT_CPUS: u32 = 8;
const DEFAULT_MEMORY: u32 = 16384;
const DEFAULT_VGA: &str = "virtio";
const DEFAULT_SSH_PORT: u16 = 2222;
const DEFAULT_IMAGE_INTERFACE: &str = "virtio";

pub type PortMap = HashMap<String, u16>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Configuration {
    pub machine: MachineConfiguration,
    pub ports: PortMap,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MachineConfiguration {
    pub ssh_port: u16,
    pub memory: u32, // megabytes
    pub cpus: u32,
    pub cpu_type: String,
    pub vga: String,
    pub image_interface: String,
}

impl std::fmt::Display for Configuration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&toml::to_string_pretty(self).map_err(|_| std::fmt::Error::default())?)
    }
}

impl Default for Configuration {
    fn default() -> Self {
        Configuration {
            machine: MachineConfiguration {
                ssh_port: DEFAULT_SSH_PORT,
                memory: DEFAULT_MEMORY,
                cpus: DEFAULT_CPUS,
                cpu_type: DEFAULT_CPU_TYPE.to_string(),
                vga: DEFAULT_VGA.to_string(),
                image_interface: DEFAULT_IMAGE_INTERFACE.to_string(),
            },
            ports: HashMap::new(),
        }
    }
}

impl Configuration {
    pub fn is_port_conflict(&self, other: &Self) -> bool {
        for key in self.ports.keys() {
            for okey in other.ports.keys() {
                if key == okey {
                    return true;
                }
            }
        }

        return false;
    }

    pub fn from_file(filename: PathBuf) -> Self {
        std::fs::read_to_string(filename).map_or_else(
            |_| Self::default(),
            |x| toml::from_str(&x).unwrap_or_default(),
        )
    }

    pub fn to_file(&self, filename: PathBuf) -> Result<()> {
        let mut f = std::fs::File::create(filename)?;
        f.write_all(self.to_string().as_bytes())?;

        Ok(())
    }

    pub fn valid(&self) -> Result<()> {
        if self.machine.memory == 0 {
            return Err(anyhow!("No memory value set"));
        }

        if self.machine.cpus == 0 {
            return Err(anyhow!("No cpus value set"));
        }

        Ok(())
    }

    pub fn map_port(&mut self, hostport: u16, guestport: u16) {
        self.ports.insert(hostport.to_string(), guestport);
    }

    pub fn unmap_port(&mut self, hostport: u16) {
        self.ports.remove(&hostport.to_string());
    }

    pub fn set_machine_value(&mut self, key: &str, value: &str) -> Result<()> {
        match key {
            "memory" => {
                self.machine.memory = value.parse::<u32>()?;
                Ok(())
            }
            "cpus" => {
                self.machine.cpus = value.parse::<u32>()?;
                Ok(())
            }
            "vga" => {
                self.machine.vga = value.to_string();
                Ok(())
            }
            "image-interface" => {
                self.machine.image_interface = value.to_string();
                Ok(())
            }
            "cpu-type" => {
                self.machine.cpu_type = value.to_string();
                Ok(())
            }
            "ssh-port" => {
                self.machine.ssh_port = value.parse::<u16>()?;
                Ok(())
            }
            _ => Err(anyhow!("key does not exist")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use std::path::PathBuf;
    use tempfile::NamedTempFile;

    #[test]
    fn test_set_machine_value() -> Result<()> {
        let mut config = Configuration::default();
        config.set_machine_value("memory", "1024")?;
        assert_eq!(config.machine.memory, 1024);
        config.set_machine_value("cpus", "2")?;
        assert_eq!(config.machine.cpus, 2);
        config.set_machine_value("vga", "none")?;
        assert_eq!(config.machine.vga, "none");
        config.set_machine_value("image-interface", "virtio")?;
        assert_eq!(config.machine.image_interface, "virtio");
        config.set_machine_value("cpu-type", "host")?;
        assert_eq!(config.machine.cpu_type, "host");
        config.set_machine_value("ssh-port", "2222")?;
        assert_eq!(config.machine.ssh_port, 2222);
        Ok(())
    }

    #[test]
    fn test_map_unmap_ports() -> Result<()> {
        let mut config = Configuration::default();
        config.map_port(2222, 22);
        assert_eq!(config.ports.get("2222"), Some(22).as_ref());
        config.unmap_port(2222);
        assert_eq!(config.ports.get("2222"), None);

        let mut conflict1 = Configuration::default();
        let mut conflict2 = Configuration::default();

        conflict1.map_port(2222, 22);
        conflict2.map_port(2222, 22);

        assert!(conflict1.is_port_conflict(&conflict2));

        conflict2.unmap_port(2222);
        assert!(!conflict1.is_port_conflict(&conflict2));

        Ok(())
    }

    #[test]
    fn test_io() -> Result<()> {
        let tmp = NamedTempFile::new()?;
        let path = tmp.path().to_path_buf();
        // failure to read results in a default configuration
        let config = Configuration::from_file(PathBuf::from("/"));
        assert_eq!(config, Configuration::default());

        // i/o of default configuration
        Configuration::default().to_file(path.clone())?;
        let config = Configuration::from_file(path.clone());
        assert_eq!(config, Configuration::default());

        let orig = Configuration {
            machine: MachineConfiguration {
                ssh_port: 2000,
                cpu_type: Default::default(),
                cpus: 4,
                image_interface: Default::default(),
                memory: 2048,
                vga: Default::default(),
            },
            ports: Default::default(),
        };

        orig.to_file(path.clone())?;
        let new = Configuration::from_file(path);
        assert_eq!(orig, new);

        Ok(())
    }
}
