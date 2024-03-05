use super::*;
use anyhow::{anyhow, Result};
use futures::TryStreamExt;
use futures_channel::mpsc::UnboundedReceiver;
use netlink_packet_core::NetlinkMessage;
use netlink_packet_route::RouteNetlinkMessage;
use netlink_proto::sys::SocketAddr;
use rand::prelude::*;
use rtnetlink::Handle;

use std::{
    sync::mpsc::{sync_channel, Receiver, SyncSender},
    sync::{Arc, Mutex},
};

const NAME_PREFIX: &str = "emu.";

#[derive(Debug, Clone, Default)]
pub struct NetlinkNetwork {
    network: Network,
    index: u32,
}

#[derive(Debug, Clone, Default)]
pub struct NetlinkInterface {
    peer_name: String,
    index: u32,
    interface: Interface,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum NetlinkOperation {
    CreateNetwork(String),
    DeleteNetwork(Network),
    ExistsNetwork(Network),
    CreateInterface,
    DeleteInterface(Interface),
    ExistsInterface(Interface),
    Bind(Network, Interface),
    Unbind(Interface),
    AddAddress(Interface, Address),
}

#[derive(Debug, Clone)]
pub enum NetlinkOperationResult {
    CreateNetwork(Network),
    CreateInterface(Interface),
    ExistsNetwork(bool),
    ExistsInterface(bool),
    Success,
    Error(String),
}

pub struct NetlinkAsyncNetworkManager {
    networks: NetworkCache,
    interfaces: InterfaceCache,
    connection: tokio::task::JoinHandle<()>,
    handle: Handle,
    receiver: UnboundedReceiver<(NetlinkMessage<RouteNetlinkMessage>, SocketAddr)>,
}

impl NetlinkAsyncNetworkManager {
    async fn new() -> Result<Self> {
        match rtnetlink::new_connection() {
            Ok((c, handle, r)) => Ok(Self {
                networks: Default::default(),
                interfaces: Default::default(),
                connection: tokio::spawn(c),
                handle,
                receiver: r,
            }),
            Err(e) => Err(anyhow!(e)),
        }
    }

    async fn interface(&self, interface: Interface) -> Result<NetlinkInterface> {
        self.interfaces.clone().lookup(interface.name, self).await
    }

    fn bridge_name(name: &str) -> String {
        NAME_PREFIX.to_string() + name
    }

    async fn lookup_network(&self, name: String) -> Result<NetlinkNetwork> {
        if let Some(resp) = self
            .handle
            .link()
            .get()
            .match_name(name.clone())
            .execute()
            .try_next()
            .await?
        {
            Ok(NetlinkNetwork {
                index: resp.header.index,
                network: Network { name },
            })
        } else {
            Err(anyhow!("network {} not found", name))
        }
    }

    async fn lookup_interface(&self, name: String) -> Result<NetlinkInterface> {
        let resp = self
            .handle
            .link()
            .get()
            .match_name(name.clone())
            .execute()
            .try_next()
            .await;

        match resp {
            Ok(Some(resp)) => Ok(NetlinkInterface {
                peer_name: format!("{}-peer", name.clone()),
                index: resp.header.index,
                interface: Interface {
                    name,
                    macaddr: None,
                    mtu: 1500,
                    up: false,
                    addresses: Vec::new(),
                },
            }),
            Err(e) => Err(anyhow!(e)),
            Ok(None) => Err(anyhow!("could not retrieve interface after creating it")),
        }
    }

    async fn bridge_index(&self, name: &str) -> Result<u32> {
        let resp = self
            .handle
            .link()
            .get()
            .match_name(Self::bridge_name(name))
            .execute()
            .try_next()
            .await;
        match resp {
            Ok(Some(resp)) => Ok(resp.header.index),
            Err(e) => Err(anyhow!(e)),
            Ok(None) => Err(anyhow!("could not retrieve network")),
        }
    }

    async fn create_network(&self, name: String) -> Result<NetlinkNetwork> {
        let bridge_name = Self::bridge_name(&name);

        let resp = self
            .handle
            .link()
            .add()
            .bridge(bridge_name.clone())
            .execute()
            .await;

        match resp {
            Ok(_) => {
                let index = self.bridge_index(&name).await?;
                Ok(NetlinkNetwork {
                    network: Network { name: bridge_name },
                    index,
                })
            }
            Err(e) => Err(anyhow!(e)),
        }
    }

    async fn delete_network(&self, network: Network) -> Result<()> {
        let index = self.networks.lookup(network.name, self).await?;
        let resp = self.handle.link().del(index).execute().await;
        match resp {
            Ok(_) => Ok(()),
            Err(e) => Err(anyhow!(e)),
        }
    }

    async fn exists_network(&self, network: Network) -> Result<bool> {
        Ok(self.networks.lookup(network.name, self).await.is_ok())
    }

    async fn create_interface(&self) -> Result<Interface> {
        let id = ['a'..='z', 'A'..='Z', '0'..='9']
            .iter()
            .flat_map(|x| x.clone().map(|y| y.to_string()).collect::<Vec<String>>())
            .choose_multiple(&mut rand::thread_rng(), rand::random::<usize>() % 5 + 5)
            .join("");

        let if_name = format!("emu-{}", id);
        let peer_name = format!("emu-{}-peer", id);
        let resp = self
            .handle
            .link()
            .add()
            .veth(if_name.clone(), peer_name.clone())
            .execute()
            .await;

        match resp {
            Ok(_) => self.lookup_interface(if_name).await.map(|x| x.interface),
            Err(e) => Err(anyhow!(e)),
        }
    }

    async fn delete_interface(&self, interface: Interface) -> Result<()> {
        let resp = self
            .handle
            .link()
            .del(self.interface(interface).await?.index)
            .execute()
            .await;

        match resp {
            Ok(_) => Ok(()),
            Err(e) => Err(anyhow!(e)),
        }
    }

    async fn exists_interface(&self, interface: Interface) -> Result<bool> {
        Ok(self.interfaces.lookup(interface.name, self).await.is_ok())
    }

    async fn bind(&self, network: Network, interface: Interface) -> Result<()> {
        let index = self.networks.lookup(network.name, self).await?;
        let resp = self
            .handle
            .link()
            .set(self.interface(interface).await?.index)
            .controller(index)
            .execute()
            .await;

        match resp {
            Ok(_) => Ok(()),
            Err(e) => Err(anyhow!(e)),
        }
    }

    async fn unbind(&self, interface: Interface) -> Result<()> {
        let resp = self
            .handle
            .link()
            .set(self.interface(interface).await?.index)
            .controller(0)
            .execute()
            .await;

        match resp {
            Ok(_) => Ok(()),
            Err(e) => Err(anyhow!(e)),
        }
    }

    async fn add_address(&self, interface: Interface, address: Address) -> Result<()> {
        let resp = self
            .handle
            .address()
            .add(
                self.interface(interface).await?.index,
                address.ip,
                address.mask,
            )
            .execute()
            .await;

        match resp {
            Ok(_) => Ok(()),
            Err(e) => Err(anyhow!(e)),
        }
    }
}

#[derive(Debug, Clone, Default)]
struct NetworkCache(HashMap<String, u32>);

impl NetworkCache {
    async fn lookup_add(
        &mut self,
        name: String,
        manager: &NetlinkAsyncNetworkManager,
    ) -> Result<u32> {
        Ok(match self.lookup(name.clone(), manager).await {
            Ok(res) => {
                self.0.insert(name, res);
                res
            }
            Err(_) => {
                // FIXME: sloppy
                let network = manager.create_network(name.clone()).await?;
                self.0.insert(name, network.index);
                network.index
            }
        })
    }

    async fn lookup(&self, name: String, manager: &NetlinkAsyncNetworkManager) -> Result<u32> {
        Ok(if let Some(res) = self.0.get(&name) {
            *res
        } else {
            manager.lookup_network(name.clone()).await?.index
        })
    }
}
#[derive(Debug, Clone, Default)]
struct InterfaceCache(HashMap<String, NetlinkInterface>);

impl InterfaceCache {
    async fn lookup_add(
        &mut self,
        name: String,
        manager: &NetlinkAsyncNetworkManager,
    ) -> Result<NetlinkInterface> {
        Ok(match self.lookup(name.clone(), manager).await {
            Ok(res) => {
                self.0.insert(name, res.clone());
                res
            }
            Err(_) => {
                // FIXME: sloppy
                manager.create_interface().await?;
                let res = manager.lookup_interface(name.clone()).await?;
                self.0.insert(name, res.clone());
                res
            }
        })
    }

    async fn lookup(
        &self,
        name: String,
        manager: &NetlinkAsyncNetworkManager,
    ) -> Result<NetlinkInterface> {
        if let Some(res) = self.0.get(&name) {
            Ok(res.clone())
        } else {
            manager.lookup_interface(name.clone()).await
        }
    }
}

#[derive(Clone)]
pub struct NetlinkNetworkManager {
    callout: Arc<Mutex<SyncSender<NetlinkOperation>>>,
    result: Arc<Mutex<Receiver<NetlinkOperationResult>>>,
}

impl NetlinkNetworkManager {
    pub async fn new() -> Result<Self> {
        let (cs, cr) = sync_channel(1);
        let (rs, rr) = sync_channel(1);

        tokio::spawn(async move {
            Self::monitor_calls(
                &mut NetlinkAsyncNetworkManager::new().await.unwrap(),
                cr,
                rs,
            )
            .await
        });

        Ok(Self {
            callout: Arc::new(Mutex::new(cs)),
            result: Arc::new(Mutex::new(rr)),
        })
    }

    fn make_call(&mut self, operation: NetlinkOperation) -> NetlinkOperationResult {
        self.callout
            .lock()
            .expect("Locking failure")
            .send(operation)
            .unwrap();
        match self
            .result
            .lock()
            .expect("Locking failure")
            .recv_timeout(std::time::Duration::new(1, 0))
        {
            Ok(res) => res,
            Err(e) => NetlinkOperationResult::Error(e.to_string()),
        }
    }

    async fn monitor_calls(
        manager: &mut NetlinkAsyncNetworkManager,
        cr: Receiver<NetlinkOperation>,
        rs: SyncSender<NetlinkOperationResult>,
    ) {
        while let Ok(operation) = cr.try_recv() {
            match operation {
                NetlinkOperation::CreateNetwork(name) => rs
                    .send(manager.create_network(name).await.map_or_else(
                        |e| NetlinkOperationResult::Error(e.to_string()),
                        |n| NetlinkOperationResult::CreateNetwork(n.network),
                    ))
                    .unwrap(),
                NetlinkOperation::DeleteNetwork(n) => rs
                    .send(manager.delete_network(n).await.map_or_else(
                        |e| NetlinkOperationResult::Error(e.to_string()),
                        |_| NetlinkOperationResult::Success,
                    ))
                    .unwrap(),
                NetlinkOperation::ExistsNetwork(n) => rs
                    .send(manager.exists_network(n).await.map_or_else(
                        |e| NetlinkOperationResult::Error(e.to_string()),
                        NetlinkOperationResult::ExistsNetwork,
                    ))
                    .unwrap(),
                NetlinkOperation::CreateInterface => rs
                    .send(manager.create_interface().await.map_or_else(
                        |e| NetlinkOperationResult::Error(e.to_string()),
                        NetlinkOperationResult::CreateInterface,
                    ))
                    .unwrap(),
                NetlinkOperation::DeleteInterface(i) => rs
                    .send(manager.delete_interface(i).await.map_or_else(
                        |e| NetlinkOperationResult::Error(e.to_string()),
                        |_| NetlinkOperationResult::Success,
                    ))
                    .unwrap(),
                NetlinkOperation::ExistsInterface(i) => rs
                    .send(manager.exists_interface(i).await.map_or_else(
                        |e| NetlinkOperationResult::Error(e.to_string()),
                        NetlinkOperationResult::ExistsInterface,
                    ))
                    .unwrap(),
                NetlinkOperation::Bind(n, i) => rs
                    .send(manager.bind(n, i).await.map_or_else(
                        |e| NetlinkOperationResult::Error(e.to_string()),
                        |_| NetlinkOperationResult::Success,
                    ))
                    .unwrap(),
                NetlinkOperation::Unbind(i) => rs
                    .send(manager.unbind(i).await.map_or_else(
                        |e| NetlinkOperationResult::Error(e.to_string()),
                        |_| NetlinkOperationResult::Success,
                    ))
                    .unwrap(),
                NetlinkOperation::AddAddress(i, address) => rs
                    .send(manager.add_address(i, address).await.map_or_else(
                        |e| NetlinkOperationResult::Error(e.to_string()),
                        |_| NetlinkOperationResult::Success,
                    ))
                    .unwrap(),
            }
        }
    }
}

impl Default for NetlinkNetworkManager {
    fn default() -> Self {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        runtime.block_on(Self::new()).unwrap()
    }
}

impl NetworkManager for NetlinkNetworkManager {
    fn create_network(&mut self, name: String) -> Result<Network> {
        match self.make_call(NetlinkOperation::CreateNetwork(name)) {
            NetlinkOperationResult::CreateNetwork(n) => Ok(n),
            NetlinkOperationResult::Error(e) => Err(anyhow!(e)),
            _ => Err(anyhow!("Unexpected result in netlink call")),
        }
    }

    fn delete_network(&mut self, network: Network) -> Result<()> {
        match self.make_call(NetlinkOperation::DeleteNetwork(network)) {
            NetlinkOperationResult::Success => Ok(()),
            NetlinkOperationResult::Error(e) => Err(anyhow!(e)),
            _ => Err(anyhow!("Unexpected result in netlink call")),
        }
    }

    fn exists_network(&mut self, network: Network) -> Result<bool> {
        match self.make_call(NetlinkOperation::ExistsNetwork(network)) {
            NetlinkOperationResult::ExistsNetwork(b) => Ok(b),
            NetlinkOperationResult::Error(e) => Err(anyhow!(e)),
            _ => Err(anyhow!("Unexpected result in netlink call")),
        }
    }

    fn create_interface(&mut self) -> Result<Interface> {
        match self.make_call(NetlinkOperation::CreateInterface) {
            NetlinkOperationResult::CreateInterface(i) => Ok(i),
            NetlinkOperationResult::Error(e) => Err(anyhow!(e)),
            _ => Err(anyhow!("Unexpected result in netlink call")),
        }
    }

    fn delete_interface(&mut self, interface: Interface) -> Result<()> {
        match self.make_call(NetlinkOperation::DeleteInterface(interface)) {
            NetlinkOperationResult::Success => Ok(()),
            NetlinkOperationResult::Error(e) => Err(anyhow!(e)),
            _ => Err(anyhow!("Unexpected result in netlink call")),
        }
    }

    fn exists_interface(&mut self, interface: Interface) -> Result<bool> {
        match self.make_call(NetlinkOperation::ExistsInterface(interface)) {
            NetlinkOperationResult::ExistsInterface(b) => Ok(b),
            NetlinkOperationResult::Error(e) => Err(anyhow!(e)),
            _ => Err(anyhow!("Unexpected result in netlink call")),
        }
    }

    fn unbind(&mut self, interface: Interface) -> Result<()> {
        match self.make_call(NetlinkOperation::Unbind(interface)) {
            NetlinkOperationResult::Success => Ok(()),
            NetlinkOperationResult::Error(e) => Err(anyhow!(e)),
            _ => Err(anyhow!("Unexpected result in netlink call")),
        }
    }

    fn bind(&mut self, network: Network, interface: Interface) -> Result<()> {
        match self.make_call(NetlinkOperation::Bind(network, interface)) {
            NetlinkOperationResult::Success => Ok(()),
            NetlinkOperationResult::Error(e) => Err(anyhow!(e)),
            _ => Err(anyhow!("Unexpected result in netlink call")),
        }
    }

    fn add_address(&mut self, interface: Interface, address: Address) -> Result<()> {
        match self.make_call(NetlinkOperation::AddAddress(interface, address)) {
            NetlinkOperationResult::Success => Ok(()),
            NetlinkOperationResult::Error(e) => Err(anyhow!(e)),
            _ => Err(anyhow!("Unexpected result in netlink call")),
        }
    }
}
