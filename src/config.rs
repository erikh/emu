use std::collections::HashMap;

use crate::error::Error;
use crate::ini_writer::*;
use ini::ini;
use std::io::Write;

const DEFAULT_CPUS: u32 = 8;
const DEFAULT_MEMORY: u32 = 16384;
const DEFAULT_VGA: &str = "virtio";

pub type PortMap = HashMap<u16, u16>;

pub struct Configuration {
    pub memory: u32, // megabytes
    pub cpus: u32,
    pub vga: String,
    pub ports: PortMap,
}

impl Default for Configuration {
    fn default() -> Self {
        Configuration {
            memory: DEFAULT_MEMORY,
            cpus: DEFAULT_CPUS,
            vga: String::from(DEFAULT_VGA),
            ports: HashMap::new(),
        }
    }
}

fn to_u32(opt: String) -> Result<u32, Error> {
    Ok(opt.parse::<u32>()?)
}

fn exists_or_default(
    ini: &HashMap<String, HashMap<String, Option<String>>>,
    section: &str,
    key: &str,
    default: &str,
) -> String {
    if !ini.contains_key(section) || !ini[section].contains_key(key) {
        return String::from(default);
    }

    ini[section][key].clone().unwrap()
}

fn get_ports(ini: &HashMap<String, HashMap<String, Option<String>>>) -> PortMap {
    let mut pm = PortMap::new();

    if ini.contains_key("ports") {
        for (k, v) in ini["ports"].iter() {
            pm.insert(
                k.parse::<u16>().unwrap(),
                v.clone().unwrap().parse::<u16>().unwrap(),
            );
        }
    }

    pm
}

impl Configuration {
    pub fn from_file(filename: &str) -> Self {
        let ini = match ini!(safe filename) {
            Ok(ini) => ini,
            Err(_) => return Configuration::default(),
        };

        Self {
            memory: to_u32(exists_or_default(
                &ini,
                "machine",
                "memory",
                &DEFAULT_MEMORY.to_string(),
            ))
            .unwrap(),
            cpus: to_u32(exists_or_default(
                &ini,
                "machine",
                "cpus",
                &DEFAULT_CPUS.to_string(),
            ))
            .unwrap(),
            vga: exists_or_default(&ini, "machine", "vga", DEFAULT_VGA),
            ports: get_ports(&ini),
        }
    }

    pub fn to_file(&self, filename: &str) -> Result<(), Error> {
        let mut f = std::fs::File::create(filename)?;
        f.write_all(to_ini(&self.to_ini()).as_bytes())?;

        Ok(())
    }

    pub fn to_string(&self) -> String {
        to_ini(&self.to_ini())
    }

    pub fn to_ini(&self) -> Ini {
        let mut ini = Ini::new();
        let mut machine = HashMap::new();
        let mut ports = HashMap::new();

        machine.insert(String::from("memory"), Some(self.memory.to_string()));
        machine.insert(String::from("cpus"), Some(self.cpus.to_string()));
        machine.insert(String::from("vga"), Some(self.vga.clone()));
        ini.insert(String::from("machine"), machine);

        for (host, guest) in self.ports.clone() {
            ports.insert(host.to_string(), Some(guest.to_string()));
        }

        ini.insert(String::from("ports"), ports);

        ini
    }

    pub fn valid(&self) -> Result<(), Error> {
        if self.memory == 0 {
            return Err(Error::new("No memory value set"));
        }

        if self.cpus == 0 {
            return Err(Error::new("No cpus value set"));
        }

        Ok(())
    }

    pub fn check_ports(&self) -> Result<(), Error> {
        Ok(())
    }
}
