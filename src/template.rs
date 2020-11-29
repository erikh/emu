use crate::error::Error;
use crate::launcher::EmulatorLauncher;
use crate::storage::{DirectoryStorageHandler, StorageHandler};
use serde::Serialize;
use std::path::PathBuf;
use tinytemplate::TinyTemplate;

const SYSTEMD_USER_DIR: &str = "systemd/user";

const SYSTEMD_UNIT: &str = "
[Unit]
Description=Virtual Machine: {vm_name}

[Service]
ExecStart={command} {{for value in args}}{value} {{ endfor }}

[Install]
WantedBy=default.target
";

#[derive(Serialize)]
pub struct Data {
    vm_name: String,
    command: String,
    args: Vec<String>,
}

impl Data {
    pub fn new(vm_name: String, command: String, args: Vec<String>) -> Self {
        Self {
            vm_name,
            command,
            args,
        }
    }
}

pub struct Systemd {
    launcher: Box<dyn EmulatorLauncher>,
    storage: DirectoryStorageHandler,
}

impl Systemd {
    pub fn new(launcher: Box<dyn EmulatorLauncher>, storage: DirectoryStorageHandler) -> Self {
        Self { launcher, storage }
    }

    fn template(&self, vm_name: &str, cdrom: Option<&str>) -> Result<String, Error> {
        let mut t = TinyTemplate::new();
        t.add_template("systemd", SYSTEMD_UNIT)?;
        let args = self
            .launcher
            .emulator_args(vm_name, cdrom, self.storage.clone())?;

        let data = Data::new(String::from(vm_name), self.launcher.emulator_path(), args);
        match t.render("systemd", &data) {
            Ok(x) => Ok(x),
            Err(e) => Err(Error::from(e)),
        }
    }

    fn systemd_dir(&self) -> Result<PathBuf, Error> {
        if let Some(config_dir) = dirs::config_dir() {
            Ok(PathBuf::from(config_dir).join(SYSTEMD_USER_DIR))
        } else {
            Err(Error::new("could not locate configuration directory"))
        }
    }

    pub fn service_filename(&self, vm_name: &str) -> Result<String, Error> {
        if !self.storage.valid_filename(vm_name) {
            return Err(Error::new("invalid vm name"));
        }

        let path = self.systemd_dir()?.join(format!("{}.emu.service", vm_name));
        Ok(String::from(path.to_str().unwrap()))
    }

    pub fn write(&self, vm_name: &str, cdrom: Option<&str>) -> Result<(), Error> {
        let systemd_dir = self.systemd_dir()?;

        std::fs::create_dir_all(systemd_dir)?;

        let path = self.service_filename(vm_name)?;
        let template = self.template(vm_name, cdrom)?;

        match std::fs::write(path, template) {
            Ok(_) => Ok(()),
            Err(e) => Err(Error::from(e)),
        }
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
        for item in std::fs::read_dir(self.systemd_dir()?)? {
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
