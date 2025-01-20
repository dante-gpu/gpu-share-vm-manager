use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

// Noticed your GPU *blushes*
#[derive(Debug, Serialize, Deserialize)]
pub struct GPUDevice {
    pub id: String,
    pub vendor_id: String,
    pub device_id: String,
    pub iommu_group: Option<u32>,
    pub is_available: bool,
}

// GPU Manager goes hard
pub struct GPUManager {
    devices: Vec<GPUDevice>,
}

impl GPUManager {
  
    pub fn new() -> Result<Self> {
        info!("GPU Manager: I'm alive! Time to catch some graphics cards!");
        Ok(Self { 
            devices: Vec::new() 
        })
    }


    pub fn discover_gpus(&mut self) -> Result<()> {
        // TODO: Implement GPU discovery -@virjilakrum
        // For now, just a placeholder that pretends to find a GPU
        warn!("GPU-chan is still learning how to find other GPUs >.<");
        
        self.devices.push(GPUDevice {
            id: "gpu-0".to_string(),
            vendor_id: "10de".to_string(), 
            device_id: "2204".to_string(), 
            iommu_group: Some(13), 
            is_available: true,
        });

        info!("Found {} GPU(s)! Sugoi!", self.devices.len());
        Ok(())
    }

    // Time to yeet this GPU into a VM!
    pub fn attach_gpu_to_vm(&mut self, gpu_id: &str, vm_xml: &str) -> Result<String> {
        // Find our precious GPU
        let gpu = self.devices.iter_mut().find(|g| g.id == gpu_id);
        
        match gpu {
            Some(gpu) if gpu.is_available => {
                info!("GPU-chan is ready to join VM-sama!");
                gpu.is_available = false;
                
                // Add GPU to VM XML config
                // TODO: Implement actual XML modification
                let new_xml = format!("{}\n<!-- GPU {} attached -->", vm_xml, gpu_id);
                
                Ok(new_xml)
            },
            Some(_) => {
                warn!("Gomenasai, GPU is already taken by another VM (；⌣̀_⌣́)");
                Err(anyhow::anyhow!("GPU not available"))
            },
            None => {
                warn!("Nani?! GPU not found ಥ_ಥ");
                Err(anyhow::anyhow!("GPU not found"))
            }
        }
    }
}

// GPU collection 
#[derive(Debug)]
pub struct GPUCollection {
    available_gpus: Vec<GPUDevice>,
    assigned_gpus: Vec<(GPUDevice, String)>, // (GPU, VM ID)
}