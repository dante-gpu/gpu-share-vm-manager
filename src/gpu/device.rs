use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracing::{info, warn, error};
use std::fs::{self};
use std::path::Path;
use std::process::Command;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GPUDevice {
    pub id: String,
    pub vendor_id: String,
    pub device_id: String,
    pub iommu_group: Option<u32>,
    pub is_available: bool,
    pub pci_address: String,
    pub driver: String,
    pub memory_mb: u64,
}

pub struct GPUManager {
    devices: Vec<GPUDevice>,
    sysfs_path: String,
}

impl GPUManager {
    pub fn new() -> Result<Self> {
        info!("Initializing GPU Manager with system checks");
        
        // Check if we have necessary permissions
        if !has_required_permissions() {
            error!("Insufficient permissions for GPU management");
            return Err(anyhow::anyhow!("Required root/admin permissions not available"));
        }

        Ok(Self { 
            devices: Vec::new(),
            sysfs_path: "/sys/bus/pci/devices".to_string(),
        })
    }

    pub fn discover_gpus(&mut self) -> Result<()> {
        info!("Starting GPU discovery process");
        self.devices.clear();

        // Read PCI devices
        let gpu_classes = ["0x030000", "0x030200"]; // VGA compatible and 3D controller
        let entries = fs::read_dir(&self.sysfs_path)?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            
            if self.is_gpu_device(&path, &gpu_classes)? {
                if let Ok(gpu) = self.create_gpu_device(&path) {
                    info!("Discovered GPU: Vendor {} Device {}", gpu.vendor_id, gpu.device_id);
                    self.devices.push(gpu);
                }
            }
        }

        // Check IOMMU groups for each GPU
        self.assign_iommu_groups()?;
        
        info!("GPU discovery completed. Found {} devices", self.devices.len());
        Ok(())
    }

    fn is_gpu_device(&self, path: &Path, gpu_classes: &[&str]) -> Result<bool> {
        let class_path = path.join("class");
        if let Ok(class) = fs::read_to_string(class_path) {
            let class = class.trim();
            return Ok(gpu_classes.contains(&class));
        }
        Ok(false)
    }

    fn create_gpu_device(&self, path: &Path) -> Result<GPUDevice> {
        let vendor_id = fs::read_to_string(path.join("vendor"))?.trim().replace("0x", "");
        let device_id = fs::read_to_string(path.join("device"))?.trim().replace("0x", "");
        let driver = fs::read_to_string(path.join("driver_override"))
            .unwrap_or_else(|_| fs::read_to_string(path.join("driver"))
            .unwrap_or_else(|_| "unknown".to_string()));
        
        let pci_address = path.file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();

        let memory_mb = self.get_gpu_memory(&pci_address)?;

        Ok(GPUDevice {
            id: format!("gpu-{}", pci_address),
            vendor_id,
            device_id,
            pci_address,
            driver: driver.trim().to_string(),
            iommu_group: None,
            is_available: true,
            memory_mb,
        })
    }

    fn get_gpu_memory(&self, pci_address: &str) -> Result<u64> {
        // Try nvidia-smi first
        if let Ok(output) = Command::new("nvidia-smi")
            .args(&["--query-gpu=memory.total", "--format=csv,noheader,nounits"])
            .output() 
        {
            if output.status.success() {
                if let Ok(memory_str) = String::from_utf8(output.stdout) {
                    if let Ok(memory) = memory_str.trim().parse::<u64>() {
                        return Ok(memory);
                    }
                }
            }
        }

        // Fallback to reading sysfs for AMD cards
        let memory_path = Path::new("/sys/class/drm")
            .join(format!("card{}", pci_address))
            .join("device/mem_info_vram_total");

        if memory_path.exists() {
            if let Ok(memory_str) = fs::read_to_string(memory_path) {
                if let Ok(memory_bytes) = memory_str.trim().parse::<u64>() {
                    return Ok(memory_bytes / (1024 * 1024)); // Convert to MB
                }
            }
        }

        // Default fallback
        Ok(0)
    }

    pub fn assign_iommu_groups(&mut self) -> Result<()> {
        let iommu_path = Path::new("/sys/kernel/iommu_groups");
        if !iommu_path.exists() {
            warn!("IOMMU groups not available on this system");
            return Ok(());
        }

        for device in &mut self.devices {
            let device_path = Path::new("/sys/bus/pci/devices")
                .join(&device.pci_address)
                .join("iommu_group");

            if let Ok(group_path) = fs::read_link(device_path) {
                if let Some(group_name) = group_path.file_name() {
                    if let Ok(group_num) = group_name.to_string_lossy().parse::<u32>() {
                        device.iommu_group = Some(group_num);
                        info!("Assigned IOMMU group {} to GPU {}", group_num, device.id);
                    }
                }
            }
        }
        Ok(())
    }

    pub fn attach_gpu_to_vm(&mut self, gpu_id: &str, vm_xml: &str) -> Result<String> {
        // Finding and cloning the GPU
        let gpu = match self.devices.iter().find(|d| d.id == gpu_id) {
            Some(device) => device.clone(),
            None => return Err(anyhow::anyhow!("GPU not found")),
        };

        if !gpu.is_available {
            return Err(anyhow::anyhow!("GPU is not available"));
        }

        // Preparing the XML configuration
        let gpu_xml = format!(
            r#"
            <hostdev mode='subsystem' type='pci' managed='yes'>
                <source>
                    <address domain='0x0000' bus='0x{}'
                            slot='0x{}' function='0x{}'/>
                </source>
                <address type='pci' domain='0x0000' bus='0x00'
                         slot='0x{:02x}' function='0x0'/>
            </hostdev>
            "#,
            &gpu.pci_address[0..2],
            &gpu.pci_address[3..5],
            &gpu.pci_address[6..7],
            self.calculate_next_free_slot(vm_xml)?
        );

        // Updating the XML
        let new_xml = if let Some(pos) = vm_xml.rfind("</devices>") {
            let (start, end) = vm_xml.split_at(pos);
            format!("{}{}{}", start, gpu_xml, end)
        } else {
            return Err(anyhow::anyhow!("Invalid VM XML: no devices section found"));
        };

        // Ticking the GPU as unavailable
        if let Some(gpu) = self.devices.iter_mut().find(|d| d.id == gpu_id) {
            gpu.is_available = false;
        }

        info!("GPU {} successfully configured for VM attachment", gpu_id);
        
        Ok(new_xml)
    }

    fn calculate_next_free_slot(&self, vm_xml: &str) -> Result<u8> {
        // Parse existing PCI slots from VM XML
        let mut used_slots = Vec::new();
        for line in vm_xml.lines() {
            if line.contains("slot='0x") {
                if let Some(slot_str) = line.split("slot='0x").nth(1) {
                    if let Some(slot_hex) = slot_str.split('\'').next() {
                        if let Ok(slot) = u8::from_str_radix(slot_hex, 16) {
                            used_slots.push(slot);
                        }
                    }
                }
            }
        }

        // Find first available slot (starting from 0x02)
        for slot in 2..32 {
            if !used_slots.contains(&slot) {
                return Ok(slot);
            }
        }

        Err(anyhow::anyhow!("No free PCI slots available"))
    }

    pub fn get_devices(&self) -> Vec<GPUDevice> {
        self.devices.clone()
    }
}

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