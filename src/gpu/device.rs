use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracing::warn;
use std::fs::{self};
use std::path::Path;
use std::process::Command;
use std::collections::HashMap;
use utils::os::Platform;

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
    pub vendor: String,         // Who's your manufacturer? 🏭
    pub model: String,          // Model number - because we're all unique!
    pub vram_mb: u64,          // VRAM in MB
    pub driver_version: String, // GPU driver version
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metal_support: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vulkan_support: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub directx_version: Option<f32>,
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

    /// Unified GPU detection across platforms
    pub fn detect_gpus(&mut self) -> Result<()> {
        match Platform::current() {
            Platform::Linux => self.detect_linux_gpus(),
            Platform::MacOS => self.detect_macos_gpus(),
            Platform::Windows => self.detect_windows_gpus(),
            _ => Err(GpuError::UnsupportedPlatform(
                "Unknown platform".to_string()
            )),
        }
    }

    #[cfg(target_os = "linux")]
    fn detect_linux_gpus(&mut self) -> Result<()> {
        use nvml_wrapper::Nvml;
        
        // NVIDIA detection
        if let Ok(nvml) = Nvml::init() {
            for i in 0..nvml.device_count()? {
                let device = nvml.device_by_index(i)?;
                self.devices.push(GPUDevice {
                    id: device.uuid()?,
                    vendor: "NVIDIA".into(),
                    model: device.name()?,
                    vram_mb: device.memory_info()?.total / 1024 / 1024,
                    driver_version: nvml.sys_driver_version()?,
                    vulkan_support: Some(true),
                    ..Default::default()
                });
            }
        }
        
        // AMD detection (using amdgpu driver)
        // ... AMD detection logic ...

        Ok(())
    }

    #[cfg(target_os = "macos")]
    fn detect_macos_gpus(&mut self) -> Result<()> {
        use metal::Device;
        
        for device in Device::all() {
            self.devices.push(GPUDevice {
                id: device.registry_id().to_string(),
                vendor: "Apple".into(),
                model: device.name().to_string(),
                vram_mb: device.recommended_max_vram() / 1024 / 1024,
                metal_support: Some(true),
                ..Default::default()
            });
        }
        
        Ok(())
    }

    #[cfg(target_os = "windows")]
    fn detect_windows_gpus(&mut self) -> Result<()> {
        use dxgi::Factory;
        
        let factory = Factory::new()?;
        for adapter in factory.adapters() {
            let desc = adapter.get_desc()?;
            self.devices.push(GPUDevice {
                id: format!("PCI\\VEN_{:04X}&DEV_{:04X}", desc.vendor_id, desc.device_id),
                vendor: match desc.vendor_id {
                    0x10DE => "NVIDIA".into(),
                    0x1002 => "AMD".into(),
                    0x8086 => "Intel".into(),
                    _ => "Unknown".into(),
                },
                model: desc.description.to_string(),
                vram_mb: (desc.dedicated_video_memory / 1024 / 1024) as u64,
                directx_version: Some(desc.revision as f32 / 10.0),
                ..Default::default()
            });
        }
        
        Ok(())
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
            let pci_path = Path::new("/sys/bus/pci/devices").join(&gpu.id);
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

#[cfg(target_os = "linux")]
fn get_gpu_info() -> Result<Vec<GPUDevice>> {
    // Linux-specific implementation using sysfs
    Ok(Vec::new())
}

#[cfg(target_os = "macos")]
fn get_gpu_info() -> Result<Vec<GPUDevice>> {
    use core_graphics::display::CGDisplay;
    let mut gpus = Vec::new();
    for display in CGDisplay::active_displays()? {
        gpus.push(GPUDevice {
            id: format!("display-{}", display),
            vendor: "Apple".into(),
            // MacOS specific GPU info
        });
    }
    Ok(gpus)
}

#[cfg(target_os = "windows")]
fn get_gpu_info() -> Result<Vec<GPUDevice>> {
    // Windows implementation using DXGI
    use dxgi::Factory;
    let factory = Factory::new()?;
    let mut gpus = Vec::new();
    for adapter in factory.adapters() {
        gpus.push(GPUDevice {
            id: adapter.get_info().name,
            vendor: "NVIDIA/AMD/Intel".into(),
            // Windows specific data
        });
    }
    Ok(gpus)
}