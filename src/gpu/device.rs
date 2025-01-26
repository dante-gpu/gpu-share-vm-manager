use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracing::warn;
use std::fs::{self};
use std::path::Path;
use std::process::Command;
use std::collections::HashMap;

// GPU Configuration - Because every GPU needs its marching orders! 🎮
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GPUConfig {
    pub gpu_id: String,         // The unique identifier of our pixel-pushing warrior
    pub iommu_group: String,    // IOMMU group - keeping our GPU in its own VIP section
}

// Our GPU Device - The silicon celebrity of our virtual world! 
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GPUDevice {
    pub id: String,             // Every star needs a unique name
    pub vendor_id: String,      // Who's your manufacturer? 🏭
    pub device_id: String,      // Model number - because we're all unique!
    pub pci_address: String,    // Where to find this beauty on the PCI runway
    pub iommu_group: Option<String>, // The VIP lounge number (if we're fancy enough)
    pub temperature: f64,        // Temperature of the GPU
    pub utilization: f64,         // Utilization of the GPU
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
    pub fn discover_gpus(&mut self) -> Result<Vec<GPUDevice>> {
        let mut devices = Vec::new();
        let pci_devices = fs::read_dir("/sys/bus/pci/devices")?;
        
        for entry in pci_devices {
            let path = entry?.path();
            let vendor = fs::read_to_string(path.join("vendor"))?;
            let device = fs::read_to_string(path.join("device"))?;
            
            if is_gpu_device(&vendor, &device) {
                let iommu_group = get_iommu_group(&path)?;
                devices.push(GPUDevice {
                    id: format!("{}:{}", vendor.trim(), device.trim()),
                    vendor_id: vendor.trim().to_string(),
                    device_id: device.trim().to_string(),
                    pci_address: path.file_name().unwrap().to_str().unwrap().to_string(),
                    iommu_group,
                    temperature: read_gpu_temperature(&path)?,
                    utilization: read_gpu_utilization(&path)?,
                });
            }
        }
        Ok(devices)
    }

    // Assign those GPUs to their IOMMU groups - like assigning students to classrooms! 
    #[allow(dead_code)]
    pub fn assign_iommu_groups(&mut self) -> Result<()> {
        // Scan all PCI devices to create IOMMU groups
        let mut iommu_groups = HashMap::new();
        let pci_devices = fs::read_dir("/sys/bus/pci/devices")?;

        for entry in pci_devices {
            let path = entry?.path();
            if let Some(group) = get_iommu_group(&path)? {
                let devices = iommu_groups.entry(group).or_insert(Vec::new());
                devices.push(path);
            }
        }

        // Match GPUs with their corresponding IOMMU groups
        for gpu in &mut self.devices {
            // Find IOMMU group from GPU's PCI address
            let pci_path = Path::new("/sys/bus/pci/devices").join(&gpu.pci_address);
            if let Some(group) = get_iommu_group(&pci_path)? {
                // Collect all devices in the group
                let group_devices = iommu_groups.get(&group)
                    .ok_or_else(|| anyhow::anyhow!("IOMMU group not found"))?;

                // Validate group safety
                if !Self::is_safe_iommu_group(group_devices) {
                    warn!("Unsafe IOMMU group {} for GPU {}", group, gpu.id);
                    continue;
                }

                // Assign group info to GPU
                gpu.iommu_group = Some(group.clone());
                
                // Log all devices in group (optional)
                // debug!("GPU {} assigned to IOMMU group {} with devices: {:?}", 
                //     gpu.id, group, group_devices);
            }
        }

        Ok(())
    }

    // IOMMU grubunun güvenli olduğunu kontrol et
    fn is_safe_iommu_group(devices: &[std::path::PathBuf]) -> bool {
        // A group should only contain GPU and audio controller
        devices.iter().all(|path| {
            let class = fs::read_to_string(path.join("class"))
                .unwrap_or_default();
            class.starts_with("0x0300") || // GPU
            class.starts_with("0x0403")    // Ses kontrolcüsü
        })
    }

    // Time to introduce our GPU to its new VM friend! 🤝
    pub async fn attach_gpu_to_vm(&mut self, _domain: &virt::domain::Domain, config: &GPUConfig) -> Result<String, anyhow::Error> {
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

// Add these helper functions
fn is_gpu_device(vendor: &str, _device: &str) -> bool {
    let vendor = vendor.trim();
    // NVIDIA, AMD, Intel vendor IDs
    vendor == "10de" || vendor == "1002" || vendor == "8086"
}

fn get_iommu_group(path: &Path) -> Result<Option<String>> {
    let iommu_link = match fs::read_link(path.join("iommu_group")) {
        Ok(link) => link,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(None);
        }
        Err(e) => return Err(e.into()),
    };

    Ok(Some(
        iommu_link
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("Invalid IOMMU group path"))?
            .split('/')
            .last()
            .unwrap()
            .to_string(),
    ))
}

fn read_gpu_temperature(path: &Path) -> Result<f64> {
    let temp_str = fs::read_to_string(path.join("temp1_input"))?.trim().to_string();
    Ok(temp_str.parse::<f64>()? / 1000.0)
}

fn read_gpu_utilization(path: &Path) -> Result<f64> {
    let util_str = fs::read_to_string(path.join("gpu_busy_percent"))?.trim().to_string();
    Ok(util_str.parse()?)
}