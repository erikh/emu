use super::{
    config_storage::XDGConfigStorage,
    supervisor::{PidSupervisor, SystemdSupervisor},
    traits::{ConfigStorageHandler, SupervisorHandler, Supervisors},
};
use crate::config::Configuration;
use serde::{de::Visitor, Deserialize, Serialize};
use std::{fmt::Display, path::PathBuf, sync::Arc};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VM {
    name: String,
    cdrom: Option<PathBuf>,
    extra_disk: Option<PathBuf>,
    config: Configuration,
    headless: bool,
    supervisor: Supervisors,
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
        Self::new(value, Arc::new(Box::new(XDGConfigStorage::default())))
    }
}

impl VM {
    pub fn new(name: String, storage: Arc<Box<dyn ConfigStorageHandler>>) -> Self {
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

    pub fn supervisor(&self) -> Arc<Box<dyn SupervisorHandler>> {
        match self.supervisor {
            Supervisors::Systemd => Arc::new(Box::new(SystemdSupervisor::default())),
            _ => Arc::new(Box::new(PidSupervisor::default())),
        }
    }

    pub fn load_config(&mut self, storage: Arc<Box<dyn ConfigStorageHandler>>) {
        self.config = Configuration::from_file(storage.config_path(self));
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

    fn visit_string<E>(self, v: String) -> Result<Self::Value, E> {
        Ok(v.into())
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
