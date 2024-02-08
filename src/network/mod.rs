// all of this code and everything underneath it is dead currently. I am building it slowly as I
// solve several problems towards making it reality.

mod address;
mod config;
mod interface;
mod netlink;
mod subnet;

pub use self::{
    address::Address,
    interface::{Interface, MacAddr},
    netlink::NetlinkNetworkManager,
};

use anyhow::{anyhow, Result};
use std::collections::HashMap;

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
