use super::*;
use anyhow::{anyhow, Result};
use futures::TryStreamExt;
use futures_channel::mpsc::UnboundedReceiver;
use netlink_packet_core::NetlinkMessage;
use netlink_packet_route::RouteNetlinkMessage;
use netlink_proto::sys::SocketAddr;
use rtnetlink::Handle;
use serde::{de::Visitor, Deserialize, Serialize};
use std::sync::{
    mpsc::{sync_channel, Receiver, SyncSender},
    Arc,
};

const NAME_PREFIX: &str = "emu.";

#[derive(Debug, Clone, Default)]
pub struct NetlinkNetwork {
    name: String,
    index: Option<u32>,
}

impl Network for NetlinkNetwork {
    fn index(&self) -> Option<u32> {
        self.index
    }

    fn name(&self) -> String {
        self.name.clone()
    }

    fn set_name(&self, name: String) -> Self {
        let mut s = self.clone();
        s.name = name;
        s
    }

    fn set_index(&self, index: u32) -> Self {
        let mut s = self.clone();
        s.index = Some(index);
        s
    }
}

impl Serialize for NetlinkNetwork {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.name)
    }
}

struct NetlinkNetworkVisitor;

impl Visitor<'_> for NetlinkNetworkVisitor {
    type Value = NetlinkNetwork;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("expecting a network name")
    }

    fn visit_str<E>(self, v: &str) -> std::result::Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(NetlinkNetwork {
            name: v.to_string(),
            index: None,
        })
    }
}

impl<'de> Deserialize<'de> for NetlinkNetwork {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_str(NetlinkNetworkVisitor)
    }
}

#[derive(Debug, Clone, Default)]
pub struct NetlinkInterface {
    name: String,
    peer_name: String,
    index: u32,
    id: u32,
    addresses: Vec<Address>,
}

impl NetworkInterface for NetlinkInterface {
    fn id(&self) -> Option<u32> {
        Some(self.id)
    }

    fn peer_name(&self) -> Option<String> {
        Some(self.peer_name.clone())
    }

    fn name(&self) -> String {
        self.name.clone()
    }

    fn index(&self) -> Option<u32> {
        Some(self.index)
    }

    fn addresses(&self) -> Vec<Address> {
        self.addresses.clone()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum NetlinkOperation<N, I>
where
    N: Network,
    I: NetworkInterface,
{
    CreateNetwork(String),
    DeleteNetwork(N),
    ExistsNetwork(N),
    CreateInterface(N, u32),
    DeleteInterface(I),
    ExistsInterface(I),
    Bind(N, I),
    Unbind(I),
    AddAddress(I, Address),
}

#[derive(Debug, Clone)]
pub enum NetlinkOperationResult<N, I>
where
    N: Network,
    I: NetworkInterface,
{
    CreateNetwork(N),
    ExistsNetwork(bool),
    CreateInterface(I),
    ExistsInterface(bool),
    Success,
    Error(String),
}

pub struct NetlinkAsyncNetworkManager {
    connection: tokio::task::JoinHandle<()>,
    handle: Handle,
    receiver: UnboundedReceiver<(NetlinkMessage<RouteNetlinkMessage>, SocketAddr)>,
}

impl NetlinkAsyncNetworkManager {
    async fn new() -> Result<Self> {
        match rtnetlink::new_connection() {
            Ok((c, handle, r)) => Ok(Self {
                connection: tokio::spawn(c),
                handle,
                receiver: r,
            }),
            Err(e) => Err(anyhow!(e)),
        }
    }

    fn bridge_name(name: &str) -> String {
        NAME_PREFIX.to_string() + name
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
                    name: bridge_name,
                    index: Some(index),
                })
            }
            Err(e) => Err(anyhow!(e)),
        }
    }

    async fn delete_network(&self, mut network: NetlinkNetwork) -> Result<()> {
        if network.index.is_none() {
            if let Ok(index) = self.bridge_index(&network.name).await {
                network.index = Some(index);
            } else {
                return Err(anyhow!("Network {} was never created", network.name));
            }
        }

        let resp = self
            .handle
            .link()
            .del(network.index.unwrap())
            .execute()
            .await;
        match resp {
            Ok(_) => Ok(()),
            Err(e) => Err(anyhow!(e)),
        }
    }

    async fn exists_network(&self, mut network: NetlinkNetwork) -> Result<bool> {
        if network.index.is_none() {
            if let Ok(index) = self.bridge_index(&network.name).await {
                network.index = Some(index);
            } else {
                return Ok(false);
            }
        }

        let resp = self
            .handle
            .link()
            .get()
            .match_index(network.index.unwrap())
            .match_name(network.name.clone())
            .execute()
            .try_next()
            .await;

        match resp {
            Ok(_) => Ok(true),
            Err(e) => match e.clone() {
                rtnetlink::Error::NetlinkError(ne) => match ne.raw_code() {
                    -19 => Ok(false), // no such device
                    _ => Err(anyhow!(e)),
                },
                _ => Err(anyhow!(e)),
            },
        }
    }

    async fn create_interface(&self, network: NetlinkNetwork, id: u32) -> Result<NetlinkInterface> {
        let if_name = network.name.clone() + &format!("-{}", id);
        let peer_name = network.name.clone() + &format!("-{}-peer", id);
        let resp = self
            .handle
            .link()
            .add()
            .veth(if_name.clone(), peer_name.clone())
            .execute()
            .await;

        match resp {
            Ok(_) => {
                let resp = self
                    .handle
                    .link()
                    .get()
                    .match_name(if_name.clone())
                    .execute()
                    .try_next()
                    .await;

                match resp {
                    Ok(Some(resp)) => Ok(NetlinkInterface {
                        name: if_name,
                        addresses: Vec::new(),
                        peer_name,
                        index: resp.header.index,
                        id,
                    }),
                    Err(e) => Err(anyhow!(e)),
                    Ok(None) => Err(anyhow!("could not retrieve interface after creating it")),
                }
            }
            Err(e) => Err(anyhow!(e)),
        }
    }

    async fn delete_interface(&self, interface: NetlinkInterface) -> Result<()> {
        let resp = self.handle.link().del(interface.index).execute().await;

        match resp {
            Ok(_) => Ok(()),
            Err(e) => Err(anyhow!(e)),
        }
    }

    async fn exists_interface(&self, interface: NetlinkInterface) -> Result<bool> {
        let resp = self
            .handle
            .link()
            .get()
            .match_name(interface.name)
            .match_index(interface.index)
            .execute()
            .try_next()
            .await;
        match resp {
            Ok(_) => Ok(true),
            Err(e) => match e.clone() {
                rtnetlink::Error::NetlinkError(ne) => match ne.raw_code() {
                    -19 => Ok(false), // no such device
                    _ => Err(anyhow!(e)),
                },
                _ => Err(anyhow!(e)),
            },
        }
    }

    async fn bind(&self, mut network: NetlinkNetwork, interface: NetlinkInterface) -> Result<()> {
        if network.index.is_none() {
            if let Ok(index) = self.bridge_index(&network.name).await {
                network.index = Some(index);
            } else {
                return Err(anyhow!("Network {} was never created", network.name));
            }
        }

        let resp = self
            .handle
            .link()
            .set(interface.index)
            .controller(network.index.unwrap())
            .execute()
            .await;

        match resp {
            Ok(_) => Ok(()),
            Err(e) => Err(anyhow!(e)),
        }
    }

    async fn unbind(&self, interface: NetlinkInterface) -> Result<()> {
        let resp = self
            .handle
            .link()
            .set(interface.index)
            .controller(0)
            .execute()
            .await;

        match resp {
            Ok(_) => Ok(()),
            Err(e) => Err(anyhow!(e)),
        }
    }

    async fn add_address(&self, interface: NetlinkInterface, address: Address) -> Result<()> {
        let resp = self
            .handle
            .address()
            .add(interface.index, address.ip, address.mask)
            .execute()
            .await;

        match resp {
            Ok(_) => Ok(()),
            Err(e) => Err(anyhow!(e)),
        }
    }
}

#[derive(Clone)]
pub struct NetlinkNetworkManager {
    callout: SyncSender<NetlinkOperation<NetlinkNetwork, NetlinkInterface>>,
    result: Arc<Receiver<NetlinkOperationResult<NetlinkNetwork, NetlinkInterface>>>,
}

impl NetlinkNetworkManager {
    async fn new() -> Result<Self> {
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
            callout: cs,
            result: Arc::new(rr),
        })
    }

    fn make_call(
        &mut self,
        operation: NetlinkOperation<NetlinkNetwork, NetlinkInterface>,
    ) -> NetlinkOperationResult<NetlinkNetwork, NetlinkInterface> {
        self.callout.send(operation).unwrap();
        match self.result.recv_timeout(std::time::Duration::new(1, 0)) {
            Ok(res) => res,
            Err(e) => NetlinkOperationResult::Error(e.to_string()),
        }
    }

    async fn monitor_calls(
        manager: &mut NetlinkAsyncNetworkManager,
        cr: Receiver<NetlinkOperation<NetlinkNetwork, NetlinkInterface>>,
        rs: SyncSender<NetlinkOperationResult<NetlinkNetwork, NetlinkInterface>>,
    ) {
        while let Ok(operation) = cr.try_recv() {
            match operation {
                NetlinkOperation::CreateNetwork(name) => rs
                    .send(manager.create_network(name).await.map_or_else(
                        |e| NetlinkOperationResult::Error(e.to_string()),
                        |n| NetlinkOperationResult::CreateNetwork(n),
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
                        |b| NetlinkOperationResult::ExistsNetwork(b),
                    ))
                    .unwrap(),
                NetlinkOperation::CreateInterface(n, index) => rs
                    .send(manager.create_interface(n, index).await.map_or_else(
                        |e| NetlinkOperationResult::Error(e.to_string()),
                        |i| NetlinkOperationResult::CreateInterface(i),
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
                        |b| NetlinkOperationResult::ExistsInterface(b),
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

impl NetworkManager<NetlinkInterface, NetlinkNetwork> for NetlinkNetworkManager {
    fn create_network(&mut self, name: String) -> Result<NetlinkNetwork> {
        match self.make_call(NetlinkOperation::CreateNetwork(name)) {
            NetlinkOperationResult::CreateNetwork(n) => Ok(n),
            NetlinkOperationResult::Error(e) => Err(anyhow!(e)),
            _ => Err(anyhow!("Unexpected result in netlink call")),
        }
    }

    fn delete_network(&mut self, network: NetlinkNetwork) -> Result<()> {
        match self.make_call(NetlinkOperation::DeleteNetwork(network)) {
            NetlinkOperationResult::Success => Ok(()),
            NetlinkOperationResult::Error(e) => Err(anyhow!(e)),
            _ => Err(anyhow!("Unexpected result in netlink call")),
        }
    }

    fn exists_network(&mut self, network: NetlinkNetwork) -> Result<bool> {
        match self.make_call(NetlinkOperation::ExistsNetwork(network)) {
            NetlinkOperationResult::ExistsNetwork(b) => Ok(b),
            NetlinkOperationResult::Error(e) => Err(anyhow!(e)),
            _ => Err(anyhow!("Unexpected result in netlink call")),
        }
    }

    fn create_interface(&mut self, network: NetlinkNetwork, id: u32) -> Result<NetlinkInterface> {
        match self.make_call(NetlinkOperation::CreateInterface(network, id)) {
            NetlinkOperationResult::CreateInterface(i) => Ok(i),
            NetlinkOperationResult::Error(e) => Err(anyhow!(e)),
            _ => Err(anyhow!("Unexpected result in netlink call")),
        }
    }

    fn delete_interface(&mut self, interface: NetlinkInterface) -> Result<()> {
        match self.make_call(NetlinkOperation::DeleteInterface(interface)) {
            NetlinkOperationResult::Success => Ok(()),
            NetlinkOperationResult::Error(e) => Err(anyhow!(e)),
            _ => Err(anyhow!("Unexpected result in netlink call")),
        }
    }

    fn exists_interface(&mut self, interface: NetlinkInterface) -> Result<bool> {
        match self.make_call(NetlinkOperation::ExistsInterface(interface)) {
            NetlinkOperationResult::ExistsInterface(b) => Ok(b),
            NetlinkOperationResult::Error(e) => Err(anyhow!(e)),
            _ => Err(anyhow!("Unexpected result in netlink call")),
        }
    }

    fn unbind(&mut self, interface: NetlinkInterface) -> Result<()> {
        match self.make_call(NetlinkOperation::Unbind(interface)) {
            NetlinkOperationResult::Success => Ok(()),
            NetlinkOperationResult::Error(e) => Err(anyhow!(e)),
            _ => Err(anyhow!("Unexpected result in netlink call")),
        }
    }

    fn bind(&mut self, network: NetlinkNetwork, interface: NetlinkInterface) -> Result<()> {
        match self.make_call(NetlinkOperation::Bind(network, interface)) {
            NetlinkOperationResult::Success => Ok(()),
            NetlinkOperationResult::Error(e) => Err(anyhow!(e)),
            _ => Err(anyhow!("Unexpected result in netlink call")),
        }
    }

    fn add_address(&mut self, interface: NetlinkInterface, address: Address) -> Result<()> {
        match self.make_call(NetlinkOperation::AddAddress(interface, address)) {
            NetlinkOperationResult::Success => Ok(()),
            NetlinkOperationResult::Error(e) => Err(anyhow!(e)),
            _ => Err(anyhow!("Unexpected result in netlink call")),
        }
    }
}
