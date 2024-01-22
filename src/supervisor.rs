use super::{
    config_storage::XDGConfigStorage,
    template::Systemd,
    traits::{ConfigStorageHandler, SupervisorHandler, SupervisorStorageHandler, Supervisors},
    vm::VM,
};
use crate::util::pid_running;
use anyhow::{anyhow, Result};
use std::{
    fs::write,
    path::PathBuf,
    process::{Command, Stdio},
    sync::Arc,
};

const SYSTEMD_USER_DIR: &str = "systemd/user";

#[derive(Debug, Clone)]
pub struct SystemdSupervisorStorage {
    basedir: PathBuf,
}

impl Default for SystemdSupervisorStorage {
    fn default() -> Self {
        let dir = dirs::config_dir().unwrap().join(SYSTEMD_USER_DIR);
        std::fs::create_dir_all(&dir).unwrap_or_default();
        Self { basedir: dir }
    }
}

impl SupervisorStorageHandler for SystemdSupervisorStorage {
    fn service_filename(&self, vm: &VM) -> PathBuf {
        self.basedir
            .join(format!("{}.service", self.service_name(vm)))
    }

    fn service_name(&self, vm: &VM) -> String {
        format!("emu.{}", vm.name())
    }

    fn remove(&self, vm: &VM) -> Result<()> {
        Ok(std::fs::remove_file(self.service_filename(vm))?)
    }

    fn list(&self) -> Result<Vec<String>> {
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

    fn create(&self, vm: &VM) -> Result<()> {
        Ok(write(
            self.service_filename(vm),
            Systemd::default().template(vm)?,
        )?)
    }
}

#[derive(Debug, Clone, Default)]
pub struct NullSupervisorStorage;

impl SupervisorStorageHandler for NullSupervisorStorage {
    fn list(&self) -> Result<Vec<String>> {
        Err(anyhow!("Null storage: cannot list services"))
    }

    fn remove(&self, _: &VM) -> Result<()> {
        Err(anyhow!("Null storage: cannot remove a service"))
    }

    fn create(&self, _: &VM) -> Result<()> {
        Err(anyhow!("Null storage: cannot create a service"))
    }

    fn service_name(&self, vm: &VM) -> String {
        vm.name()
    }

    fn service_filename(&self, vm: &VM) -> PathBuf {
        vm.name().into()
    }
}

fn systemd(mut command: Vec<&str>) -> Result<()> {
    command.insert(0, "--user");
    match Command::new("/bin/systemctl")
        .args(command)
        .stderr(Stdio::null())
        .stdout(Stdio::null())
        .status()
    {
        Ok(es) => {
            if es.success() {
                Ok(())
            } else {
                Err(anyhow!("systemctl exited uncleanly: {}", es))
            }
        }
        Err(e) => Err(anyhow!(e)),
    }
}

#[derive(Debug, Clone)]
pub struct SystemdSupervisor {
    config: Arc<Box<dyn ConfigStorageHandler>>,
}

impl Default for SystemdSupervisor {
    fn default() -> Self {
        Self {
            config: Arc::new(Box::new(XDGConfigStorage::default())),
        }
    }
}

impl SupervisorHandler for SystemdSupervisor {
    fn storage(&self) -> Arc<Box<dyn SupervisorStorageHandler>> {
        Arc::new(Box::new(NullSupervisorStorage::default()))
    }

    fn supervised(&self) -> bool {
        true
    }

    fn reload(&self) -> Result<()> {
        Err(anyhow!("PIDs cannot be reloaded"))
    }

    fn is_active(&self, vm: &VM) -> Result<bool> {
        Ok(
            systemd(vec!["is-active", &self.storage().service_name(vm), "-q"])
                .map_or_else(|_| false, |_| true),
        )
    }

    fn pidof(&self, vm: &VM) -> Result<u32> {
        Ok(std::fs::read_to_string(self.config.pidfile(vm))?.parse::<u32>()?)
    }

    fn kind(&self) -> Supervisors {
        Supervisors::Pid
    }
}

#[derive(Debug, Clone)]
pub struct PidSupervisor {
    config: Arc<Box<dyn ConfigStorageHandler>>,
}

impl Default for PidSupervisor {
    fn default() -> Self {
        Self {
            config: Arc::new(Box::new(XDGConfigStorage::default())),
        }
    }
}

impl SupervisorHandler for PidSupervisor {
    fn storage(&self) -> Arc<Box<dyn SupervisorStorageHandler>> {
        Arc::new(Box::new(NullSupervisorStorage::default()))
    }

    fn supervised(&self) -> bool {
        false
    }

    fn reload(&self) -> Result<()> {
        Err(anyhow!("PIDs cannot be reloaded"))
    }

    fn is_active(&self, vm: &VM) -> Result<bool> {
        Ok(pid_running(
            std::fs::read_to_string(self.config.pidfile(&vm))?.parse::<u32>()?,
        ))
    }

    fn pidof(&self, vm: &VM) -> Result<u32> {
        Ok(std::fs::read_to_string(self.config.pidfile(vm))?.parse::<u32>()?)
    }

    fn kind(&self) -> Supervisors {
        Supervisors::Pid
    }
}
