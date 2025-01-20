use anyhow::Result;
use tracing::{info, error};
use virt::connect::Connect;
use virt::domain::Domain;

// uwu time to manage some VMs! >.
pub struct LibvirtManager {
    conn: Connect,
}

impl LibvirtManager {
    // hehe connect me senpai! ^_^
    pub fn new() -> Result<Self> {
        // YOLO: connecting to local hypervisor
        let conn = match Connect::open("qemu:///system") {
            Ok(c) => c,
            Err(e) => {
                error!("Failed to connect to libvirt: {}", e);
                return Err(anyhow::anyhow!("Libvirt connection failed"));
            }
        };

        info!("Successfully connected to libvirt");
        Ok(Self { conn })
    }

    // omae wa mou shindeiru (VM creation time!)
    pub fn create_vm(&self, name: &str, memory_kb: u64, vcpus: u32) -> Result<Domain> {
        // XML goes brrrr
        let xml = format!(r#"
            <domain type='kvm'>
                <name>{}</name>
                <memory unit='KiB'>{}</memory>
                <vcpu placement='static'>{}</vcpu>
                <os>
                    <type arch='x86_64' machine='pc-q35-7.0'>hvm</type>
                    <boot dev='hd'/>
                </os>
                <!-- TODO: add more fancy stuff here -->
            </domain>
        "#, name, memory_kb, vcpus);

        match self.conn.domain_define_xml(&xml) {
            Ok(dom) => {
                info!("VM {} created successfully", name);
                Ok(dom)
            }
            Err(e) => {
                error!("Failed to create VM {}: {}", name, e);
                Err(anyhow::anyhow!("VM creation failed"))
            }
        }
    }

    // sayonara VM-chan!
    pub fn destroy_vm(&self, name: &str) -> Result<()> {
        if let Ok(domain) = self.conn.lookup_domain_by_name(name) {
            domain.destroy()?;
            info!("VM {} destroyed", name);
        }
        Ok(())
    }
}

// oof size: LARGE - error handling time
#[derive(Debug, thiserror::Error)]
pub enum LibvirtError {
    #[error("Connection failed: {0}")]
    ConnectionError(String),
    #[error("VM operation failed: {0}")]
    VMError(String),
}