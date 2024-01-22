use super::vm::VM;
use anyhow::Result;
use std::{fmt::Debug, path::PathBuf, process::ExitStatus, sync::Arc};

#[derive(Debug, Clone, Default)]
pub enum Supervisors {
    Systemd,
    #[default]
    Pid,
}

pub trait ImageHandler: Debug {
    fn import(&self, new_file: PathBuf, orig_file: PathBuf, format: String) -> Result<()>;
    fn create(&self, target: PathBuf, gbs: usize) -> Result<()>;
    fn remove(&self, disk: PathBuf) -> Result<()>;
    fn clone_image(&self, old: PathBuf, new: PathBuf) -> Result<()>;
}

pub trait SupervisorHandler: Debug {
    fn reload(&self) -> Result<()>;
    fn pidof(&self, vm: &VM) -> Result<u32>;
    fn is_active(&self, vm: &VM) -> Result<bool>;
    fn supervised(&self) -> bool;
    fn storage(&self) -> Arc<Box<dyn SupervisorStorageHandler>>;
    fn kind(&self) -> Supervisors;
}

pub trait SupervisorStorageHandler: Debug {
    fn service_name(&self, vm: &VM) -> String;
    fn service_filename(&self, vm: &VM) -> PathBuf;
    fn remove(&self, vm: &VM) -> Result<()>;
    fn create(&self, vm: &VM) -> Result<()>;
    fn list(&self) -> Result<Vec<String>>;
    fn exists(&self, vm: &VM) -> bool;
}

pub trait ConfigStorageHandler: Debug {
    fn base_path(&self) -> PathBuf;
    fn config_path(&self, vm: &VM) -> PathBuf;
    fn vm_root(&self, vm: &VM) -> PathBuf;
    fn monitor_path(&self, vm: &VM) -> PathBuf;
    fn write_config(&self, vm: VM) -> Result<()>;
    fn vm_exists(&self, vm: &VM) -> bool;
    fn vm_list(&self) -> Result<Vec<VM>>;
    fn vm_path(&self, vm: &VM, filename: &str) -> PathBuf;
    fn vm_path_exists(&self, vm: &VM, filename: &str) -> bool;
    fn rename(&self, old: &VM, new: &VM) -> Result<()>;
    fn disk_list(&self, vm: &VM) -> Result<Vec<PathBuf>>;
    fn pidfile(&self, vm: &VM) -> PathBuf;
    fn size(&self, vm: &VM) -> Result<usize>;
}

pub trait Launcher: Debug {
    fn launch_attached(&self, vm: &VM) -> Result<ExitStatus>;
    fn launch_detached(&self, vm: &VM) -> Result<()>;
    fn shutdown_wait(&self, vm: &VM) -> Result<ExitStatus>;
    fn shutdown_immediately(&self, vm: &VM) -> Result<()>;
}
