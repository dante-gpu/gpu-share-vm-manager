use serde::{Deserialize, Serialize};
use std::path::PathBuf;
// use anyhow::Result;

// The configuration for our virtual machines
// Because every VM needs a good config, like every developer needs coffee! ☕
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct VMConfig {
    pub name: String,
    pub memory_kb: u64,  // Memory in kilobytes (we're old school!)
    pub vcpus: u32,      // Virtual CPUs (the more the merrier!)
    pub disk_path: PathBuf,  // Where we store our VM's digital dreams
    pub disk_size_gb: u64,   // How much space for those dreams
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VirtualMachine {
    pub id: String,
    pub name: String,
    pub status: VMStatus,
    pub resources: VMResources,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum VMStatus {
    Running,    // Vrooooom! 🏎️
    Stopped,    // Taking a nap 😴
    Failed,     // Houston, we have a problem! 🚨
    Creating,   // Building the dream machine 🏗️
    Deleting,   // Time to say goodbye 👋
}

impl From<u32> for VMStatus {
    fn from(state: u32) -> Self {
        match state {
            1 => VMStatus::Running,
            5 => VMStatus::Stopped,
            _ => VMStatus::Failed,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VMResources {
    pub cpu_cores: u32,      // The brain power! 🧠
    pub memory_mb: u64,      // RAM - because we all need memories
    pub gpu_attached: bool,  // Got that gaming power? 🎮
}

/*  Cross-platform VM configuration
Handles platform-specific virtualization settings
*/
impl VMConfig {
    // Generate platform-optimized XML configuration
    pub fn to_platform_xml(&self) -> Result<String> {
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

        format!(
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
            self.name,
            self.memory_kb,
            self.vcpus,
            arch,
            machine_type,
            self.platform_specific_devices()
        )
    }

    /// Platform-specific device configuration
    fn platform_specific_devices(&self) -> String {
        match Platform::current() {
            Platform::MacOS => {
                // Apple Silicon T2 security device emulation
                r#"
                <devices>
                    <controller type='usb' model='qemu-xhci'/>
                    <input type='keyboard' bus='virtio'/>
                    <input type='mouse' bus='virtio'/>
                    <graphics type='cocoa'/>
                </devices>
                "#
            }
            Platform::Windows => {
                // Windows Hyper-V enlightenment features
                r#"
                <features>
                    <hyperv>
                        <relaxed state='on'/>
                        <vapic state='on'/>
                        <spinlocks state='on' retries='8191'/>
                    </hyperv>
                </features>
                "#
            }
            _ => {
                // Standard QEMU devices for Linux
                r#"
                <devices>
                    <emulator>/usr/bin/qemu-system-x86_64</emulator>
                    <disk type='file' device='disk'>
                        <driver name='qemu' type='qcow2'/>
                        <source file='{}'/>
                        <target dev='vda' bus='virtio'/>
                    </disk>
                </devices>
                "#
            }
        }.to_string()
    }
}