use anyhow::{anyhow, Result};
use async_trait::async_trait;
use futures::TryStreamExt;

const NAME_PREFIX: &str = "emu.";

#[derive(Debug, Clone)]
pub struct Network {
    name: String,
    index: u32,
}

#[derive(Debug, Clone)]
pub struct Interface {
    name: String,
    #[allow(dead_code)]
    peer_name: String,
    index: u32,
    #[allow(dead_code)]
    id: u32,
}

#[async_trait]
pub trait NetworkManager {
    async fn create_network(&self, name: &str) -> Result<Network>;
    async fn delete_network(&self, network: &Network) -> Result<()>;
    async fn exists_network(&self, network: &Network) -> Result<bool>;
    async fn create_interface(&self, network: &Network, id: u32) -> Result<Interface>;
    async fn delete_interface(&self, interface: &Interface) -> Result<()>;
    async fn exists_interface(&self, interface: &Interface) -> Result<bool>;
    async fn bind(&self, network: &Network, interface: &Interface) -> Result<()>;
    async fn unbind(&self, interface: &Interface) -> Result<()>;
}

pub struct BridgeManager {}

#[async_trait]
impl NetworkManager for BridgeManager {
    async fn create_network(&self, name: &str) -> Result<Network> {
        match rtnetlink::new_connection() {
            Ok(connection) => {
                let (c, handle, r) = connection;
                tokio::spawn(c);

                let bridge_name = String::from(NAME_PREFIX) + name;

                let resp = handle
                    .link()
                    .add()
                    .bridge(bridge_name.clone())
                    .execute()
                    .await;
                match resp {
                    Ok(_) => {
                        let resp = handle
                            .link()
                            .get()
                            .match_name(bridge_name.clone())
                            .execute()
                            .try_next()
                            .await;
                        drop(r);
                        match resp {
                            Ok(Some(resp)) => Ok(Network {
                                name: bridge_name.clone(),
                                index: resp.header.index,
                            }),
                            Err(e) => Err(anyhow!(e)),
                            Ok(None) => {
                                Err(anyhow!("could not retrieve network after creating it"))
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

    async fn delete_network(&self, network: &Network) -> Result<()> {
        match rtnetlink::new_connection() {
            Ok(connection) => {
                let (c, handle, r) = connection;
                tokio::spawn(c);

                let resp = handle.link().del(network.index).execute().await;
                drop(r);
                match resp {
                    Ok(_) => Ok(()),
                    Err(e) => Err(anyhow!(e)),
                }
            }
            Err(e) => Err(anyhow!(e)),
        }
    }

    async fn exists_network(&self, network: &Network) -> Result<bool> {
        match rtnetlink::new_connection() {
            Ok(connection) => {
                let (c, handle, r) = connection;
                tokio::spawn(c);

                let resp = handle
                    .link()
                    .get()
                    .match_index(network.index)
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

    async fn create_interface(&self, network: &Network, id: u32) -> Result<Interface> {
        match rtnetlink::new_connection() {
            Ok(connection) => {
                let (c, handle, r) = connection;
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
                            Ok(Some(resp)) => Ok(Interface {
                                name: if_name,
                                peer_name: peer_name.clone(),
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

    async fn delete_interface(&self, interface: &Interface) -> Result<()> {
        match rtnetlink::new_connection() {
            Ok(connection) => {
                let (c, handle, r) = connection;
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

    async fn exists_interface(&self, interface: &Interface) -> Result<bool> {
        match rtnetlink::new_connection() {
            Ok(connection) => {
                let (c, handle, r) = connection;
                tokio::spawn(c);

                let resp = handle
                    .link()
                    .get()
                    .match_name(interface.name.clone())
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

    async fn bind(&self, network: &Network, interface: &Interface) -> Result<()> {
        match rtnetlink::new_connection() {
            Ok(connection) => {
                let (c, handle, r) = connection;
                tokio::spawn(c);

                let resp = handle
                    .link()
                    .set(interface.index)
                    .controller(network.index)
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

    async fn unbind(&self, interface: &Interface) -> Result<()> {
        match rtnetlink::new_connection() {
            Ok(connection) => {
                let (c, handle, r) = connection;
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
}
