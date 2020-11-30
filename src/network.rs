use crate::error::Error;
use async_trait::async_trait;
use network_bridge::{create_bridge, delete_bridge};
use std::sync::mpsc::{channel, Sender};

#[derive(Debug, Clone)]
pub struct Network {
    name: String,
    index: i32,
}

#[derive(Debug, Clone)]
pub struct Interface {
    name: String,
    peer_name: String,
}

#[async_trait]
pub trait NetworkManager {
    fn create_network(&self, name: &str) -> Result<Network, Error>;
    fn delete_network(&self, network: Network) -> Result<(), Error>;
    async fn create_interface(&self, network: Network) -> Result<Interface, Error>;
    fn delete_interface(&self, interface: Interface) -> Result<(), Error>;
    fn bind(&self, network: Network, interface: Interface) -> Result<(), Error>;
    fn unbind(&self, network: Network, interface: Interface) -> Result<(), Error>;
}

pub struct BridgeManager {}

#[async_trait]
impl NetworkManager for BridgeManager {
    fn create_network(&self, name: &str) -> Result<Network, Error> {
        match create_bridge(name) {
            Ok(index) => Ok(Network {
                name: String::from(name),
                index,
            }),
            Err(e) => Err(Error::from(e)),
        }
    }

    fn delete_network(&self, network: Network) -> Result<(), Error> {
        match delete_bridge(network.name.as_str()) {
            Ok(_) => Ok(()),
            Err(e) => Err(Error::from(e)),
        }
    }

    async fn create_interface(&self, network: Network) -> Result<Interface, Error> {
        match rtnetlink::new_connection() {
            Ok(connection) => {
                let (_, handle, _) = connection;

                println!("here");
                let resp = handle
                    .link()
                    .add()
                    .veth(network.name.clone() + "-1-1", network.name.clone() + "-1-2")
                    .execute()
                    .await;

                println!("here2");
                match resp {
                    Ok(_) => Ok(Interface {
                        name: network.name.clone() + "-1-1",
                        peer_name: network.name.clone() + "-1-2",
                    }),
                    Err(e) => Err(Error::from(e)),
                }
            }
            Err(e) => Err(Error::from(e)),
        }
    }

    fn delete_interface(&self, interface: Interface) -> Result<(), Error> {
        Err(Error::new("unimplemented"))
    }

    fn bind(&self, network: Network, interface: Interface) -> Result<(), Error> {
        Err(Error::new("unimplemented"))
    }

    fn unbind(&self, network: Network, interface: Interface) -> Result<(), Error> {
        Err(Error::new("unimplemented"))
    }
}
