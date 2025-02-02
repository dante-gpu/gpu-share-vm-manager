use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use anyhow::{Result, anyhow};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VirtualGPU {
    pub id: u32,
    pub vram_mb: u32,
    pub compute_units: u32,
    pub allocated_to: Option<String>,
}

pub struct GPUPool {
    pub gpus: HashMap<u32, VirtualGPU>,
}

impl GPUPool {
    pub fn new() -> Self {
        let mut gpus = HashMap::new();
        gpus.insert(0, VirtualGPU {
            id: 0,
            vram_mb: 8192,
            compute_units: 32,
            allocated_to: None
        });
        gpus.insert(1, VirtualGPU {
            id: 1,
            vram_mb: 16384,
            compute_units: 64,
            allocated_to: None
        });
        Self { gpus }
    }
    
    pub fn allocate(&mut self, user: &str, gpu_id: u32) -> anyhow::Result<f64> {
        let gpu = self.gpus.get_mut(&gpu_id).ok_or(anyhow!("GPU not found"))?;
        if gpu.allocated_to.is_some() {
            return Err(anyhow!("GPU already allocated"));
        }
        
        gpu.allocated_to = Some(user.to_string());
        let cost = self.calculate_cost(gpu_id)?;
        Ok(cost)
    }
    
    fn calculate_cost(&self, gpu_id: u32) -> anyhow::Result<f64> {
        let gpu = self.gpus.get(&gpu_id).ok_or(anyhow!("GPU not found"))?;
        Ok(gpu.vram_mb as f64 * 0.1 + gpu.compute_units as f64 * 2.0)
    }
    
    pub fn release(&mut self, gpu_id: u32) -> Result<(), anyhow::Error> {
        let gpu = self.gpus.get_mut(&gpu_id)
            .ok_or_else(|| anyhow!("GPU not found"))?;
        
        gpu.allocated_to = None;
        Ok(())
    }
    
    pub fn get_allocated_gpus(&self, user: &str) -> Vec<&VirtualGPU> {
        self.gpus.values()
            .filter(|g| g.allocated_to.as_ref() == Some(&user.to_string()))
            .collect()
    }
}

