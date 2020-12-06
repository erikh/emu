use crate::error::Error;

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

impl Configuration {
    pub fn valid(&self) -> Result<(), Error> {
        // FIXME fill this in later
        Ok(())
    }
}
