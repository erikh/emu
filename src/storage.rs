use crate::error::Error;
use std::fmt;
use std::path::PathBuf;

pub trait StorageHandler: fmt::Debug {
    fn base_path(&self) -> String;
    fn vm_root(&self, name: &str) -> Result<String, Error>;
    fn monitor_path(&self, vm_name: &str) -> Result<String, Error>;
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
        !(name.contains("..") || name.contains(std::path::MAIN_SEPARATOR) || name.contains("\0"))
    }

    fn base_path(&self) -> String {
        return self.basedir.to_string();
    }

    fn create_monitor(&self, vm_name: &str) -> Result<(), Error> {
        match self.monitor_path(vm_name) {
            Ok(path) => {
                let monitor = std::ffi::CString::new(path).unwrap();
                unsafe {
                    libc::mkfifo(monitor.as_ptr(), libc::S_IRUSR | libc::S_IWUSR);
                };
                Ok(())
            }
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
