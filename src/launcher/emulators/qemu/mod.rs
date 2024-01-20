pub mod linux;

use crate::{
    launcher,
    qmp::{Client, UnixSocket},
    storage::{DirectoryStorageHandler, StorageHandler},
};
use anyhow::{anyhow, Result};
use std::os::unix::net::UnixStream;

pub struct EmulatorController {
    dsh: DirectoryStorageHandler,
}

impl EmulatorController {
    pub fn new(dsh: DirectoryStorageHandler) -> Self {
        Self { dsh }
    }
}

impl launcher::EmulatorController for EmulatorController {
    fn shutdown(&self, name: &str) -> Result<()> {
        let _ = std::fs::remove_file(self.dsh.vm_root(name)?.join("pid"));
        match UnixStream::connect(self.dsh.monitor_path(name)?) {
            Ok(stream) => {
                let mut us = UnixSocket::new(stream)?;
                us.handshake()?;
                us.send_command("qmp_capabilities", None)?;
                us.send_command("system_powerdown", None)?;
                Ok(())
            }
            Err(_) => Err(anyhow!("{} is not running or not monitored", name)),
        }
    }
}
