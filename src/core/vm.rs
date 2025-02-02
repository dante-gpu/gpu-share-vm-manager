use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use anyhow::{Result, Context};
use crate::utils::Platform;
use crate::core::docker_manager::DockerManager;

/// Virtual Machine Configuration
/// Platform-agnostic configuration with platform-specific optimizations
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct VMConfig {
    pub name: String,
    pub memory_kb: u64,
    pub vcpus: u32,
    pub disk_path: PathBuf,
    pub disk_size_gb: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gpu_passthrough: Option<crate::gpu::device::GPUConfig>,
}

/// Virtual Machine Runtime State
#[derive(Debug, Serialize, Deserialize)]
pub struct VirtualMachine {
    pub id: String,
    pub name: String,
    pub status: VMStatus,
    pub resources: VMResources,
    pub host_platform: Platform,
    pub vcpus: u32,
    pub memory_kb: u64,
}

/// Virtual Machine Status
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub enum VMStatus {
    Running,
    Stopped,
    Paused,
    Suspended,
    Crashed,
    Creating,
    Migrating,
    Deleting,
    Unknown,
}

/// Virtual Machine Resource Utilization
#[derive(Debug, Serialize, Deserialize)]
pub struct VMResources {
    pub cpu_usage: f32,
    pub memory_usage: f32,
    pub disk_usage: f32,
    pub network_rx: u64,
    pub network_tx: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gpu_usage: Option<f32>,
}

impl VMConfig {
    /// Create new VM configuration with platform defaults
    pub fn new(name: &str, memory_gb: u64, vcpus: u32) -> Self {
        let mut disk_path = PathBuf::new();
        
        #[cfg(target_os = "linux")]
        disk_path.push("/var/lib/libvirt/images");
        
        #[cfg(target_os = "macos")]
        disk_path.push("/Users/Shared/VirtualMachines");
        
        #[cfg(target_os = "windows")]
        disk_path.push("C:\\VirtualMachines");

        disk_path.push(format!("{}.qcow2", name));

        VMConfig {
            name: name.to_string(),
            memory_kb: memory_gb * 1024 * 1024,
            vcpus,
            disk_path,
            disk_size_gb: 20, // Default size
            gpu_passthrough: None,
        }
    }

    /// Generate platform-optimized XML configuration
    pub fn to_xml(&self) -> Result<String> {
        let arch = match Platform::current() {
            Platform::Linux | Platform::Windows => "x86_64",
            Platform::MacOS => {
                if cfg!(target_arch = "aarch64") {
                    "aarch64"
                } else {
                    "x86_64"
                }
            }
            _ => "x86_64",
        };

        let machine_type = match Platform::current() {
            Platform::Linux => "pc-q35-6.2",
            Platform::MacOS => "virt",
            Platform::Windows => "pc-q35-6.2",
            _ => "pc-q35-6.2",
        };

        let devices = self.platform_specific_devices()?;

        Ok(format!(
            r#"
            <domain type='kvm'>
                <name>{}</name>
                <memory unit='KiB'>{}</memory>
                <vcpu placement='static'>{}</vcpu>
                <os>
                    <type arch='{}' machine='{}'>hvm</type>
                    <boot dev='hd'/>
                </os>
                {}
            </domain>
            "#,
            self.name, self.memory_kb, self.vcpus, arch, machine_type, devices
        ))
    }

    /// Platform-specific device configuration
    fn platform_specific_devices(&self) -> Result<String> {
        let mut devices = String::new();

        // Common devices
        devices.push_str(
            r#"
            <devices>
                <console type='pty'/>
                <channel type='unix'>
                    <target type='virtio' name='org.qemu.guest_agent.0'/>
                </channel>
            "#
        );

        // Platform-specific devices
        match Platform::current() {
            Platform::MacOS => {
                devices.push_str(
                    r#"
                    <controller type='usb' model='qemu-xhci'/>
                    <input type='keyboard' bus='virtio'/>
                    <input type='mouse' bus='virtio'/>
                    <graphics type='cocoa'/>
                    "#
                );
            }
            Platform::Windows => {
                devices.push_str(
                    r#"
                    <features>
                        <hyperv>
                            <relaxed state='on'/>
                            <vapic state='on'/>
                            <spinlocks state='on' retries='8191'/>
                        </hyperv>
                    </features>
                    <video>
                        <model type='qxl' ram='65536' vram='65536'/>
                    </video>
                    "#
                );
            }
            _ => {
                devices.push_str(
                    r#"
                    <video>
                        <model type='virtio'/>
                    </video>
                    "#
                );
            }
        }

        // GPU passthrough
        if let Some(gpu) = &self.gpu_passthrough {
            let pci_parts: Vec<&str> = gpu.gpu_id.split(':').collect();
            if pci_parts.len() != 3 {
                return Err(anyhow::anyhow!("Invalid PCI address format"));
            }
            devices.push_str(&format!(
                r#"
                <hostdev mode='subsystem' type='pci' managed='yes'>
                    <source>
                        <address domain='0x0000' bus='{}' slot='{}' function='{}'/>
                    </source>
                </hostdev>
                "#,
                pci_parts[0],
                pci_parts[1],
                pci_parts[2].trim_end_matches(".0")
            ));
        }

        devices.push_str("</devices>");
        Ok(devices)
    }
}

impl VirtualMachine {
    /// Start VM
    pub async fn start(&self, docker: &DockerManager) -> Result<()> {
        docker.start_container(&self.id)
            .await
            .context("Container başlatılamadı")?;
        Ok(())
    }

    /// Get memory statistics
    pub fn memory_stats(&self) -> Result<Vec<u64>> {
        // Implementation varies by platform
        Ok(vec![
            self.resources.memory_usage as u64,
            self.resources.memory_usage as u64 * 1024,
        ])
    }

    /// Get vCPU statistics
    pub fn vcpu_stats(&self) -> Result<Vec<u64>> {
        Ok(vec![
            self.resources.cpu_usage as u64,
            self.vcpus as u64,
        ])
    }
}

impl Default for VMResources {
    fn default() -> Self {
        Self {
            cpu_usage: 0.0,
            memory_usage: 0.0,
            disk_usage: 0.0,
            network_rx: 0,
            network_tx: 0,
            gpu_usage: None,
        }
    }
}

impl From<u32> for VMStatus {
    fn from(state: u32) -> Self {
        match state {
            1 => VMStatus::Running,
            2 => VMStatus::Stopped,
            3 => VMStatus::Paused,
            4 => VMStatus::Suspended,
            5 => VMStatus::Crashed,
            _ => VMStatus::Unknown,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vm_config_creation() {
        let config = VMConfig::new("test-vm", 4, 2);
        assert_eq!(config.memory_kb, 4 * 1024 * 1024);
        assert!(config.disk_path.to_string_lossy().contains("test-vm"));
    }

    #[test]
    fn test_vm_status_conversion() {
        assert_eq!(VMStatus::from(1), VMStatus::Running);
        assert_eq!(VMStatus::from(5), VMStatus::Crashed);
        assert_eq!(VMStatus::from(99), VMStatus::Unknown);
    }

    #[test]
    fn test_xml_generation() {
        let config = VMConfig::new("test-xml", 2, 1);
        let xml = config.to_xml().unwrap();
        assert!(xml.contains("test-xml"));
        assert!(xml.contains("KiB"));
    }
}