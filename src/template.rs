use crate::error::Error;
use crate::launcher::EmulatorLauncher;
use crate::storage::DirectoryStorageHandler;
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
        let args = match self
            .launcher
            .emulator_args(vm_name, cdrom, self.storage.clone())
        {
            Ok(args) => args,
            Err(e) => return Err(e),
        };

        let data = Data::new(String::from(vm_name), self.launcher.emulator_path(), args);
        match t.render("systemd", &data) {
            Ok(x) => Ok(x),
            Err(e) => Err(Error::from(e)),
        }
    }

    pub fn write(&self, vm_name: &str, cdrom: Option<&str>) -> Result<(), Error> {
        if let Some(config_dir) = dirs::config_dir() {
            // FIXME check path trav on vm_name
            let path = PathBuf::from(config_dir)
                .join(SYSTEMD_USER_DIR)
                .join(format!("{}.service", vm_name));

            let template = self.template(vm_name, cdrom)?;

            match std::fs::write(path, template) {
                Ok(_) => Ok(()),
                Err(e) => Err(Error::from(e)),
            }
        } else {
            Err(Error::new("could not locate configuration directory"))
        }
    }
}
