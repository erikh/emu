use super::*;
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use futures::TryStreamExt;
use serde::{de::Visitor, Deserialize, Serialize};

const NAME_PREFIX: &str = "emu.";

#[derive(Debug, Clone, Default)]
pub struct NetlinkNetwork {
    name: String,
    index: Option<u32>,
}

impl Network<'_> for NetlinkNetwork {
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

    fn visit_string<E>(self, v: String) -> std::result::Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(NetlinkNetwork {
            name: v,
            index: None,
        })
    }
}

impl<'de> Deserialize<'de> for NetlinkNetwork {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_string(NetlinkNetworkVisitor)
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

#[derive(Debug, Clone, Default)]
pub struct NetlinkNetworkManager {}

impl NetlinkNetworkManager {
    fn bridge_name(name: &str) -> String {
        NAME_PREFIX.to_string() + name
    }

    async fn bridge_index(&self, name: &str) -> Result<u32> {
        match rtnetlink::new_connection() {
            Ok((c, handle, r)) => {
                tokio::spawn(c);
                let resp = handle
                    .link()
                    .get()
                    .match_name(Self::bridge_name(name))
                    .execute()
                    .try_next()
                    .await;
                drop(r);
                match resp {
                    Ok(Some(resp)) => Ok(resp.header.index),
                    Err(e) => Err(anyhow!(e)),
                    Ok(None) => Err(anyhow!("could not retrieve network")),
                }
            }
            Err(e) => Err(anyhow!(e)),
        }
    }
}

#[async_trait]
impl NetworkManager<'_, NetlinkInterface, NetlinkNetwork> for NetlinkNetworkManager
where
    Self: Clone,
{
    async fn create_network(&self, name: &str) -> Result<NetlinkNetwork> {
        match rtnetlink::new_connection() {
            Ok((c, handle, r)) => {
                tokio::spawn(c);

                let bridge_name = Self::bridge_name(name);

                let resp = handle
                    .link()
                    .add()
                    .bridge(bridge_name.clone())
                    .execute()
                    .await;
                drop(r);

                match resp {
                    Ok(_) => {
                        let index = self.bridge_index(name).await?;
                        Ok(NetlinkNetwork {
                            name: bridge_name,
                            index: Some(index),
                        })
                    }
                    Err(e) => Err(anyhow!(e)),
                }
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

        match rtnetlink::new_connection() {
            Ok((c, handle, r)) => {
                tokio::spawn(c);

                let resp = handle.link().del(network.index.unwrap()).execute().await;
                drop(r);
                match resp {
                    Ok(_) => Ok(()),
                    Err(e) => Err(anyhow!(e)),
                }
            }
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

        match rtnetlink::new_connection() {
            Ok((c, handle, r)) => {
                tokio::spawn(c);

                let resp = handle
                    .link()
                    .get()
                    .match_index(network.index.unwrap())
                    .match_name(network.name.clone())
                    .execute()
                    .try_next()
                    .await;

                drop(r);

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
            Err(e) => Err(anyhow!(e)),
        }
    }

    async fn create_interface(&self, network: NetlinkNetwork, id: u32) -> Result<NetlinkInterface> {
        match rtnetlink::new_connection() {
            Ok((c, handle, r)) => {
                tokio::spawn(c);

                let if_name = network.name.clone() + &format!("-{}", id);
                let peer_name = network.name.clone() + &format!("-{}-peer", id);
                let resp = handle
                    .link()
                    .add()
                    .veth(if_name.clone(), peer_name.clone())
                    .execute()
                    .await;

                match resp {
                    Ok(_) => {
                        let resp = handle
                            .link()
                            .get()
                            .match_name(if_name.clone())
                            .execute()
                            .try_next()
                            .await;

                        drop(r);

                        match resp {
                            Ok(Some(resp)) => Ok(NetlinkInterface {
                                name: if_name,
                                addresses: Vec::new(),
                                peer_name,
                                index: resp.header.index,
                                id,
                            }),
                            Err(e) => Err(anyhow!(e)),
                            Ok(None) => {
                                Err(anyhow!("could not retrieve interface after creating it"))
                            }
                        }
                    }
                    Err(e) => {
                        drop(r);
                        Err(anyhow!(e))
                    }
                }
            }
            Err(e) => Err(anyhow!(e)),
        }
    }

    async fn delete_interface(&self, interface: NetlinkInterface) -> Result<()> {
        match rtnetlink::new_connection() {
            Ok((c, handle, r)) => {
                tokio::spawn(c);

                let resp = handle.link().del(interface.index).execute().await;

                drop(r);
                match resp {
                    Ok(_) => Ok(()),
                    Err(e) => Err(anyhow!(e)),
                }
            }
            Err(e) => Err(anyhow!(e)),
        }
    }

    async fn exists_interface(&self, interface: NetlinkInterface) -> Result<bool> {
        match rtnetlink::new_connection() {
            Ok((c, handle, r)) => {
                tokio::spawn(c);

                let resp = handle
                    .link()
                    .get()
                    .match_name(interface.name)
                    .match_index(interface.index)
                    .execute()
                    .try_next()
                    .await;
                drop(r);
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
            Err(e) => Err(anyhow!(e)),
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

        match rtnetlink::new_connection() {
            Ok((c, handle, r)) => {
                tokio::spawn(c);

                let resp = handle
                    .link()
                    .set(interface.index)
                    .controller(network.index.unwrap())
                    .execute()
                    .await;

                drop(r);
                match resp {
                    Ok(_) => Ok(()),
                    Err(e) => Err(anyhow!(e)),
                }
            }
            Err(e) => Err(anyhow!(e)),
        }
    }

    async fn unbind(&self, interface: NetlinkInterface) -> Result<()> {
        match rtnetlink::new_connection() {
            Ok((c, handle, r)) => {
                tokio::spawn(c);

                let resp = handle
                    .link()
                    .set(interface.index)
                    .controller(0)
                    .execute()
                    .await;

                drop(r);
                match resp {
                    Ok(_) => Ok(()),
                    Err(e) => Err(anyhow!(e)),
                }
            }
            Err(e) => Err(anyhow!(e)),
        }
    }

    async fn add_address(&self, interface: NetlinkInterface, address: &Address) -> Result<()> {
        match rtnetlink::new_connection() {
            Ok((c, handle, r)) => {
                tokio::spawn(c);

                let resp = handle
                    .address()
                    .add(interface.index, address.ip, address.mask)
                    .execute()
                    .await;

                drop(r);
                match resp {
                    Ok(_) => Ok(()),
                    Err(e) => Err(anyhow!(e)),
                }
            }
            Err(e) => Err(anyhow!(e)),
        }
    }
}
