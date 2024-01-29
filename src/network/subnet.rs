use super::address::Address;
use crate::vm::VM;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub type SubnetList = HashMap<Address, Subnet>;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Subnet {
    id: u32,
    names: HashMap<VM, Vec<Address>>,
    addresses: HashMap<Address, VM>,
    network: Address,
}

impl Subnet {
    pub fn new(id: u32, network: Address) -> Self {
        Self {
            id,
            network,
            ..Default::default()
        }
    }

    pub fn id(&self) -> u32 {
        self.id
    }

    pub fn network(&self) -> Address {
        self.network.clone()
    }

    pub fn vm(&self, address: Address) -> Option<VM> {
        self.addresses.get(&address).cloned()
    }

    pub fn addresses(&self, vm: VM) -> Option<Vec<Address>> {
        self.names.get(&vm).cloned()
    }

    pub fn launch(&self, launcher: impl crate::traits::Launcher) -> Result<()> {
        // TODO: configure network
        for (vm, _) in &self.names {
            if vm.supervisor().is_active(&vm)? {
                launcher.launch_detached(&vm)?;
            }
        }

        Ok(())
    }

    pub fn shutdown(&self, launcher: impl crate::traits::Launcher) -> Result<()> {
        for (vm, _) in &self.names {
            if vm.supervisor().is_active(&vm)? {
                launcher.shutdown_immediately(vm)?;
            }
        }

        Ok(())
    }
}
