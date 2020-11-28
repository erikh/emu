use crate::error::Error;
use std::fmt;
use std::path::PathBuf;

pub trait StorageHandler: fmt::Debug {
    fn base_path(&self) -> String;
    fn vm_root(&self, name: &str) -> Option<String>;
    fn monitor_path(&self, vm_name: &str) -> Option<String>;
    fn vm_exists(&self, name: &str) -> bool;
    fn vm_list(&self) -> Result<Vec<String>, Error>;
    fn vm_path(&self, name: &str, filename: &str) -> Result<String, Error>;
    fn vm_path_exists(&self, name: &str, filename: &str) -> bool;
}

#[derive(Debug, Clone, Copy)]
pub struct DirectoryStorageHandler {
    pub basedir: &'static str,
}

impl StorageHandler for DirectoryStorageHandler {
    fn base_path(&self) -> String {
        return self.basedir.to_string();
    }

    fn vm_root(&self, name: &str) -> Option<String> {
        match PathBuf::from(self.base_path()).join(name).to_str() {
            None => None,
            Some(s) => Some(String::from(s)),
        }
    }

    fn monitor_path(&self, vm_name: &str) -> Option<String> {
        Some(String::from(
            PathBuf::from(self.vm_root(vm_name)?).join("mon").to_str()?,
        ))
    }

    fn vm_exists(&self, name: &str) -> bool {
        match self.vm_root(name) {
            Some(vmpath) => match std::fs::metadata(vmpath) {
                Ok(_) => true,
                Err(_) => false,
            },
            None => false,
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
        if name.contains("..")
            || filename.contains("..")
            || name.contains(std::path::MAIN_SEPARATOR)
            || filename.contains(std::path::MAIN_SEPARATOR)
        {
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
