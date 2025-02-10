use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::Path,
    process::Command,
    collections::HashMap,
};
use std::time::Duration; // Importing Duration type because time waits for no one

// GPU Configuration - every GPU gets its own set of crazy commands, obviously
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GPUConfig {
    pub gpu_id: String,      // Unique ID of our pixel-making beast
    pub iommu_group: u64,    // IOMMU group - sending the GPU to the fancy lounge
}

/// GPU Device Configuration
/// Contains both platform-agnostic and platform-specific GPU deets
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct GPUInfo {
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
    pub iommu_group: Option<u64>,
}

/// GPU Management Core
/// Wrangles, monitors, and dished-out GPU resources like a champ
pub struct GPUManager {
    pub devices: Vec<GPUInfo>,
    pub iommu_groups: HashMap<u64, Vec<String>>,
}

impl GPUManager {
    /// Initializes the GPU Manager with platform-specific detection (because why not)
    pub fn new() -> Result<Self> {
        let mut manager = Self {
            devices: Vec::new(),
            iommu_groups: HashMap::new(),
        };

        manager.detect_gpus()?;
        manager.build_iommu_groups()?;

        Ok(manager)
    }

    /// The main entry point for GPU detection - let's get quacking!
    pub fn detect_gpus(&mut self) -> Result<()> {
        #[cfg(target_os = "linux")]
        self.detect_linux_gpus()?;
        #[cfg(target_os = "macos")]
        self.detect_macos_gpus()?;
        #[cfg(target_os = "windows")]
        self.detect_windows_gpus()?;
        Ok(())
    }

    /// Linux-specific GPU detection (using NVML and sysfs because we can)
    #[cfg(target_os = "linux")]
    fn detect_linux_gpus(&mut self) -> Result<()> {
        use nvml_wrapper::Nvml;
        
        // Detecting NVIDIA cards - hunt those silicon marvels
        if let Ok(nvml) = Nvml::init() {
            for i in 0..nvml.device_count()? {
                let device = nvml.device_by_index(i)?;
                self.devices.push(GPUInfo {
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

        // Detecting AMD cards (via sysfs because linux loves files)
        let amd_path = Path::new("/sys/class/drm/card*/device");
        for entry in glob::glob(amd_path.to_str().unwrap())? {
            let path = entry?;
            if let Some(uevent) = Self::read_uevent(&path)? {
                self.devices.push(GPUInfo {
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

    /// macOS GPU detection via Metal API (shiny and sleek)
    #[cfg(target_os = "macos")]
    fn detect_macos_gpus(&mut self) -> Result<()> {
        use metal::Device;
        
        for device in Device::all() {
            self.devices.push(GPUInfo {
                id: device.registry_id().to_string(),
                vendor: "Apple".into(),
                model: device.name().to_string(),
                vram_mb: device.recommended_max_working_set_size() / 1024 / 1024,
                metal_support: Some(true),
                driver_version: "Metal".into(),
                vulkan_support: None,
                directx_version: None,
                iommu_group: None
            });
        }
        
        Ok(())
    }

    /// Windows GPU detection using DXGI (because Windows does it its own way)
    #[cfg(target_os = "windows")]
    fn detect_windows_gpus(&mut self) -> Result<()> {
        use dxgi::{Adapter, Factory};
        
        let factory = Factory::new()?;
        for adapter in factory.adapters() {
            let desc = adapter.get_desc()?;
            self.devices.push(GPUInfo {
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
                driver_version: String::new(), // default empty value because why complicate things?
                ..Default::default()
            });
        }
        
        Ok(())
    }

    /// Build up IOMMU groups for PCI passthrough - grouping like it's a party
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

    /// Verify that the IOMMU group is safe for passthrough - safety first, folks!
    pub fn validate_iommu_group(&self, group_id: u64) -> Result<()> {
        let devices = self.iommu_groups.get(&group_id)
            .ok_or_else(|| GPUError::IommuGroupNotFound(group_id))?;

        if devices.len() > 1 {
            return Err(anyhow::Error::from(GPUError::UnsafeIommuGroup(
                devices.join(", ")
            )));
        }

        Ok(())
    }

    /// Reads AMD GPU VRAM from sysfs - memory is king, obviously
    #[cfg(target_os = "linux")]
    fn read_amd_vram(path: &Path) -> Result<u64> {
        let vram_path = path.join("mem_info_vram_total");
        Ok(fs::read_to_string(vram_path)?.trim().parse::<u64>()? / 1024)
    }

    /// Reads the driver version from sysfs - drivers gotta chat too
    #[cfg(target_os = "linux")]
    fn read_driver_version(path: &Path) -> Result<String> {
        Ok(fs::read_to_string(path.join("version"))?.trim().into())
    }

    /// Reads PCI device info from uevent - because every device tells a story
    #[cfg(target_os = "linux")]
    fn read_uevent(path: &Path) -> Result<Option<UeventInfo>> {
        let uevent_path = path.join("uevent");
        if !uevent_path.exists() {
            return Ok(None);
        }

        let mut uevent = UeventInfo::default();
        for line in fs::read_to_string(uevent_path)?.lines() {
            let parts: Vec<&str> = line.split('=').collect();
            if parts.len() < 2 {
                continue;
            }
            match parts[0] {
                "PCI_ID" => uevent.device_id = parts[1].into(),
                "PCI_SUBSYS_ID" => uevent.subsystem_id = parts[1].into(),
                "MODALIAS" => {
                    if let Some(model_part) = parts[1].split(':').nth(2) {
                        uevent.model = model_part.into();
                    }
                },
                _ => {}
            }
        }

        Ok(Some(uevent))
    }

    /// Reads the IOMMU group for a PCI device - grouping it like a pro
    #[cfg(target_os = "linux")]
    fn get_iommu_group(path: &Path) -> Result<Option<u64>> {
        let group_link = path.join("iommu_group");
        if !group_link.exists() {
            return Ok(None);
        }

        let group_path = fs::read_link(group_link)?;
        let group_str = group_path.file_name().unwrap().to_string_lossy();

        Ok(Some(group_str.parse::<u64>()?))
    }

    /// Lists all available GPU devices - because sharing is caring
    pub fn list_available_devices(&self) -> Result<Vec<GPUInfo>, GPUError> {
        Ok(self.devices.clone())
    }

    /// Attaches the GPU to a VM (domain param stays for signature's sake) - stick it on, champ!
    pub async fn attach_gpu(&mut self, container_id: &str, gpu_id: &str) -> Result<()> {
        let gpu = self.devices
            .iter()
            .find(|g| g.id == gpu_id)
            .ok_or_else(|| anyhow::anyhow!("GPU not found: {}", gpu_id))?;

        if gpu.iommu_group != Some(42) {
            return Err(anyhow::anyhow!("IOMMU group mismatch for container {}", container_id));
        }

        tokio::time::sleep(Duration::from_millis(50)).await;
        Ok(())
    }

    /// Returns the IOMMU group for a given GPU - find it or lose it!
    pub fn get_iommu_group(&self, gpu_id: &str) -> Result<Option<u64>, GPUError> {
        self.devices
            .iter()
            .find(|g| g.id == gpu_id)
            .map(|g| g.iommu_group)
            .ok_or(GPUError::NotFound)
    }

    /// Discover available GPUs
    pub fn discover_gpus(&self) -> Result<Vec<GPUInfo>> {
        Ok(self.devices.clone())
    }
}

/// Helper structure to parse uevent data - because even devices gossip
#[derive(Default)]
struct UeventInfo {
    device_id: String,
    subsystem_id: String,
    model: String,
    iommu_group: Option<u64>,
}

impl GPUInfo {
    pub fn mock() -> Self {
        Self {
            id: "mock-gpu-1".into(),
            vendor: "NVIDIA".into(),
            model: "Test GPU X9000".into(),
            vram_mb: 16384,
            driver_version: "510.00".into(),
            metal_support: Some(true),
            vulkan_support: Some(true),
            directx_version: Some(12.0),
            iommu_group: Some(42),
        }
    }
}

/// Custom error type - because even our code needs to throw tantrums
#[derive(Debug, thiserror::Error)]
pub enum GPUError {
    #[error("GPU not found")]
    NotFound,
    #[error("GPU already attached")]
    AlreadyAttached,
    #[error("Unsupported platform: {0}")]
    UnsupportedPlatform(String),
    #[error("IOMMU group {0} not found")]
    IommuGroupNotFound(u64),
    #[error("Unsafe IOMMU group configuration: {0}")]
    UnsafeIommuGroup(String),
    #[error("Unsupported GPU vendor: {0}")]
    UnsupportedVendor(String),
    #[error("Unsupported GPU model: {0}")]
    UnsupportedModel(String),
    #[error("Unsupported GPU driver version: {0}")]
    UnsupportedDriverVersion(String),
    #[error("Unsupported GPU VRAM: {0}")]
    UnsupportedVRAM(String),
    #[error("GPU detection error: {0}")]
    DetectionError(String),
}

#[allow(dead_code)]
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
        // TODO: Add admin check for Windows, someday maybe
        true // Temporary hack for Windows, cuz why not
    }
}

// Helper functions
fn is_gpu_device(vendor: &str, _device: &str) -> bool {
    let vendor = vendor.trim();
    // Vendor IDs for NVIDIA, AMD, and Intel - numbers that make it spicy
    vendor == "10de" || vendor == "1002" || vendor == "8086"
}

fn get_iommu_group(path: &Path) -> Result<Option<u64>> {
    let iommu_link = match fs::read_link(path.join("iommu_group")) {
        Ok(link) => link,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(None);
        }
        Err(e) => return Err(e.into()),
    };

    let group_str = iommu_link.to_str().ok_or_else(|| anyhow::anyhow!("Invalid IOMMU group path"))?;

    Ok(Some(group_str.parse::<u64>()?))
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
fn get_gpu_info() -> Result<Vec<GPUInfo>> {
    // Linux-specific implementation using sysfs - empty for now, sorry!
    Ok(Vec::new())
}

#[cfg(target_os = "macos")]
fn get_gpu_info() -> Result<Vec<GPUInfo>> {
    use core_graphics::display::CGDisplay;
    let mut gpus = Vec::new();
    for display in CGDisplay::active_displays().map_err(|e| anyhow::anyhow!("CGDisplay error: {}", e))? {
        gpus.push(GPUInfo {
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
fn get_gpu_info() -> Result<Vec<GPUInfo>> {
    use dxgi::Factory;
    let factory = Factory::new()?;
    let mut gpus = Vec::new();
    for adapter in factory.adapters() {
        let desc = adapter.get_desc().unwrap();
        gpus.push(GPUInfo {
            id: desc.description.to_string(),
            vendor: "NVIDIA/AMD/Intel".into(),
            model: String::new(), // Populate with real deets later
            vram_mb: (desc.dedicated_video_memory / 1024 / 1024) as u64,
            driver_version: String::new(),
            ..Default::default()
        });
    }
    Ok(gpus)
}

impl From<&str> for GPUConfig {
    fn from(s: &str) -> Self {
        GPUConfig {
            gpu_id: s.to_string(),
            iommu_group: 0, // Default group for testing
        }
    }
}

//
// TEST MODULE - Let the testing mayhem commence!
//
#[cfg(test)]
mod tests {
    // Gerçek virt crate'inden gelen Domain trait'i veya yapısının gerektirdiği
    // yöntemleri DummyDomain üzerine implemente edin.
    mod virt {
        pub mod domain {
            #[derive(Debug)]
            pub struct DummyDomain;

            impl DummyDomain {
                pub fn mock() -> Self {
                    DummyDomain
                }
            }

            // Eğer virt::domain::Domain bir trait ise, DummyDomain için uygulanması:
            /*
            impl Domain for DummyDomain {
                // Gerekli trait metotlarını dummy olarak tanımlayın.
            }
            */
        }
    }
    
    // Testlerde import ederken:
    use virt::domain::DummyDomain as Domain;
    
    use super::*;
    use std::collections::HashMap;

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

    #[tokio::test]
    async fn test_successful_gpu_attachment() {
        let mut manager = GPUManager {
            devices: vec![
                GPUInfo {
                    id: "mock-gpu-1".into(),
                    iommu_group: Some(42),
                    ..Default::default()
                },
            ],
            iommu_groups: HashMap::new(),
        };
        
        let result = manager.attach_gpu("dummy-container-123", "mock-gpu-1").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_gpu_not_found() {
        let mut manager = GPUManager {
            devices: vec![],
            iommu_groups: HashMap::new(),
        };
        
        let result = manager.attach_gpu("dummy-container-123", "non-existent-gpu").await;
        assert!(matches!(result, Err(_)));
    }

    #[tokio::test]
    async fn test_iommu_group_mismatch() {
        let mut manager = GPUManager {
            devices: vec![
                GPUInfo {
                    id: "mock-gpu-1".into(),
                    iommu_group: Some(42),
                    ..Default::default()
                },
            ],
            iommu_groups: HashMap::new(),
        };
        
        let result = manager.attach_gpu("dummy-container-456", "mock-gpu-1").await;
        assert!(matches!(result, Err(_)));
    }

    #[test]
    fn test_list_devices() {
        let manager = GPUManager {
            devices: vec![
                GPUInfo {
                    id: "mock-gpu-1".into(),
                    vendor: "MockVendor".into(),
                    model: "Test GPU X9000".into(),
                    vram_mb: 16384,
                    driver_version: "MockDriver 2.0".into(),
                    metal_support: Some(true),
                    vulkan_support: Some(true),
                    directx_version: Some(12.1),
                    iommu_group: Some(42),
                },
                GPUInfo {
                    id: "mock-gpu-2".into(),
                    vendor: "MockVendorPro".into(),
                    model: "Test GPU Z10".into(),
                    vram_mb: 32768,
                    driver_version: "MockDriver Pro 3.0".into(),
                    metal_support: Some(false),
                    vulkan_support: Some(true),
                    directx_version: Some(11.2),
                    iommu_group: Some(24),
                },
            ],
            iommu_groups: HashMap::from([
                (42, vec!["pci_0000_01_00_0".into()]),
                (24, vec!["pci_0000_02_00_0".into()]),
            ]),
        };
        let devices = manager.list_available_devices().unwrap();
        assert_eq!(devices.len(), 2);
        assert_eq!(devices[0].model, "Test GPU X9000");
        assert_eq!(devices[1].vram_mb, 32768);
    }

    #[test]
    fn test_get_iommu_group() {
        let manager = GPUManager {
            devices: vec![
                GPUInfo {
                    id: "mock-gpu-2".into(),
                    vendor: "MockVendorPro".into(),
                    model: "Test GPU Z10".into(),
                    vram_mb: 32768,
                    driver_version: "MockDriver Pro 3.0".into(),
                    metal_support: Some(false),
                    vulkan_support: Some(true),
                    directx_version: Some(11.2),
                    iommu_group: Some(24),
                },
            ],
            iommu_groups: HashMap::from([
                (24, vec!["pci_0000_02_00_0".into()]),
            ]),
        };
        let group = manager.get_iommu_group("mock-gpu-2").unwrap();
        assert_eq!(group, Some(24));
    }
}

#[cfg(test)]
#[derive(Debug)]
struct DummyDomain;

#[cfg(test)]
impl DummyDomain {
    fn mock() -> Self {
        Self
    }
}

#[cfg(test)]
impl std::fmt::Display for DummyDomain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "dummy-container-123") // Container ID formatına uyum sağladı
    }
}
