use serde::{Deserialize, Serialize};
// use anyhow::Result;

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

#[derive(Debug, Clone)]
pub struct VMConfig {
    pub name: String,
    pub memory_kb: u64,
    pub vcpus: u32,
    pub disk_path: String,
    pub disk_size_gb: u64,
}