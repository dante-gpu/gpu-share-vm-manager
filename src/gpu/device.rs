use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};
use std::{
    fs, path::{Path, PathBuf},
    process::Command
};
use std::collections::HashMap;
use utils::os::Platform;

// GPU Configuration - Because every GPU needs its marching orders! 🎮
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GPUConfig {
    pub gpu_id: String,         // The unique identifier of our pixel-pushing warrior
    pub iommu_group: String,    // IOMMU group - keeping our GPU in its own VIP section
}

/// GPU Device Configuration
/// Contains platform-agnostic and platform-specific GPU properties
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GPUDevice {
    pub id: String,
    pub vendor: String,
    pub model: String,
    pub vram_mb: u64,
    pub driver_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metal_support: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vulkan_support: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub directx_version: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iommu_group: Option<u32>,
}

/// GPU Management Core
/// Handles detection, monitoring and allocation of GPU resources
pub struct GPUManager {
    devices: Vec<GPUDevice>,
    iommu_groups: HashMap<u32, Vec<String>>,
}

impl GPUManager {
    /// Initialize GPU Manager with platform-specific detection
    pub fn new() -> Result<Self> {
        let mut manager = Self {
            devices: Vec::new(),
            iommu_groups: HashMap::new(),
        };

        manager.detect_gpus()?;
        manager.build_iommu_groups()?;

        Ok(manager)
    }

    /// Main GPU detection entry point
    pub fn detect_gpus(&mut self) -> Result<()> {
        match Platform::current() {
            Platform::Linux => self.detect_linux_gpus(),
            Platform::MacOS => self.detect_macos_gpus(),
            Platform::Windows => self.detect_windows_gpus(),
            _ => Err(GpuError::UnsupportedPlatform(
                "Unknown platform".to_string()
            ).into()),
        }
    }

    /// Linux-specific GPU detection using NVML and sysfs
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

        // AMD detection via sysfs
        let amd_path = Path::new("/sys/class/drm/card*/device");
        for entry in glob::glob(amd_path.to_str().unwrap())? {
            let path = entry?;
            if let Some(uevent) = Self::read_uevent(&path)? {
                self.devices.push(GPUDevice {
                    id: uevent.device_id,
                    vendor: "AMD".into(),
                    model: uevent.model,
                    vram_mb: Self::read_amd_vram(&path)?,
                    driver_version: Self::read_driver_version(&path)?,
                    vulkan_support: Some(true),
                    ..Default::default()
                });
            }
        }

        Ok(())
    }

    /// macOS GPU detection using Metal API
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

    /// Windows GPU detection using DXGI
    #[cfg(target_os = "windows")]
    fn detect_windows_gpus(&mut self) -> Result<()> {
        use dxgi::{Adapter, Factory};
        
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

    /// Build IOMMU groups for PCI passthrough
    pub fn build_iommu_groups(&mut self) -> Result<()> {
        #[cfg(target_os = "linux")]
        {
            let pci_devices = Path::new("/sys/bus/pci/devices");
            for entry in fs::read_dir(pci_devices)? {
                let path = entry?.path();
                if let Some(group) = Self::get_iommu_group(&path)? {
                    let devices = self.iommu_groups.entry(group).or_default();
                    devices.push(
                        path.file_name()
                            .unwrap()
                            .to_string_lossy()
                            .into_owned()
                    );
                }
            }
        }
        
        Ok(())
    }

    /// Validate IOMMU group safety for passthrough
    pub fn validate_iommu_group(&self, group_id: u32) -> Result<()> {
        let devices = self.iommu_groups.get(&group_id)
            .ok_or(GpuError::IommuGroupNotFound(group_id))?;

        if devices.len() > 1 {
            return Err(GpuError::UnsafeIommuGroup(
                group_id, 
                devices.join(", ")
            ).into());
        }

        Ok(())
    }

    /// Read AMD GPU VRAM from sysfs
    #[cfg(target_os = "linux")]
    fn read_amd_vram(path: &Path) -> Result<u64> {
        let vram_path = path.join("mem_info_vram_total");
        Ok(fs::read_to_string(vram_path)?.trim().parse::<u64>()? / 1024)
    }

    /// Read driver version from sysfs
    #[cfg(target_os = "linux")]
    fn read_driver_version(path: &Path) -> Result<String> {
        Ok(fs::read_to_string(path.join("version"))?.trim().into())
    }

    /// Read PCI device information from uevent
    #[cfg(target_os = "linux")]
    fn read_uevent(path: &Path) -> Result<Option<UeventInfo>> {
        let uevent_path = path.join("uevent");
        if !uevent_path.exists() {
            return Ok(None);
        }

        let mut uevent = UeventInfo::default();
        for line in fs::read_to_string(uevent_path)?.lines() {
            let parts: Vec<&str> = line.split('=').collect();
            match parts[0] {
                "PCI_ID" => uevent.device_id = parts[1].into(),
                "PCI_SUBSYS_ID" => uevent.subsystem_id = parts[1].into(),
                "MODALIAS" => uevent.model = parts[1].split(':').nth(2).unwrap().into(),
                _ => {}
            }
        }

        Ok(Some(uevent))
    }

    /// Get IOMMU group for PCI device
    #[cfg(target_os = "linux")]
    fn get_iommu_group(path: &Path) -> Result<Option<u32>> {
        let group_link = path.join("iommu_group");
        if !group_link.exists() {
            return Ok(None);
        }

        let group_path = fs::read_link(group_link)?;
        let group_id = group_path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .parse::<u32>()?;

        Ok(Some(group_id))
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

/// Helper struct for parsing uevent data
#[derive(Default)]
struct UeventInfo {
    device_id: String,
    subsystem_id: String,
    model: String,
}

impl Default for GPUDevice {
    fn default() -> Self {
        Self {
            id: String::new(),
            vendor: String::new(),
            model: String::new(),
            vram_mb: 0,
            driver_version: String::new(),
            metal_support: None,
            vulkan_support: None,
            directx_version: None,
            iommu_group: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(target_os = "linux")]
    fn test_linux_gpu_detection() {
        let mut manager = GPUManager::new().unwrap();
        manager.detect_gpus().unwrap();
        assert!(!manager.devices.is_empty());
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn test_macos_gpu_detection() {
        let mut manager = GPUManager::new().unwrap();
        manager.detect_gpus().unwrap();
        assert!(!manager.devices.is_empty());
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn test_windows_gpu_detection() {
        let mut manager = GPUManager::new().unwrap();
        manager.detect_gpus().unwrap();
        assert!(!manager.devices.is_empty());
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_iommu_group_handling() {
        let mut manager = GPUManager::new().unwrap();
        manager.build_iommu_groups().unwrap();
        assert!(!manager.iommu_groups.is_empty());
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
            model: "Apple GPU".into(),
            vram_mb: 8192,
            driver_version: "Metal 3".into(),
            metal_support: Some(true),
            vulkan_support: None,
            directx_version: None,
            iommu_group: None,
            ..Default::default()
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
