// all of this code and everything underneath it is dead currently. I am building it slowly as I
// solve several problems towards making it reality.

mod address;
mod netlink;
mod subnet;

use self::address::Address;
use crate::vm::VM;

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::PathBuf};

pub trait Network: Send + Clone + Default + Serialize + Deserialize<'static> {
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

pub trait NetworkManager<I, N>: Clone + Default
where
    I: NetworkInterface,
    N: Network,
{
    fn create_network(&mut self, name: String) -> Result<N>;
    fn delete_network(&mut self, network: N) -> Result<()>;
    fn exists_network(&mut self, network: N) -> Result<bool>;
    fn create_interface(&mut self, network: N, id: u32) -> Result<I>;
    fn delete_interface(&mut self, interface: I) -> Result<()>;
    fn exists_interface(&mut self, interface: I) -> Result<bool>;
    fn bind(&mut self, network: N, interface: I) -> Result<()>;
    fn unbind(&mut self, interface: I) -> Result<()>;
    fn add_address(&mut self, interface: I, address: Address) -> Result<()>;
}

pub trait VMInterface<N>: Default + Clone
where
    N: Network,
{
    fn add_to_network(&self, network: N) -> Result<()>;
    fn remove_from_network(&self, network: N) -> Result<()>;
    fn configure_addresses(&self, addresses: Vec<Address>) -> Result<()>;
}

#[derive(Debug, Clone, Default)]
pub struct VMNetwork<V, N>
where
    N: Network,
    V: VMInterface<N>,
{
    network: N,
    vms: HashMap<String, (VM, Vec<V>)>,
    vm: std::marker::PhantomData<V>,
}

impl<V, N> VMNetwork<V, N>
where
    N: Network,
    V: VMInterface<N>,
{
    pub fn add_vm(&mut self, vm: &VM, interface: V) -> Result<()> {
        self.vms.insert(vm.name(), (vm.clone(), vec![interface]));
        Ok(())
    }

    pub fn remove_vm(&mut self, vm: &VM) -> Result<()> {
        self.vms.remove(&vm.name());
        Ok(())
    }

    pub fn list(&self) -> Result<Vec<(VM, Vec<V>)>> {
        Ok(self
            .vms
            .values()
            .map(|x| (x.0.clone(), x.1.clone()))
            .collect::<Vec<(VM, Vec<V>)>>())
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

pub type VMNetworkMap<V, N> = HashMap<String, VMNetwork<V, N>>;

#[derive(Debug, Clone, Default)]
pub struct NetworkList<V, M, T, N>
where
    N: Network,
    V: VMInterface<N>,
    M: NetworkManager<T, N>,
    T: NetworkInterface,
{
    networks: VMNetworkMap<V, N>,
    manager: M,
    interface: std::marker::PhantomData<T>,
}

impl<V, M, T, N> NetworkList<V, M, T, N>
where
    N: Network,
    V: VMInterface<N>,
    M: NetworkManager<T, N>,
    T: NetworkInterface,
{
    pub fn exists(&self, name: &str) -> bool {
        self.networks.contains_key(name)
    }

    pub fn create(&mut self, name: String) -> Result<()> {
        if self.exists(&name) {
            return Err(anyhow!("network already exists"));
        }

        let network = self.manager.create_network(name.clone())?;
        self.networks.insert(
            name,
            VMNetwork {
                network,
                ..Default::default()
            },
        );
        Ok(())
    }

    pub fn teardown(&mut self, name: String) -> Result<()> {
        if let Some(network) = self.networks.get(&name) {
            self.manager.delete_network(network.network())?;
            self.networks.remove(&name);
            Ok(())
        } else {
            Err(anyhow!("network doesn't exist"))
        }
    }

    pub fn save(&self, filename: PathBuf) -> Result<()> {
        let mut config = NetworkConfig::default();

        for (name, network) in &self.networks {
            config.networks.insert(
                name.to_string(),
                network
                    .list()?
                    .iter()
                    .map(|n| n.0.name())
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
            let mut tmp: HashMap<String, (VM, Vec<V>)> = HashMap::default();
            for vm in vms {
                tmp.insert(vm.clone(), (vm.clone().into(), Vec::new()));
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

        Ok(Self {
            networks,
            manager,
            ..Default::default()
        })
    }

    pub fn manager(&self) -> M {
        self.manager.clone()
    }
}

impl<V, M, T, N> std::ops::Deref for NetworkList<V, M, T, N>
where
    N: Network,
    V: VMInterface<N>,
    M: NetworkManager<T, N>,
    T: NetworkInterface,
{
    type Target = HashMap<String, VMNetwork<V, N>>;

    fn deref(&self) -> &Self::Target {
        &self.networks
    }
}

impl<V, M, T, N> std::ops::DerefMut for NetworkList<V, M, T, N>
where
    N: Network,
    V: VMInterface<N>,
    M: NetworkManager<T, N>,
    T: NetworkInterface,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.networks
    }
}
