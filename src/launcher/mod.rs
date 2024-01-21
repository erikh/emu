pub mod emulators;

use crate::storage::{DirectoryStorageHandler, StorageHandler};
use anyhow::{anyhow, Result};
use fork::{daemon, Fork};
use std::{path::PathBuf, process::Command};

pub struct RuntimeConfig {
    pub cdrom: Option<PathBuf>,
    pub extra_disk: Option<PathBuf>,
    pub headless: bool,
    pub dsh: DirectoryStorageHandler,
}

pub trait Emulator {
    fn args(&self, vm_name: &str, rc: &RuntimeConfig) -> Result<Vec<String>>;
    fn bin(&self) -> Result<String>;
}

pub trait EmulatorController {
    fn shutdown(&self, vm_name: &str) -> Result<()>;
}

pub struct Launcher {
    emu: Box<dyn Emulator>,
    rc: RuntimeConfig,
}

impl Launcher {
    pub fn new(emu: Box<dyn Emulator>, rc: RuntimeConfig) -> Self {
        Self { emu, rc }
    }

    pub fn launch(&self, vm_name: &str, detach: bool) -> Result<Option<std::process::ExitStatus>> {
        let args = self.emu.args(vm_name, &self.rc)?;
        let mut cmd = Command::new(self.emu.bin()?);
        let spawnres = if detach {
            if let Ok(Fork::Child) = daemon(false, false) {
                cmd.args(args).spawn()
            } else {
                Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "could not fork",
                ))
            }
        } else {
            cmd.args(args).spawn()
        };

        match spawnres {
            Ok(mut child) => {
                std::fs::write(
                    &self.rc.dsh.pidfile(vm_name)?,
                    format!("{}", child.id()).as_bytes(),
                )?;
                Ok(Some(child.wait()?))
            }
            Err(e) => Err(anyhow!(e)),
        }
    }
}
