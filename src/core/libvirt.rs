use anyhow::Result;
use tracing::info;
use virt::connect::Connect;
use virt::domain::Domain;
use crate::core::vm::VMConfig;
// use virt::sys;

// time to manage some VMs! >.
#[derive(Clone)]
#[allow(dead_code)]  
pub struct LibvirtManager {
    conn: Connect,
}

impl LibvirtManager {
    // hehe connect me senpai! ^_^
    pub fn new() -> Result<Self> {
        let conn = Connect::open(Some("qemu:///system"))?;
        Ok(Self { conn })
    }

    // omae wa mou shindeiru (VM creation time!)
    pub async fn create_vm(&self, config: &VMConfig) -> Result<Domain> {
        info!("Creating a new VM: {} - another star is born! ✨", config.name);
        
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
                    <disk type='file' device='disk'>
                        <driver name='qemu' type='qcow2'/>
                        <source file='{}'/>
                        <target dev='vda' bus='virtio'/>
                    </disk>
                </devices>
            </domain>
            "#,
            config.name, config.memory_kb, config.vcpus,
            config.disk_path.display()
        );

        match Domain::create_xml(&self.conn, &xml, 0) {
            Ok(domain) => Ok(domain),
            Err(e) => Err(anyhow::anyhow!("Failed to create VM: {}", e))
        }
    }

    // sayonara VM-chan!
    #[allow(dead_code)]
    pub async fn destroy_vm(&self, name: &str) -> Result<()> {
        info!("Destroying VM {} - it's not goodbye, it's see you later!", name);
        if let Ok(domain) = Domain::lookup_by_name(&self.conn, name) {
            domain.destroy()?;
        }
        Ok(())
    }

    pub fn list_domains(&self) -> Result<Vec<Domain>> {
        info!("Listing all domains - time for roll call!");
        let domain_ids = self.conn.list_domains()?;
        let mut domains = Vec::new();
        
        for id in domain_ids {
            if let Ok(domain) = Domain::lookup_by_id(&self.conn, id) {
                domains.push(domain);
            }
        }
        
        Ok(domains)
    }
    
    pub fn lookup_domain(&self, id: &str) -> Result<Domain> {
        if let Ok(domain) = Domain::lookup_by_uuid_string(&self.conn, id) {
            return Ok(domain);
        }
        
        match Domain::lookup_by_name(&self.conn, id) {
            Ok(domain) => Ok(domain),
            Err(e) => Err(anyhow::anyhow!("Failed to find domain: {}", e))
        }
    }

    #[allow(dead_code)]
    pub async fn list_active_domains(&self) -> Result<Vec<String>> {
        let domain_ids = self.conn.list_domains()
            .map_err(|e| anyhow::anyhow!("Failed to list active domains: {:?}", e))?;
        
        let mut active_domains = Vec::new();
        for id in domain_ids {
            if let Ok(domain) = Domain::lookup_by_id(&self.conn, id) {
                if let Ok((state, _)) = domain.get_state() {
                    if state == 1 { // VIR_DOMAIN_RUNNING
                        if let Ok(name) = domain.get_name() {
                            active_domains.push(name);
                        }
                    }
                }
            }
        }
        Ok(active_domains)
    }

    #[allow(dead_code)] // TODO: Implement this - @virjilakrum
    pub async fn list_inactive_domains(&self) -> Result<Vec<String>> {
        self.conn.list_defined_domains()
            .map_err(|e| anyhow::anyhow!("Failed to list inactive domains: {:?}", e))
    }

    pub async fn start_domain(&self, id: &str) -> Result<(), anyhow::Error> {
        let domain = self.lookup_domain(id)?;
        domain.create()
            .map(|_| ())
            .map_err(|e| anyhow::anyhow!("Failed to start domain: {}", e))
    }

    pub async fn stop_domain(&self, id: &str) -> Result<(), anyhow::Error> {
        let domain = self.lookup_domain(id)?;
        domain.shutdown()
            .map(|_| ())
            .map_err(|e| anyhow::anyhow!("Failed to stop domain: {}", e))
    }

    pub async fn delete_domain(&self, id: &str) -> Result<(), anyhow::Error> {
        let domain = self.lookup_domain(id)?;
        domain.undefine()
            .map(|_| ())
            .map_err(|e| anyhow::anyhow!("Failed to delete domain: {}", e))
    }
}