use serde::{Deserialize, Serialize};
use std::path::PathBuf;
// use anyhow::Result;

// The configuration for our virtual machines
// Because every VM needs a good config, like every developer needs coffee! ☕
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct VMConfig {
    pub name: String,
    pub memory_kb: u64,  // Memory in kilobytes (we're old school!)
    pub vcpus: u32,      // Virtual CPUs (the more the merrier!)
    pub disk_path: PathBuf,  // Where we store our VM's digital dreams
    pub disk_size_gb: u64,   // How much space for those dreams
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VirtualMachine {
    pub id: String,
    pub name: String,
    pub status: VMStatus,
    pub resources: VMResources,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum VMStatus {
    Running,    // Vrooooom! 🏎️
    Stopped,    // Taking a nap 😴
    Failed,     // Houston, we have a problem! 🚨
    Creating,   // Building the dream machine 🏗️
    Deleting,   // Time to say goodbye 👋
}

impl From<u32> for VMStatus {
    fn from(state: u32) -> Self {
        match state {
            1 => VMStatus::Running,
            5 => VMStatus::Stopped,
            _ => VMStatus::Failed,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VMResources {
    pub cpu_cores: u32,      // The brain power! 🧠
    pub memory_mb: u64,      // RAM - because we all need memories
    pub gpu_attached: bool,  // Got that gaming power? 🎮
}