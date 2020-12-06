use crate::error::Error;
use ini::ini;

pub struct Configuration {
    pub memory: u32, // megabytes
    pub cpus: u32,
}

impl Default for Configuration {
    fn default() -> Self {
        Configuration {
            memory: 16384,
            cpus: 8,
        }
    }
}

fn to_u32(opt: Option<String>) -> u32 {
    opt.unwrap().parse::<u32>().unwrap()
}

impl Configuration {
    pub fn from_file(filename: &str) -> Self {
        let ini = ini!(filename);
        Self {
            memory: to_u32(ini["machine"]["memory"].clone()),
            cpus: to_u32(ini["machine"]["cpus"].clone()),
        }
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
}
