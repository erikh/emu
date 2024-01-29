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

    pub fn check_ports(&self) -> Result<()> {
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
