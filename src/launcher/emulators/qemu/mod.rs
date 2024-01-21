pub mod linux;

use crate::{
    launcher,
    qmp::{Client, UnixSocket},
    storage::{DirectoryStorageHandler, StorageHandler},
};
use anyhow::{anyhow, Result};
use std::{
    fs::{metadata, read_to_string, remove_file},
    os::unix::net::UnixStream,
    thread::sleep,
    time::Duration,
};

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
        match UnixStream::connect(self.dsh.monitor_path(name)?) {
            Ok(stream) => {
                let mut us = UnixSocket::new(stream)?;
                us.handshake()?;
                us.send_command("qmp_capabilities", None)?;
                us.send_command("system_powerdown", None)?;
            }
            Err(_) => return Err(anyhow!("{} is not running or not monitored", name)),
        }
        let pidfile = self.dsh.vm_root(name)?.join("pid");
        let pid = read_to_string(pidfile.clone())?.parse::<u32>()?;
        let mut total = Duration::new(0, 0);
        let amount = Duration::new(0, 50);
        while let Ok(_) = metadata(&format!("/proc/{}", pid)) {
            total += amount;
            sleep(amount);
            if amount > Duration::new(10, 0) {
                eprintln!("Waiting for qemu to quit...");
                total = Duration::new(0, 0);
            }
        }
        remove_file(pidfile)?;

        Ok(())
    }
}
