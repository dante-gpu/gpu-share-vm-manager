use anyhow::Result;
use tracing::{info, error};
use virt::connect::Connect;
use virt::domain::Domain;
// use virt::sys;

// time to manage some VMs! >.
pub struct LibvirtManager {
    conn: Connect,
}

impl LibvirtManager {
    // hehe connect me senpai! ^_^
    pub fn new() -> Result<Self> {
        info!("Initializing libvirt connection");
        let conn = Connect::open(Some("qemu:///system"))?;
        Ok(Self { conn })
    }

    // omae wa mou shindeiru (VM creation time!)
    pub fn create_vm(&self, name: &str, memory_kb: u64, vcpus: u32) -> Result<Domain> {
        let xml = format!(
            r#"
            <domain type='kvm'>
                <name>{}</name>
                <memory unit='KiB'>{}</memory>
                <vcpu placement='static'>{}</vcpu>
                <os>
                    <type arch='x86_64' machine='pc-q35-6.2'>hvm</type>
                    <boot dev='hd'/>
                </os>
                <devices>
                    <emulator>/usr/bin/qemu-system-x86_64</emulator>
                </devices>
            </domain>
            "#,
            name, memory_kb, vcpus
        );

        match Domain::create_xml(&self.conn, &xml, 0) {
            Ok(domain) => Ok(domain),
            Err(e) => Err(anyhow::anyhow!("Failed to create VM: {}", e))
        }
    }

    // sayonara VM-chan!
    pub fn destroy_vm(&self, name: &str) -> Result<()> {
        if let Ok(domain) = Domain::lookup_by_name(&self.conn, name) {
            domain.destroy()?;
            info!("VM {} destroyed", name);
        }
        Ok(())
    }

    pub fn list_domains(&self) -> Result<Vec<Domain>> {
        match self.conn.list_domains() {
            Ok(domain_ids) => {
                let mut domains = Vec::new();
                for id in domain_ids {
                    if let Ok(domain) = Domain::lookup_by_id(&self.conn, id) {
                        domains.push(domain);
                    }
                }
                Ok(domains)
            },
            Err(e) => Err(anyhow::anyhow!("Failed to list domains: {}", e))
        }
    }
    
    pub fn lookup_domain(&self, id: &str) -> Result<Domain> {
        // Önce UUID olarak dene
        if let Ok(domain) = Domain::lookup_by_uuid_string(&self.conn, id) {
            return Ok(domain);
        }
        
        // UUID olarak bulunamazsa isim olarak dene
        match Domain::lookup_by_name(&self.conn, id) {
            Ok(domain) => Ok(domain),
            Err(e) => Err(anyhow::anyhow!("Failed to find domain: {}", e))
        }
    }

    pub fn list_active_domains(&self) -> Result<Vec<Domain>> {
        match self.conn.list_domains() {
            Ok(domain_ids) => {
                let mut domains = Vec::new();
                for id in domain_ids {
                    if let Ok(domain) = Domain::lookup_by_id(&self.conn, id) {
                        if let Ok((state, _)) = domain.get_state() {
                            if state == 1 { // VIR_DOMAIN_RUNNING
                                domains.push(domain);
                            }
                        }
                    }
                }
                Ok(domains)
            },
            Err(e) => Err(anyhow::anyhow!("Failed to list active domains: {}", e))
        }
    }

    pub fn list_inactive_domains(&self) -> Result<Vec<Domain>> {
        match self.conn.list_defined_domains() {
            Ok(names) => {
                let mut domains = Vec::new();
                for name in names {
                    if let Ok(domain) = Domain::lookup_by_name(&self.conn, &name) {
                        domains.push(domain);
                    }
                }
                Ok(domains)
            },
            Err(e) => Err(anyhow::anyhow!("Failed to list inactive domains: {}", e))
        }
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