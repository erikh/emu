// all of this code and everything underneath it is dead currently. I am building it slowly as I
// solve several problems towards making it reality.

mod address;
mod netlink;
mod subnet;

use self::address::Address;
use crate::vm::VM;

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::PathBuf};

pub trait Network<'a>: Send + Clone + Default + Serialize + Deserialize<'a> {
    fn name(&self) -> String;
    fn set_name(&self, name: String) -> Self;

    // optional
    fn index(&self) -> Option<u32>;
    fn set_index(&self, index: u32) -> Self;
}

pub trait NetworkInterface: Send + Default {
    fn name(&self) -> String;
    fn addresses(&self) -> Vec<Address>;

    // optional stuff
    fn peer_name(&self) -> Option<String>;
    fn index(&self) -> Option<u32>;
    fn id(&self) -> Option<u32>;
}

#[async_trait]
pub trait NetworkManager<'a, I, N>
where
    Self: Clone,
    I: NetworkInterface,
    N: Network<'a>,
{
    async fn create_network(&self, name: &str) -> Result<N>;
    async fn delete_network(&self, network: N) -> Result<()>;
    async fn exists_network(&self, network: N) -> Result<bool>;
    async fn create_interface(&self, network: N, id: u32) -> Result<I>;
    async fn delete_interface(&self, interface: I) -> Result<()>;
    async fn exists_interface(&self, interface: I) -> Result<bool>;
    async fn bind(&self, network: N, interface: I) -> Result<()>;
    async fn unbind(&self, interface: I) -> Result<()>;
    async fn add_address(&self, interface: I, address: &Address) -> Result<()>;
}

pub trait VMInterface<N>
where
    Self: Default,
    N: Network<'static>,
{
    fn add_to_network(&self, network: N) -> Result<()>;
    fn remove_from_network(&self, network: N) -> Result<()>;
    fn configure_addresses(&self, addresses: Vec<Address>) -> Result<()>;
}

#[derive(Debug, Clone, Default)]
pub struct VMNetwork<V, I, N>
where
    N: Network<'static>,
    V: VMInterface<N>,
    I: NetworkInterface,
{
    network: N,
    vms: HashMap<String, VM>,
    vm: std::marker::PhantomData<V>,
    interface: std::marker::PhantomData<I>,
}

impl<V, T, N> VMNetwork<V, T, N>
where
    N: Network<'static>,
    V: VMInterface<N>,
    T: NetworkInterface,
{
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

    pub fn network(&self) -> N {
        self.network.clone()
    }
}

pub type NetworkMap = HashMap<String, Vec<String>>;
pub type NetworkIndexMap = HashMap<String, u32>;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NetworkConfig {
    networks: NetworkMap,
    indexes: NetworkIndexMap,
}

pub type VMNetworkMap<V, T, N> = HashMap<String, VMNetwork<V, T, N>>;

pub struct NetworkList<V, M, T, N>
where
    N: Network<'static>,
    V: VMInterface<N>,
    M: NetworkManager<'static, T, N>,
    T: NetworkInterface,
{
    networks: VMNetworkMap<V, T, N>,
    manager: M,
}

impl<V, M, T, N> NetworkList<V, M, T, N>
where
    N: Network<'static>,
    V: VMInterface<N>,
    M: NetworkManager<'static, T, N>,
    T: NetworkInterface,
{
    pub fn create(&self, _: String) -> Result<()> {
        Ok(())
    }

    pub fn teardown(&self, _: String) -> Result<()> {
        Ok(())
    }

    pub fn save(&self, filename: PathBuf) -> Result<()> {
        let mut config = NetworkConfig::default();

        for (name, network) in &self.networks {
            config.networks.insert(
                name.to_string(),
                network
                    .list()?
                    .iter()
                    .map(|n| n.name())
                    .collect::<Vec<String>>(),
            );

            config.indexes.insert(
                network.network().name(),
                network.network().index().unwrap_or_default(),
            );
        }

        Ok(std::fs::write(filename, toml::to_string(&config)?)?)
    }

    pub fn load(manager: M, filename: PathBuf) -> Result<Self> {
        let map: NetworkConfig = toml::from_str(&std::fs::read_to_string(filename)?)?;
        let mut networks = VMNetworkMap::default();

        for (key, vms) in map.networks {
            let mut tmp: HashMap<String, VM> = HashMap::default();
            for vm in vms {
                tmp.insert(vm.clone(), vm.clone().into());
            }

            networks.insert(
                key.to_string(),
                VMNetwork {
                    network: N::default()
                        .set_name(key.to_string())
                        .set_index(map.indexes.get(&key).map_or_else(|| 0, |x| *x)),
                    vms: tmp,
                    ..Default::default()
                },
            );
        }

        Ok(Self { networks, manager })
    }

    pub fn manager(&self) -> M {
        self.manager.clone()
    }
}

impl<V, M, T, N> std::ops::Deref for NetworkList<V, M, T, N>
where
    N: Network<'static>,
    V: VMInterface<N>,
    M: NetworkManager<'static, T, N>,
    T: NetworkInterface,
{
    type Target = HashMap<String, VMNetwork<V, T, N>>;

    fn deref(&self) -> &Self::Target {
        &self.networks
    }
}

impl<V, M, T, N> std::ops::DerefMut for NetworkList<V, M, T, N>
where
    N: Network<'static>,
    V: VMInterface<N>,
    M: NetworkManager<'static, T, N>,
    T: NetworkInterface,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.networks
    }
}
