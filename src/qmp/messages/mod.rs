mod block;

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct GenericReturn {
    #[serde(rename = "return")]
    pub result: Option<HashMap<(), ()>>,
    pub error: Option<ErrorDetail>,
}

impl<T> From<GenericReturn> for Result<T>
where
    T: for<'de> serde::Deserialize<'de> + Default + std::fmt::Debug,
{
    fn from(value: GenericReturn) -> Self {
        if let Some(error) = value.error {
            return Err(anyhow!("{}", error));
        }

        Ok(T::default())
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct QueryBlock {
    #[serde(rename = "return")]
    pub result: Vec<block::Block>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct QueryJobs {
    #[serde(rename = "return")]
    pub result: Vec<JobInfo>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct JobInfo {
    pub id: String,
    #[serde(rename = "type")]
    pub typ: String,
    pub status: String,
    pub current_progress: u64,
    pub total_progress: u64,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Event {
    pub timestamp: Option<Timestamp>,
    pub event: String,
    pub data: Option<EventData>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Timestamp {
    pub seconds: u64,
    pub microseconds: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct EventData {
    pub status: String,
    pub id: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ErrorReturn {
    pub error: ErrorDetail,
}

impl From<ErrorReturn> for anyhow::Error {
    fn from(value: ErrorReturn) -> Self {
        anyhow!("{}", value.error)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ErrorDetail {
    pub class: String,
    pub desc: String,
}

impl std::fmt::Display for ErrorDetail {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&format!("{}: {}", self.class, self.desc))
    }
}
