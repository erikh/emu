use crate::error::Error;
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
    peer_name: String,
    index: u32,
    id: u32,
}

#[async_trait]
pub trait NetworkManager {
    async fn create_network(&self, name: &str) -> Result<Network, Error>;
    async fn delete_network(&self, network: &Network) -> Result<(), Error>;
    async fn exists_network(&self, network: &Network) -> Result<bool, Error>;
    async fn create_interface(&self, network: &Network, id: u32) -> Result<Interface, Error>;
    async fn delete_interface(&self, interface: &Interface) -> Result<(), Error>;
    async fn exists_interface(&self, interface: &Interface) -> Result<bool, Error>;
    async fn bind(&self, network: &Network, interface: &Interface) -> Result<(), Error>;
    async fn unbind(&self, interface: &Interface) -> Result<(), Error>;
}

pub struct BridgeManager {}

#[async_trait]
impl NetworkManager for BridgeManager {
    async fn create_network(&self, name: &str) -> Result<Network, Error> {
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
                            .set_name_filter(bridge_name.clone())
                            .execute()
                            .try_next()
                            .await;
                        drop(r);
                        match resp {
                            Ok(Some(resp)) => Ok(Network {
                                name: bridge_name.clone(),
                                index: resp.header.index,
                            }),
                            Err(e) => Err(Error::from(e)),
                            Ok(None) => {
                                Err(Error::new("could not retrieve network after creating it"))
                            }
                        }
                    }
                    Err(e) => {
                        drop(r);
                        Err(Error::from(e))
                    }
                }
            }
            Err(e) => Err(Error::from(e)),
        }
    }

    async fn delete_network(&self, network: &Network) -> Result<(), Error> {
        match rtnetlink::new_connection() {
            Ok(connection) => {
                let (c, handle, r) = connection;
                tokio::spawn(c);

                let resp = handle.link().del(network.index).execute().await;
                drop(r);
                match resp {
                    Ok(_) => Ok(()),
                    Err(e) => Err(Error::from(e)),
                }
            }
            Err(e) => Err(Error::from(e)),
        }
    }

    async fn exists_network(&self, network: &Network) -> Result<bool, Error> {
        match rtnetlink::new_connection() {
            Ok(connection) => {
                let (c, handle, r) = connection;
                tokio::spawn(c);

                let resp = handle
                    .link()
                    .get()
                    .match_index(network.index)
                    .set_name_filter(network.name.clone())
                    .execute()
                    .try_next()
                    .await;

                drop(r);

                match resp {
                    Ok(_) => Ok(true),
                    Err(e) => match e.clone() {
                        rtnetlink::Error::NetlinkError(ne) => match ne.code {
                            -19 => Ok(false), // no such device
                            _ => Err(Error::from(e)),
                        },
                        _ => Err(Error::from(e)),
                    },
                }
            }
            Err(e) => Err(Error::from(e)),
        }
    }

    async fn create_interface(&self, network: &Network, id: u32) -> Result<Interface, Error> {
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
                            .set_name_filter(if_name.clone())
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
                            Err(e) => Err(Error::from(e)),
                            Ok(None) => {
                                Err(Error::new("could not retrieve interface after creating it"))
                            }
                        }
                    }
                    Err(e) => {
                        drop(r);
                        Err(Error::from(e))
                    }
                }
            }
            Err(e) => Err(Error::from(e)),
        }
    }

    async fn delete_interface(&self, interface: &Interface) -> Result<(), Error> {
        match rtnetlink::new_connection() {
            Ok(connection) => {
                let (c, handle, r) = connection;
                tokio::spawn(c);

                let resp = handle.link().del(interface.index).execute().await;

                drop(r);
                match resp {
                    Ok(_) => Ok(()),
                    Err(e) => Err(Error::from(e)),
                }
            }
            Err(e) => Err(Error::from(e)),
        }
    }

    async fn exists_interface(&self, interface: &Interface) -> Result<bool, Error> {
        match rtnetlink::new_connection() {
            Ok(connection) => {
                let (c, handle, r) = connection;
                tokio::spawn(c);

                let resp = handle
                    .link()
                    .get()
                    .set_name_filter(interface.name.clone())
                    .match_index(interface.index)
                    .execute()
                    .try_next()
                    .await;
                drop(r);
                match resp {
                    Ok(_) => Ok(true),
                    Err(e) => match e.clone() {
                        rtnetlink::Error::NetlinkError(ne) => match ne.code {
                            -19 => Ok(false), // no such device
                            _ => Err(Error::from(e)),
                        },
                        _ => Err(Error::from(e)),
                    },
                }
            }
            Err(e) => Err(Error::from(e)),
        }
    }

    async fn bind(&self, network: &Network, interface: &Interface) -> Result<(), Error> {
        match rtnetlink::new_connection() {
            Ok(connection) => {
                let (c, handle, r) = connection;
                tokio::spawn(c);

                let resp = handle
                    .link()
                    .set(interface.index)
                    .master(network.index)
                    .execute()
                    .await;

                drop(r);
                match resp {
                    Ok(_) => Ok(()),
                    Err(e) => Err(Error::from(e)),
                }
            }
            Err(e) => Err(Error::from(e)),
        }
    }

    async fn unbind(&self, interface: &Interface) -> Result<(), Error> {
        match rtnetlink::new_connection() {
            Ok(connection) => {
                let (c, handle, r) = connection;
                tokio::spawn(c);

                let resp = handle.link().set(interface.index).master(0).execute().await;

                drop(r);
                match resp {
                    Ok(_) => Ok(()),
                    Err(e) => Err(Error::from(e)),
                }
            }
            Err(e) => Err(Error::from(e)),
        }
    }
}
