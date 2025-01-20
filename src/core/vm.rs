use serde::{Deserialize, Serialize};
use anyhow::Result;

#[derive(Debug, Serialize, Deserialize)]
pub struct VirtualMachine {
    pub id: String,
    pub name: String,
    pub status: VMStatus,
    pub resources: VMResources,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum VMStatus {
    Running,
    Stopped,
    Failed,
    Creating,
    Deleting,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VMResources {
    pub cpu_cores: u32,
    pub memory_mb: u64,
    pub gpu_attached: bool,
}