use crate::config::Configuration;
use anyhow::{anyhow, Result};
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

    pub fn init(&self) -> Result<()> {
        Ok(std::fs::create_dir_all(&self.basedir)?)
    }

    pub fn service_filename(&self, vm_name: &str) -> Result<String> {
        if !self.storage.valid_filename(vm_name) {
            return Err(anyhow!("invalid vm name"));
        }

        let path = self.basedir.join(format!("emu.{}.service", vm_name));
        Ok(path.to_str().unwrap().to_string())
    }

    pub fn remove(&self, vm_name: &str) -> Result<()> {
        let path = self.service_filename(vm_name)?;

        match std::fs::remove_file(path) {
            Ok(_) => Ok(()),
            Err(e) => Err(anyhow!(e)),
        }
    }

    pub fn list(&self) -> Result<Vec<String>> {
        let mut v: Vec<String> = Vec::new();
        for item in std::fs::read_dir(&self.basedir)? {
            match item {
                Ok(item) => {
                    let filename = item.file_name().to_str().unwrap().to_string();
                    if filename.starts_with("emu.") && filename.ends_with(".service") {
                        v.push(
                            filename
                                .trim_start_matches("emu.")
                                .trim_end_matches(".service")
                                .to_string(),
                        )
                    }
                }
                Err(_) => {}
            }
        }
        Ok(v)
    }
}

pub trait StorageHandler: std::fmt::Debug {
    fn base_path(&self) -> PathBuf;
    fn vm_root(&self, name: &str) -> Result<PathBuf>;
    fn monitor_path(&self, vm_name: &str) -> Result<PathBuf>;
    fn config(&self, vm_name: &str) -> Result<Configuration>;
    fn write_config(&self, vm_name: &str, config: Configuration) -> Result<()>;
    fn vm_exists(&self, name: &str) -> bool;
    fn vm_list(&self) -> Result<Vec<StoragePath>>;
    fn vm_path(&self, name: &str, filename: &str) -> Result<String>;
    fn vm_path_exists(&self, name: &str, filename: &str) -> bool;
    fn create_monitor(&self, vm_name: &str) -> Result<()>;
    fn valid_filename(&self, name: &str) -> bool;
}

#[derive(Clone, Debug)]
pub struct StoragePath {
    name: String,
    base: PathBuf,
}

impl std::fmt::Display for StoragePath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&format!(
            "{} ({:.2})",
            self.name,
            byte_unit::Byte::from_u128(self.size().unwrap() as u128)
                .unwrap()
                .get_appropriate_unit(byte_unit::UnitType::Decimal),
        ))
    }
}

impl StoragePath {
    pub fn with_base(&self) -> PathBuf {
        self.base.join(self.name.clone())
    }

    fn size(&self) -> Result<usize> {
        let dir = std::fs::read_dir(self.with_base())?;
        let mut total = 0;
        let mut items = Vec::new();
        let mut dirs = vec![dir];
        while let Some(dir) = dirs.pop() {
            for item in dir {
                match item {
                    Ok(item) => {
                        let meta = item.metadata()?;
                        if meta.is_file() {
                            items.push(item);
                        }
                    }
                    _ => {}
                }
            }
        }

        for item in items {
            let meta = item.metadata()?;
            total += meta.len() as usize;
        }

        Ok(total)
    }
}

#[derive(Debug, Clone)]
pub struct DirectoryStorageHandler {
    pub basedir: PathBuf,
}

impl Default for DirectoryStorageHandler {
    fn default() -> Self {
        let dir = dirs::data_dir().unwrap_or(dirs::home_dir().unwrap());
        let root = dir.join("emu");

        std::fs::create_dir_all(root.clone()).unwrap_or(());

        Self { basedir: root }
    }
}

impl StorageHandler for DirectoryStorageHandler {
    fn valid_filename(&self, name: &str) -> bool {
        !(name.contains("..") || name.contains(std::path::MAIN_SEPARATOR) || name.contains("\x00"))
    }

    fn base_path(&self) -> PathBuf {
        self.basedir.clone()
    }

    fn create_monitor(&self, vm_name: &str) -> Result<()> {
        match self.monitor_path(vm_name) {
            Ok(_) => Ok(()),
            Err(e) => return Err(e),
        }
    }

    fn vm_root(&self, name: &str) -> Result<PathBuf> {
        if !self.valid_filename(name) {
            return Err(anyhow!("path contains invalid characters"));
        }

        Ok(self.base_path().join(name))
    }

    fn monitor_path(&self, vm_name: &str) -> Result<PathBuf> {
        Ok(self.vm_root(vm_name)?.join("mon"))
    }

    fn config(&self, vm_name: &str) -> Result<Configuration> {
        Ok(Configuration::from_file(
            self.vm_root(vm_name)?.join("config"),
        ))
    }

    fn write_config(&self, vm_name: &str, config: Configuration) -> Result<()> {
        if let Some(path) = PathBuf::from(self.vm_root(vm_name)?)
            .join("config")
            .to_str()
        {
            config.to_file(path)
        } else {
            Err(anyhow!("cannot construct path for vm"))
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

    fn vm_list(&self) -> Result<Vec<StoragePath>> {
        match std::fs::read_dir(self.base_path()) {
            Ok(rd) => {
                let mut ret = Vec::new();
                for dir in rd {
                    match dir {
                        Ok(dir) => {
                            // in this case, filenames which cannot be converted to string are silently
                            // ignored. Maybe when I give a bigger shit.
                            match dir.file_name().into_string() {
                                Ok(s) => ret.push(StoragePath{name: s, base: self.base_path()}),
                                Err(_) => return Err(anyhow!("could not iterate base directory; some vm filenames are invalid")),
                            }
                        }
                        Err(e) => return Err(anyhow!("could not iterate directory: {}", e)),
                    }
                }

                Ok(ret)
            }
            Err(e) => Err(anyhow!(e)),
        }
    }

    fn vm_path(&self, name: &str, filename: &str) -> Result<String> {
        if !self.valid_filename(name) || !self.valid_filename(filename) {
            return Err(anyhow!("path contains invalid characters"));
        }

        match self.base_path().join(name).join(filename).to_str() {
            None => Err(anyhow!("could not construct path")),
            Some(s) => Ok(s.to_string()),
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
