use crate::launcher::EmulatorLauncher;
use crate::storage::DirectoryStorageHandler;
use crate::{error::Error, storage::SystemdStorage};
use serde::Serialize;
use tinytemplate::TinyTemplate;

const EMU_DEFAULT_PATH: &str = "/bin/emu";

const SYSTEMD_UNIT: &str = "
[Unit]
Description=Virtual Machine: {vm_name}

[Service]
Type=simple
ExecStart={command} {{for value in args}}{value} {{ endfor }}
TimeoutStopSec=30
ExecStop={emu_path} shutdown {vm_name}
KillSignal=SIGCONT
FinalKillSignal=SIGKILL

[Install]
WantedBy=default.target
";

#[derive(Serialize)]
pub struct Data {
    vm_name: String,
    command: String,
    args: Vec<String>,
    emu_path: String,
}

impl Data {
    pub fn new(vm_name: String, command: String, args: Vec<String>) -> Self {
        Self {
            vm_name,
            command,
            args,
            emu_path: match std::env::current_exe() {
                Ok(path) => match path.to_str() {
                    Some(path) => String::from(path),
                    None => String::from(EMU_DEFAULT_PATH),
                },
                Err(_) => String::from(EMU_DEFAULT_PATH),
            },
        }
    }
}

pub struct Systemd {
    launcher: Box<dyn EmulatorLauncher>,
    storage: DirectoryStorageHandler,
    systemd_storage: SystemdStorage,
}

impl Systemd {
    pub fn new(
        launcher: Box<dyn EmulatorLauncher>,
        storage: DirectoryStorageHandler,
        systemd_storage: SystemdStorage,
    ) -> Self {
        Self {
            launcher,
            storage,
            systemd_storage,
        }
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

    pub fn write(&self, vm_name: &str, cdrom: Option<&str>) -> Result<(), Error> {
        let path = self.systemd_storage.service_filename(vm_name)?;
        let template = self.template(vm_name, cdrom)?;

        match std::fs::write(path, template) {
            Ok(_) => Ok(()),
            Err(e) => Err(Error::from(e)),
        }
    }
}
