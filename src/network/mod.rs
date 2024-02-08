// all of this code and everything underneath it is dead currently. I am building it slowly as I
// solve several problems towards making it reality.

mod address;
mod interface;
mod netlink;
mod subnet;

pub use self::{
    address::Address,
    interface::{Interface, MacAddr},
    netlink::NetlinkNetworkManager,
};

use crate::vm::VM;

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::PathBuf};

#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Network {
    name: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum NetworkManagerType {
    #[default]
    Netlink,
}

impl NetworkManagerType {
    pub fn into_manager(&self) -> Box<dyn NetworkManager> {
        match self {
            Self::Netlink => Box::<NetlinkNetworkManager>::default(),
        }
    }
}

impl std::fmt::Display for NetworkManagerType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Netlink => "netlink",
        })
    }
}

impl std::str::FromStr for NetworkManagerType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "netlink" => Ok(Self::Netlink),
            _ => Err(anyhow!("Invalid driver")),
        }
    }
}

pub trait NetworkManager {
    fn create_network(&mut self, name: String) -> Result<Network>;
    fn delete_network(&mut self, network: Network) -> Result<()>;
    fn exists_network(&mut self, network: Network) -> Result<bool>;
    fn create_interface(&mut self) -> Result<Interface>;
    fn delete_interface(&mut self, interface: Interface) -> Result<()>;
    fn exists_interface(&mut self, interface: Interface) -> Result<bool>;
    fn bind(&mut self, network: Network, interface: Interface) -> Result<()>;
    fn unbind(&mut self, interface: Interface) -> Result<()>;
    fn add_address(&mut self, interface: Interface, address: Address) -> Result<()>;
}

#[derive(Debug, Clone)]
pub struct VMNetwork {
    network: Network,
    vms: HashMap<String, VM>,
}

impl VMNetwork {
    pub fn add_vm(&mut self, vm: &VM) -> Result<()> {
        self.vms.insert(vm.name(), vm.clone());
        Ok(())
    }

    pub fn remove_vm(&mut self, vm: &VM) -> Result<()> {
        self.vms.remove(&vm.name());
        Ok(())
    }

    pub fn list(&self) -> Result<Vec<VM>> {
        Ok(self.vms.values().map(Clone::clone).collect::<Vec<VM>>())
    }

    pub fn network(&self) -> Network {
        self.network.clone()
    }
}

pub type NetworkMap = HashMap<String, Vec<String>>;
pub type NetworkIndexMap = HashMap<String, u32>;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NetworkConfig {
    networks: NetworkMap,
}

#[derive(Debug, Clone)]
pub struct NetworkList<M>
where
    M: NetworkManager,
{
    networks: NetworkMap,
    manager: M,
}

impl<M> NetworkList<M>
where
    M: NetworkManager,
{
    pub fn exists(&self, name: &str) -> bool {
        self.networks.contains_key(name)
    }

    pub fn create(&mut self, name: String) -> Result<()> {
        if self.exists(&name) {
            return Err(anyhow!("network already exists"));
        }

        self.manager.create_network(name.clone())?;
        self.networks.insert(name, vec![]);
        Ok(())
    }

    pub fn teardown(&mut self, name: String) -> Result<()> {
        if let Some(_) = self.networks.get(&name) {
            self.manager
                .delete_network(Network { name: name.clone() })?;
            self.networks.remove(&name);
            Ok(())
        } else {
            Err(anyhow!("network doesn't exist"))
        }
    }

    pub fn save(&self, filename: PathBuf) -> Result<()> {
        Ok(std::fs::write(
            filename,
            toml::to_string(&NetworkConfig {
                networks: self.networks.clone(),
            })?,
        )?)
    }

    pub fn load(manager: M, filename: PathBuf) -> Result<Self> {
        let map: NetworkConfig = toml::from_str(&std::fs::read_to_string(filename)?)?;

        Ok(Self {
            networks: map.networks,
            manager,
        })
    }

    pub fn manager(&self) -> &M {
        &self.manager
    }
}

impl<M> std::ops::Deref for NetworkList<M>
where
    M: NetworkManager,
{
    type Target = NetworkMap;

    fn deref(&self) -> &Self::Target {
        &self.networks
    }
}

impl<M> std::ops::DerefMut for NetworkList<M>
where
    M: NetworkManager,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.networks
    }
}
