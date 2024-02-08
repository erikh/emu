use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub type NetworkMap = HashMap<String, Vec<String>>;
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NetworkConfig {
    networks: NetworkMap,
}
