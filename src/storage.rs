use crate::config::Configuration;
use crate::error::Error;
use std::fmt;
use std::path::PathBuf;

const SYSTEMD_USER_DIR: &str = "systemd/user";

#[derive(Debug, Clone)]
pub struct SystemdStorage {
    basedir: PathBuf,
    storage: DirectoryStorageHandler,
}

impl Default for SystemdStorage {
    fn default() -> Self {
        Self::new(dirs::config_dir().unwrap())
    }
}

impl SystemdStorage {
    pub fn new(path: PathBuf) -> Self {
        let s = path.join(SYSTEMD_USER_DIR);
        Self {
            basedir: s,
            storage: DirectoryStorageHandler::default(),
        }
    }

    pub fn init(&self) -> Result<(), Error> {
        Ok(std::fs::create_dir_all(&self.basedir)?)
    }

    pub fn service_filename(&self, vm_name: &str) -> Result<String, Error> {
        if !self.storage.valid_filename(vm_name) {
            return Err(Error::new("invalid vm name"));
        }

        let path = self.basedir.join(format!("{}.emu.service", vm_name));
        Ok(String::from(path.to_str().unwrap()))
    }

    pub fn remove(&self, vm_name: &str) -> Result<(), Error> {
        let path = self.service_filename(vm_name)?;

        match std::fs::remove_file(path) {
            Ok(_) => Ok(()),
            Err(e) => Err(Error::from(e)),
        }
    }

    pub fn list(&self) -> Result<Vec<String>, Error> {
        let mut v: Vec<String> = Vec::new();
        for item in std::fs::read_dir(&self.basedir)? {
            match item {
                Ok(item) => {
                    let filename = String::from(item.file_name().to_str().unwrap());
                    if filename.ends_with(".emu.service") {
                        v.push(filename.replace(".emu.service", ""))
                    }
                }
                Err(_) => {}
            }
        }
        Ok(v)
    }
}

pub trait StorageHandler: fmt::Debug {
    fn base_path(&self) -> String;
    fn vm_root(&self, name: &str) -> Result<String, Error>;
    fn monitor_path(&self, vm_name: &str) -> Result<String, Error>;
    fn config(&self, vm_name: &str) -> Result<Configuration, Error>;
    fn write_config(&self, vm_name: &str, config: Configuration) -> Result<(), Error>;
    fn vm_exists(&self, name: &str) -> bool;
    fn vm_list(&self) -> Result<Vec<String>, Error>;
    fn vm_path(&self, name: &str, filename: &str) -> Result<String, Error>;
    fn vm_path_exists(&self, name: &str, filename: &str) -> bool;
    fn create_monitor(&self, vm_name: &str) -> Result<(), Error>;
    fn valid_filename(&self, name: &str) -> bool;
}

#[derive(Debug, Clone)]
pub struct DirectoryStorageHandler {
    pub basedir: String,
}

impl Default for DirectoryStorageHandler {
    fn default() -> Self {
        let dir = dirs::data_dir().unwrap_or(dirs::home_dir().unwrap());
        let root = PathBuf::from(dir).join("emu");

        std::fs::create_dir_all(root.clone()).unwrap_or(());

        Self {
            basedir: String::from(root.to_str().unwrap()),
        }
    }
}

impl StorageHandler for DirectoryStorageHandler {
    fn valid_filename(&self, name: &str) -> bool {
        !(name.contains("..") || name.contains(std::path::MAIN_SEPARATOR) || name.contains("\x00"))
    }

    fn base_path(&self) -> String {
        return self.basedir.to_string();
    }

    fn create_monitor(&self, vm_name: &str) -> Result<(), Error> {
        match self.monitor_path(vm_name) {
            Ok(_) => Ok(()),
            Err(e) => return Err(e),
        }
    }

    fn vm_root(&self, name: &str) -> Result<String, Error> {
        if !self.valid_filename(name) {
            return Err(Error::new("path contains invalid characters"));
        }

        match PathBuf::from(self.base_path()).join(name).to_str() {
            None => Err(Error::new("could not manage path")),
            Some(s) => Ok(String::from(s)),
        }
    }

    fn monitor_path(&self, vm_name: &str) -> Result<String, Error> {
        if let Some(path) = PathBuf::from(self.vm_root(vm_name)?).join("mon").to_str() {
            Ok(String::from(path))
        } else {
            Err(Error::new("could not calculate monitor path"))
        }
    }

    fn config(&self, vm_name: &str) -> Result<Configuration, Error> {
        if let Some(path) = PathBuf::from(self.vm_root(vm_name)?)
            .join("config")
            .to_str()
        {
            Ok(Configuration::from_file(path))
        } else {
            Ok(Configuration::default())
        }
    }

    fn write_config(&self, vm_name: &str, config: Configuration) -> Result<(), Error> {
        if let Some(path) = PathBuf::from(self.vm_root(vm_name)?)
            .join("config")
            .to_str()
        {
            config.to_file(path)
        } else {
            Err(Error::new("cannot construct path for vm"))
        }
    }

    fn vm_exists(&self, name: &str) -> bool {
        match self.vm_root(name) {
            Ok(vmpath) => match std::fs::metadata(vmpath) {
                Ok(_) => true,
                Err(_) => false,
            },
            Err(_) => false,
        }
    }

    fn vm_list(&self) -> Result<Vec<String>, Error> {
        match std::fs::read_dir(self.base_path()) {
            Ok(rd) => {
                let mut ret: Vec<String> = Vec::new();
                for dir in rd {
                    match dir {
                        Ok(dir) => {
                            // in this case, filenames which cannot be converted to string are silently
                            // ignored. Maybe when I give a bigger shit.
                            match dir.file_name().into_string() {
                                Ok(s) => ret.push(s),
                                Err(_) => return Err(Error::new("could not iterate base directory; some vm filenames are invalid")),
                            }
                        }
                        Err(e) => {
                            return Err(Error::new(&format!("could not iterate directory: {}", e)))
                        }
                    }
                }

                Ok(ret)
            }
            Err(e) => Err(Error::from(e)),
        }
    }

    fn vm_path(&self, name: &str, filename: &str) -> Result<String, Error> {
        if !self.valid_filename(name) || !self.valid_filename(filename) {
            return Err(Error::new("path contains invalid characters"));
        }

        match PathBuf::from(self.base_path())
            .join(name)
            .join(filename)
            .to_str()
        {
            None => Err(Error::new("could not construct path")),
            Some(s) => Ok(String::from(s)),
        }
    }

    fn vm_path_exists(&self, name: &str, filename: &str) -> bool {
        // a gross simplification of path handling in rust!
        match self.vm_path(name, filename) {
            Ok(path) => match std::fs::metadata(path) {
                Ok(_) => true,
                Err(_) => false,
            },
            Err(_) => false,
        }
    }
}
