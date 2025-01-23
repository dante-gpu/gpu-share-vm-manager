use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracing::{info, warn, error};
use std::fs::{self};
use std::path::Path;
use std::process::Command;
// use std::path::PathBuf;

// GPU Configuration - Because every GPU needs its marching orders! 🎮
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GPUConfig {
    pub gpu_id: String,         // The unique identifier of our pixel-pushing warrior
    pub iommu_group: String,    // IOMMU group - keeping our GPU in its own VIP section
}

// Our GPU Device - The silicon celebrity of our virtual world! ⭐
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GPUDevice {
    pub id: String,             // Every star needs a unique name
    pub vendor_id: String,      // Who's your manufacturer? 🏭
    pub device_id: String,      // Model number - because we're all unique!
    pub pci_address: String,    // Where to find this beauty on the PCI runway
    pub iommu_group: Option<String>, // The VIP lounge number (if we're fancy enough)
}

// The mastermind behind our GPU operations! 🧙‍♂️
pub struct GPUManager {
    devices: Vec<GPUDevice>,    // Our collection of pixel-pushing powerhouses
}

impl GPUManager {
    // Time to wake up our GPU manager! Rise and shine! 🌅
    pub fn new() -> Result<Self> {
        Ok(Self {
            devices: Vec::new(),
        })
    }

    // Let's discover what GPUs are hiding in this machine! 🔍
    pub fn discover_gpus(&self) -> Result<Vec<GPUDevice>, anyhow::Error> {
        // TODO: Implement actual GPU discovery
        Ok(self.devices.clone())
    }

    // Assign those GPUs to their IOMMU groups - like assigning students to classrooms! 🏫
    pub fn assign_iommu_groups(&mut self) -> Result<()> {
        // TODO: Implement IOMMU group assignment
        Ok(())
    }

    // Time to introduce our GPU to its new VM friend! 🤝
    pub async fn attach_gpu_to_vm(&mut self, domain: &virt::domain::Domain, config: &GPUConfig) -> Result<String, anyhow::Error> {
        // Validate GPU exists and is available
        let gpu = self.devices.iter()
            .find(|g| g.id == config.gpu_id)
            .ok_or_else(|| anyhow::anyhow!("GPU not found"))?;

        // Check IOMMU group matches
        if gpu.iommu_group.as_ref() != Some(&config.iommu_group) {
            return Err(anyhow::anyhow!("IOMMU group mismatch"));
        }

        // TODO: Implement actual GPU passthrough
        // For now, just return success
        Ok("GPU attached successfully!".to_string())
    }

    pub fn get_iommu_group(&self, gpu_id: &str) -> Result<Option<String>, anyhow::Error> {
        let gpu = self.devices.iter()
            .find(|gpu| gpu.id == gpu_id)
            .ok_or_else(|| anyhow::anyhow!("GPU not found: {}", gpu_id))?;
        
        Ok(gpu.iommu_group.clone())
    }
}

#[allow(dead_code)] //hehhee
fn has_required_permissions() -> bool {
    if cfg!(unix) {
        Command::new("id")
            .arg("-u")
            .output()
            .map(|output| {
                String::from_utf8_lossy(&output.stdout).trim() == "0"
            })
            .unwrap_or(false)
    } else {
        // TODO: Windows admin check would go here -@virjilakrum
        true // Placeholder for Windows implementation
    }
}