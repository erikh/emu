use super::{
    config::Configuration,
    config_storage::XDGConfigStorage,
    network::Interface,
    supervisor::{PidSupervisor, SystemdSupervisor},
    traits::{ConfigStorageHandler, SupervisorHandler, Supervisors},
};
use anyhow::Result;
use serde::{de::Visitor, Deserialize, Serialize};
use std::{fmt::Display, path::PathBuf, rc::Rc};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VM {
    name: String,
    cdrom: Option<PathBuf>,
    extra_disk: Option<PathBuf>,
    config: Configuration,
    headless: bool,
    supervisor: Supervisors,
    interfaces: Vec<Interface>,
}

impl std::hash::Hash for VM {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.name.hash(state)
    }
}

impl Display for VM {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.name())
    }
}

impl From<String> for VM {
    fn from(value: String) -> Self {
        Self::new(value, Rc::new(Box::<XDGConfigStorage>::default()))
    }
}

impl VM {
    pub fn new(name: String, storage: Rc<Box<dyn ConfigStorageHandler>>) -> Self {
        let mut obj = Self {
            name,
            ..Default::default()
        };

        if SystemdSupervisor::default().storage().exists(&obj) {
            obj.supervisor = Supervisors::Systemd;
        }

        obj.load_config(storage);
        obj
    }

    pub fn set_headless(&mut self, headless: bool) {
        self.headless = headless
    }

    pub fn headless(&self) -> bool {
        self.headless
    }

    pub fn name(&self) -> String {
        self.name.clone()
    }

    pub fn cdrom(&self) -> Option<PathBuf> {
        self.cdrom.clone()
    }

    pub fn set_cdrom(&mut self, cdrom: PathBuf) {
        self.cdrom = Some(cdrom)
    }

    pub fn extra_disk(&self) -> Option<PathBuf> {
        self.cdrom.clone()
    }

    pub fn set_extra_disk(&mut self, extra_disk: PathBuf) {
        self.extra_disk = Some(extra_disk)
    }

    pub fn config(&self) -> Configuration {
        self.config.clone()
    }

    pub fn supervisor(&self) -> Rc<Box<dyn SupervisorHandler>> {
        match self.supervisor {
            Supervisors::Systemd => Rc::new(Box::<SystemdSupervisor>::default()),
            _ => Rc::new(Box::<PidSupervisor>::default()),
        }
    }

    pub fn load_config(&mut self, storage: Rc<Box<dyn ConfigStorageHandler>>) {
        self.config = Configuration::from_file(storage.config_path(self));
    }

    pub fn save_config(&mut self, storage: Rc<Box<dyn ConfigStorageHandler>>) -> Result<()> {
        self.config.to_file(storage.config_path(self))
    }

    pub fn set_config(&mut self, config: Configuration) {
        self.config = config;
    }
}

impl Serialize for VM {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.name)
    }
}

struct VMVisitor;

impl Visitor<'_> for VMVisitor {
    type Value = VM;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("expecting a vm name")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E> {
        Ok(v.to_string().into())
    }
}

impl<'de> Deserialize<'de> for VM {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_str(VMVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config_storage::XDGConfigStorage;
    use anyhow::Result;
    use std::rc::Rc;
    use tempfile::tempdir;

    #[test]
    fn test_into() -> Result<()> {
        let vm1: VM = "vm1".to_string().into();
        assert_eq!(vm1.name(), "vm1".to_string());
        Ok(())
    }

    #[test]
    fn test_serde() -> Result<()> {
        let vm1: VM = "vm1".to_string().into();
        assert_eq!(serde_json::to_string(&vm1)?, "\"vm1\"");
        let vm1: VM = serde_json::from_str("\"vm1\"")?;
        assert_eq!(vm1.name(), "vm1".to_string());
        Ok(())
    }

    #[test]
    fn test_vm_operations() -> Result<()> {
        let dir = tempdir()?;
        let base_path = dir.path().to_path_buf();
        let storage: Rc<Box<dyn ConfigStorageHandler>> =
            Rc::new(Box::new(XDGConfigStorage::new(base_path.clone())));

        let mut vm = VM::new("vm1".to_string(), storage.clone());
        storage.create(&vm)?;

        assert!(!vm.supervisor().supervised());
        assert!(!vm.supervisor().is_active(&vm)?);
        let mut config = vm.config();
        config.machine.ssh_port = 2000;
        vm.set_config(config.clone());
        vm.save_config(storage.clone())?;
        vm.load_config(storage.clone());
        assert_eq!(vm.config(), config);

        vm.set_cdrom(PathBuf::from("/cdrom"));
        assert_eq!(vm.cdrom(), Some(PathBuf::from("/cdrom")));
        vm.set_extra_disk(PathBuf::from("/cdrom"));
        assert_eq!(vm.extra_disk(), Some(PathBuf::from("/cdrom")));
        vm.set_headless(true);
        assert!(vm.headless());

        dir.close()?;
        Ok(())
    }
}
