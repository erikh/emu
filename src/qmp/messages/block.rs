use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Block {
    pub device: String,
    pub inserted: Option<Drive>,
    pub io_status: Option<String>,
    pub locked: Option<bool>,
    pub qdev: Option<String>,
    pub removable: Option<bool>,
    #[serde(rename = "type")]
    pub typ: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Drive {
    pub backing_file: Option<String>,
    pub backing_file_depth: Option<usize>,
    pub bps: Option<usize>,
    pub bps_rd: Option<usize>,
    pub bps_wr: Option<usize>,
    pub cache: Option<Cache>,
    pub detect_zeroes: Option<String>,
    pub drv: Option<String>,
    pub encrypted: Option<bool>,
    pub file: Option<String>,
    pub image: Option<Image>,
    pub iops: Option<usize>,
    pub iops_rd: Option<usize>,
    pub iops_wr: Option<usize>,
    pub node_name: Option<String>,
    pub ro: Option<bool>,
    pub write_threshold: Option<usize>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Image {
    pub actual_size: Option<usize>,
    pub backing_image: Option<BackingImage>,
    pub cluster_size: Option<usize>,
    pub dirty_flag: Option<bool>,
    pub filename: Option<String>,
    pub format: Option<String>,
    pub format_specific: Option<Format>,
    pub snapshots: Option<Snapshots>,
    pub virtual_size: Option<usize>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct BackingImage {
    pub actual_size: Option<usize>,
    pub cluster_size: Option<usize>,
    pub dirty_flag: Option<bool>,
    pub filename: Option<String>,
    pub format: Option<String>,
    pub format_specific: Option<Format>,
    pub virtual_size: Option<usize>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Format {
    pub data: Option<FormatData>,
    #[serde(rename = "type")]
    pub typ: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct FormatData {
    pub compat: Option<String>,
    pub compression_type: Option<String>,
    pub corrupt: Option<bool>,
    pub extended_l2: Option<bool>,
    pub lazy_refcounts: Option<bool>,
    pub refcount_bits: Option<usize>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Snapshots(pub Vec<Snapshot>);

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Snapshot {
    pub date_nsec: Option<usize>,
    pub date_sec: Option<usize>,
    pub id: Option<String>,
    pub name: Option<String>,
    pub vm_clock_nsec: Option<usize>,
    pub vm_clock_sec: Option<usize>,
    pub vm_state_size: Option<usize>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Cache {
    pub direct: Option<bool>,
    pub no_flush: Option<bool>,
    pub writeback: Option<bool>,
}
