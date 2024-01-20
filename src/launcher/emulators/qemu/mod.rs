pub mod linux;

use crate::{
    launcher,
    qmp::{Client, UnixSocket},
    storage::{DirectoryStorageHandler, StorageHandler},
};
use anyhow::Result;
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
        let stream = UnixStream::connect(self.dsh.monitor_path(name)?)?;
        let mut us = UnixSocket::new(stream)?;
        us.handshake()?;
        us.send_command("qmp_capabilities", None)?;
        us.send_command("system_powerdown", None)?;
        Ok(())
    }
}
