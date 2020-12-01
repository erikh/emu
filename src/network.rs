use crate::error::Error;
use async_trait::async_trait;
use futures::TryStreamExt;

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
}

#[async_trait]
pub trait NetworkManager {
    async fn create_network(&self, name: &str) -> Result<Network, Error>;
    async fn delete_network(&self, network: &Network) -> Result<(), Error>;
    async fn create_interface(&self, network: &Network) -> Result<Interface, Error>;
    fn delete_interface(&self, interface: &Interface) -> Result<(), Error>;
    async fn bind(&self, network: &Network, interface: &Interface) -> Result<(), Error>;
    fn unbind(&self, network: &Network, interface: &Interface) -> Result<(), Error>;
}

pub struct BridgeManager {}

#[async_trait]
impl NetworkManager for BridgeManager {
    async fn create_network(&self, name: &str) -> Result<Network, Error> {
        match rtnetlink::new_connection() {
            Ok(connection) => {
                let (c, handle, r) = connection;
                tokio::spawn(c);

                let resp = handle
                    .link()
                    .add()
                    .bridge(String::from(name))
                    .execute()
                    .await;
                match resp {
                    Ok(_) => {
                        let resp = handle
                            .link()
                            .get()
                            .set_name_filter(String::from(name))
                            .execute()
                            .try_next()
                            .await;
                        drop(r);
                        match resp {
                            Ok(Some(resp)) => Ok(Network {
                                name: String::from(name),
                                index: resp.header.index,
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

    async fn create_interface(&self, network: &Network) -> Result<Interface, Error> {
        match rtnetlink::new_connection() {
            Ok(connection) => {
                let (c, handle, r) = connection;
                tokio::spawn(c);

                let resp = handle
                    .link()
                    .add()
                    .veth(network.name.clone() + "-1-1", network.name.clone() + "-1-2")
                    .execute()
                    .await;

                match resp {
                    Ok(_) => {
                        let resp = handle
                            .link()
                            .get()
                            .set_name_filter(network.name.clone() + "-1-1")
                            .execute()
                            .try_next()
                            .await;

                        drop(r);

                        match resp {
                            Ok(Some(resp)) => Ok(Interface {
                                name: network.name.clone() + "-1-1",
                                peer_name: network.name.clone() + "-1-2",
                                index: resp.header.index,
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

    fn delete_interface(&self, interface: &Interface) -> Result<(), Error> {
        Err(Error::new("unimplemented"))
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

    fn unbind(&self, network: &Network, interface: &Interface) -> Result<(), Error> {
        Err(Error::new("unimplemented"))
    }
}
