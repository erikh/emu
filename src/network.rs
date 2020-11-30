use crate::error::Error;
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

pub trait NetworkManager {
    fn create_network(&self, name: &str) -> Result<Network, Error>;
    fn delete_network(&self, network: Network) -> Result<(), Error>;
    fn create_interface(&self, network: Network) -> Result<Interface, Error>;
    fn delete_interface(&self, interface: Interface) -> Result<(), Error>;
    fn bind(&self, network: Network, interface: Interface) -> Result<(), Error>;
    fn unbind(&self, network: Network, interface: Interface) -> Result<(), Error>;
}

pub struct BridgeManager {}

async fn create_netlink_interface(network: Network, s: Sender<Result<Interface, Error>>) {
    match rtnetlink::new_connection() {
        Ok(connection) => {
            match connection
                .1
                .link()
                .add()
                .veth(network.name.clone() + "-1-1", network.name.clone() + "-1-2")
                .execute()
                .await
            {
                Ok(_) => {
                    if let Err(e) = s.send(Ok(Interface {
                        name: network.name.clone() + "-1-1",
                        peer_name: network.name.clone() + "-1-2",
                    })) {
                        panic!(e);
                    }
                }
                Err(e) => {
                    if let Err(err) = s.send(Err(Error::from(e))) {
                        panic!(err);
                    }
                }
            }
        }
        Err(e) => {
            if let Err(err) = s.send(Err(Error::from(e))) {
                panic!(err);
            }
        }
    }
}

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

    fn create_interface(&self, network: Network) -> Result<Interface, Error> {
        let (s, r) = channel::<Result<Interface, Error>>();

        create_netlink_interface(network, s).await;

        match r.recv() {
            Ok(res) => res,
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
