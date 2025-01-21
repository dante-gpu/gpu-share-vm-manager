/*
* GPU Passthrough Implementation
* ----------------------------------------
* @author: @virjilakrum
* @module: gpu/passthrough
*
* Technical Overview:
* -----------------
* This module implements GPU passthrough functionality for virtual machines using
* IOMMU (Input-Output Memory Management Unit) virtualization. The implementation
* follows a modular architecture with three main components:
*
* Core Components:
* --------------
* 1. PassthroughManager: Main orchestrator that coordinates:
*    - IOMMU management (group isolation)
*    - Driver management (unbinding/binding)
*    - Device management (verification/monitoring)
*
* 2. IOMMU Management:
*    - Validates IOMMU support via kernel messages
*    - Manages IOMMU groups for device isolation
*    - Handles group viability checks
*
* 3. Driver Operations:
*    - Manages driver unbinding from GPU
*    - Handles VFIO driver binding
*    - Verifies driver states
*
* 4. Device Management:
*    - Validates device readiness
*    - Monitors power states
*    - Verifies memory BAR configuration
*
* Error Handling:
* -------------
* - Comprehensive error types via GpuError
* - Recovery mechanisms for failed operations
* - Graceful rollback capabilities
*
* Security Considerations:
* ---------------------
* - IOMMU group isolation enforcement
* - Safe driver operations
* - Configurable security parameters via PassthroughConfig
*
* Performance Notes:
* ---------------
* - Minimal overhead for IOMMU operations
* - Efficient driver switching
* - Optimized device verification
*
* Usage Warning:
* ------------
* GPU passthrough operations can affect system stability if not properly
* configured. Ensure proper IOMMU support and follow security guidelines.
*/

use crate::errors::{GpuShareError, GpuError, ErrorRecovery};
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::{info, warn, error};

pub struct PassthroughManager {
    iommu_manager: IommuManager,
    driver_manager: DriverManager,
    device_manager: DeviceManager,
}

impl PassthroughManager {
    pub fn new() -> Result<Self, GpuShareError> {
        info!("Initializing GPU Passthrough Manager");
        
        // Check IOMMU support first
        if !Self::check_iommu_support()? {
            return Err(GpuError::IommuError {
                message: "IOMMU not enabled in system".to_string(),
                group_id: None,
            }.into());
        }

        Ok(Self {
            iommu_manager: IommuManager::new()?,
            driver_manager: DriverManager::new()?,
            device_manager: DeviceManager::new()?,
        })
    }

    fn check_iommu_support() -> Result<bool, GpuShareError> {
        let dmesg_output = Command::new("dmesg")
            .output()
            .map_err(|e| GpuError::SystemError {
                message: format!("Failed to execute dmesg: {}", e),
            })?;

        let output = String::from_utf8_lossy(&dmesg_output.stdout);
        Ok(output.contains("IOMMU enabled") || output.contains("AMD-Vi enabled"))
    }

    pub fn prepare_gpu_passthrough(&mut self, gpu_id: &str) -> Result<(), GpuShareError> {
        info!("Preparing GPU {} for passthrough", gpu_id);

        // Get GPU IOMMU group
        let iommu_group = self.iommu_manager.get_gpu_iommu_group(gpu_id)?;
        
        // Unbind current driver
        self.driver_manager.unbind_current_driver(gpu_id)?;
        
        // Bind to VFIO driver
        self.driver_manager.bind_to_vfio(gpu_id, iommu_group)?;
        
        // Verify device is ready
        self.device_manager.verify_device_ready(gpu_id)?;

        info!("GPU {} successfully prepared for passthrough", gpu_id);
        Ok(())
    }
}

struct IommuManager {
    iommu_groups_path: PathBuf,
}

impl IommuManager {
    fn new() -> Result<Self, GpuShareError> {
        Ok(Self {
            iommu_groups_path: PathBuf::from("/sys/kernel/iommu_groups"),
        })
    }

    fn get_gpu_iommu_group(&self, gpu_id: &str) -> Result<u32, GpuShareError> {
        let gpu_path = Path::new("/sys/bus/pci/devices").join(gpu_id);
        
        let iommu_group_link = fs::read_link(gpu_path.join("iommu_group"))
            .map_err(|e| GpuError::IommuError {
                message: format!("Failed to read IOMMU group symlink: {}", e),
                group_id: None,
            })?;

        let group_id = iommu_group_link
            .file_name()
            .and_then(|n| n.to_str())
            .and_then(|s| s.parse::<u32>().ok())
            .ok_or_else(|| GpuError::IommuError {
                message: "Invalid IOMMU group format".to_string(),
                group_id: None,
            })?;

        Ok(group_id)
    }
}

struct DriverManager {
    drivers_path: PathBuf,
}

impl DriverManager {
    fn new() -> Result<Self, GpuShareError> {
        Ok(Self {
            drivers_path: PathBuf::from("/sys/bus/pci/drivers"),
        })
    }

    fn unbind_current_driver(&self, gpu_id: &str) -> Result<(), GpuShareError> {
        // Get current driver
        let current_driver = self.get_current_driver(gpu_id)?;
        
        // Write to unbind
        let unbind_path = self.drivers_path.join(&current_driver).join("unbind");
        fs::write(&unbind_path, gpu_id).map_err(|e| GpuError::DriverError {
            message: format!("Failed to unbind driver: {}", e),
            driver_name: current_driver,
        })?;

        Ok(())
    }

    fn bind_to_vfio(&self, gpu_id: &str, iommu_group: u32) -> Result<(), GpuShareError> {
        // Write device ID to vfio-pci new_id
        let new_id_path = self.drivers_path.join("vfio-pci/new_id");
        let device_info = self.get_device_info(gpu_id)?;
        fs::write(new_id_path, device_info).map_err(|e| GpuError::DriverError {
            message: format!("Failed to bind to VFIO: {}", e),
            driver_name: "vfio-pci".to_string(),
        })?;

        Ok(())
    }

    fn get_current_driver(&self, gpu_id: &str) -> Result<String, GpuShareError> {
        let driver_link = fs::read_link(format!("/sys/bus/pci/devices/{}/driver", gpu_id))
            .map_err(|e| GpuError::DriverError {
                message: format!("Failed to read driver symlink: {}", e),
                driver_name: String::new(),
            })?;

        Ok(driver_link.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string())
    }

    fn get_device_info(&self, gpu_id: &str) -> Result<String, GpuShareError> {
        let vendor_id = fs::read_to_string(format!("/sys/bus/pci/devices/{}/vendor", gpu_id))?;
        let device_id = fs::read_to_string(format!("/sys/bus/pci/devices/{}/device", gpu_id))?;
        Ok(format!("{} {}", vendor_id.trim(), device_id.trim()))
    }
}

// src/gpu/passthrough.rs (devam)

struct DeviceManager {
    pci_devices_path: PathBuf,
}

impl DeviceManager {
    fn new() -> Result<Self, GpuShareError> {
        Ok(Self {
            pci_devices_path: PathBuf::from("/sys/bus/pci/devices"),
        })
    }

    fn verify_device_ready(&self, gpu_id: &str) -> Result<(), GpuShareError> {
        // Check if device exists
        let device_path = self.pci_devices_path.join(gpu_id);
        if !device_path.exists() {
            return Err(GpuError::NotFound {
                gpu_id: gpu_id.to_string(),
            }.into());
        }

        // Verify VFIO binding
        let current_driver = self.get_current_driver(gpu_id)?;
        if current_driver != "vfio-pci" {
            return Err(GpuError::DriverError {
                message: format!("Device not bound to VFIO-PCI, current driver: {}", current_driver),
                driver_name: current_driver,
            }.into());
        }

        // Check power state
        self.verify_power_state(gpu_id)?;

        // Verify memory BAR
        self.verify_memory_bar(gpu_id)?;

        Ok(())
    }

    fn get_current_driver(&self, gpu_id: &str) -> Result<String, GpuShareError> {
        let driver_link = fs::read_link(self.pci_devices_path.join(gpu_id).join("driver"))
            .map_err(|e| GpuError::SystemError {
                message: format!("Failed to read driver link: {}", e),
            })?;

        Ok(driver_link
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string())
    }

    fn verify_power_state(&self, gpu_id: &str) -> Result<(), GpuShareError> {
        let power_state_path = self.pci_devices_path
            .join(gpu_id)
            .join("power_state");

        let power_state = fs::read_to_string(&power_state_path)
            .map_err(|e| GpuError::SystemError {
                message: format!("Failed to read power state: {}", e),
            })?;

        if power_state.trim() != "D0" {
            return Err(GpuError::SystemError {
                message: format!("Device not in active power state: {}", power_state.trim()),
            }.into());
        }

        Ok(())
    }

    fn verify_memory_bar(&self, gpu_id: &str) -> Result<(), GpuShareError> {
        let resource_path = self.pci_devices_path
            .join(gpu_id)
            .join("resource");

        let file = File::open(&resource_path)
            .map_err(|e| GpuError::SystemError {
                message: format!("Failed to read PCI resources: {}", e),
            })?;

        let reader = BufReader::new(file);
        let mut valid_bar = false;

        for line in reader.lines() {
            let line = line?;
            if line.contains("Memory at") {
                valid_bar = true;
                break;
            }
        }

        if !valid_bar {
            return Err(GpuError::SystemError {
                message: "No valid memory BAR found".to_string(),
            }.into());
        }

        Ok(())
    }
}

// VFIO Group Management
#[derive(Debug)]
pub struct VfioGroup {
    group_id: u32,
    devices: Vec<String>,
}

impl VfioGroup {
    pub fn new(group_id: u32) -> Result<Self, GpuShareError> {
        Ok(Self {
            group_id,
            devices: Vec::new(),
        })
    }

    pub fn add_device(&mut self, device_id: String) -> Result<(), GpuShareError> {
        if !self.devices.contains(&device_id) {
            self.devices.push(device_id);
        }
        Ok(())
    }

    pub fn is_viable(&self) -> Result<bool, GpuShareError> {
        // Check if all devices in group can be passed through
        for device_id in &self.devices {
            if !self.is_device_eligible(device_id)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn is_device_eligible(&self, device_id: &str) -> Result<bool, GpuShareError> {
        let class_path = PathBuf::from("/sys/bus/pci/devices")
            .join(device_id)
            .join("class");

        let class = fs::read_to_string(class_path)
            .map_err(|e| GpuError::SystemError {
                message: format!("Failed to read device class: {}", e),
            })?;

        // Check if device is passthrough compatible
        Ok(!class.trim().starts_with("0x060")) // Exclude PCI bridges
    }
}

impl ErrorRecovery for PassthroughManager {
    fn recover(&self) -> Result<(), GpuShareError> {
        info!("Attempting to recover GPU passthrough configuration");
        
        // Reset VFIO bindings
        self.driver_manager.reset_vfio_bindings()?;
        
        // Re-scan PCI bus
        self.device_manager.rescan_pci_bus()?;
        
        // Verify IOMMU groups
        self.iommu_manager.verify_groups()?;
        
        Ok(())
    }

    fn rollback(&self) -> Result<(), GpuShareError> {
        warn!("Rolling back GPU passthrough changes");
        
        // Restore original drivers
        self.driver_manager.restore_original_drivers()?;
        
        // Reset device state
        self.device_manager.reset_devices()?;
        
        Ok(())
    }
}

pub struct PassthroughConfig {
    pub enable_unsafe_interrupts: bool,
    pub enable_acs_override: bool,
    pub enable_power_management: bool,
}

impl Default for PassthroughConfig {
    fn default() -> Self {
        Self {
            enable_unsafe_interrupts: false,
            enable_acs_override: false,
            enable_power_management: true,
        }
    }
}

