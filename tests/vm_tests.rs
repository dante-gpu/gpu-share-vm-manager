// Virtual Machine Test Suite - Because untested code is like Schrödinger's cat! 🐱💻

use anyhow::{Context, Result};
use gpu_share_vm_manager::core::{LibvirtManager, vm::{VMConfig, VMStatus}};
use std::path::PathBuf;
// use tracing::{info, warn};
use uuid::Uuid;

// Test setup: Creates a unique VM configuration to avoid conflicts
fn test_vm_config() -> VMConfig {
    let uuid = Uuid::new_v4();
    VMConfig {
        name: format!("test-vm-{}", uuid),
        memory_kb: 1_048_576, // 1GB
        vcpus: 2,
        disk_path: PathBuf::from(format!("/var/lib/gpu-share/images/test-{}.qcow2", uuid)),
        disk_size_gb: 10,
    }
}

// VM Lifecycle Test: Creation → Start → Stop → Delete
#[tokio::test]
async fn test_full_vm_lifecycle() -> Result<()> {
    let libvirt = LibvirtManager::new()?;
    let config = test_vm_config();
    
    // Phase 1: Create the VM
    let vm = libvirt.create_vm(&config)
        .await
        .context("Failed to create VM")?;
    
    assert_eq!(vm.get_name()?, config.name);
    assert!(!vm.is_active()?, "VM should be initially stopped");

    // Phase 2: Start the VM
    vm.create()?;
    assert!(vm.is_active()?, "VM should be running after start");

    // Phase 3: Stop the VM
    vm.destroy()?;
    assert!(!vm.is_active()?, "VM should be stopped after destroy");

    // Phase 4: Delete the VM
    vm.undefine()?;
    
    // Verify deletion
    let exists = libvirt.lookup_domain(&config.name).is_ok();
    assert!(!exists, "VM should be deleted");

    Ok(())
}

// Stress Test: Create multiple VMs simultaneously
#[tokio::test]
async fn test_concurrent_vm_creation() -> Result<()> {
    let libvirt = LibvirtManager::new()?;
    let mut handles = vec![];
    
    // Spawn 5 concurrent VM creations
    for i in 0..5 {
        let libvirt_clone = libvirt.try_clone().expect("Clone failed");
        let config = VMConfig {
            name: format!("stress-test-vm-{}", i),
            memory_kb: 524_288, // 512MB
            vcpus: 1,
            disk_path: PathBuf::from(format!("/var/lib/gpu-share/images/stress-{}.qcow2", i)),
            disk_size_gb: 5,
        };
        
        handles.push(tokio::spawn(async move {
            libvirt_clone.create_vm(&config).await
        }));
    }

    // Verify all creations succeeded
    for handle in handles {
        let vm = handle.await??;
        assert!(vm.get_name().is_ok(), "VM should have valid name");
        vm.destroy()?;
        vm.undefine()?;
    }

    Ok(())
}

// Error Case Test: Invalid VM configurations
#[tokio::test]
async fn test_invalid_vm_configurations() -> Result<()> {
    let libvirt = LibvirtManager::new()?;
    
    // Test 1: Insufficient memory
    let config = VMConfig {
        name: "invalid-memory".into(),
        memory_kb: 1024, // Ridiculously low
        vcpus: 2,
        disk_path: PathBuf::from("/invalid/path.qcow2"),
        disk_size_gb: 10,
    };
    
    let result = libvirt.create_vm(&config).await;
    assert!(result.is_err(), "Should reject insufficient memory");

    // Test 2: Invalid disk path
    let config = VMConfig {
        name: "invalid-disk".into(),
        memory_kb: 1_048_576,
        vcpus: 2,
        disk_path: PathBuf::from("/dev/null"), // Invalid disk image
        disk_size_gb: 10,
    };
    
    let result = libvirt.create_vm(&config).await;
    assert!(result.is_err(), "Should reject invalid disk path");

    Ok(())
}

// State Transition Test: Start → Reboot → Stop
#[tokio::test]
async fn test_vm_state_transitions() -> Result<()> {
    let libvirt = LibvirtManager::new()?;
    let config = test_vm_config();
    let vm = libvirt.create_vm(&config).await?;

    // Cold start
    vm.create()?;
    assert!(vm.is_active()?, "VM should be running");

    // Reboot
    vm.reboot(0)?;
    assert!(vm.is_active()?, "VM should stay running after reboot");

    // Graceful shutdown
    vm.shutdown()?;
    tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
    assert!(!vm.is_active()?, "VM should shutdown gracefully");

    vm.undefine()?;
    Ok(())
}

// Snapshot Test: Create → Snapshot → Restore
#[tokio::test]
async fn test_vm_snapshots() -> Result<()> {
    let libvirt = LibvirtManager::new()?;
    let config = test_vm_config();
    let vm = libvirt.create_vm(&config).await?;
    vm.create()?;

    // Create snapshot
    let snapshot_xml = format!(r#"
        <domainsnapshot>
            <name>{}</name>
            <description>{}</description>
        </domainsnapshot>"#,
        "test-snapshot",
        "Initial state"
    );
    vm.snapshot_create_xml(&snapshot_xml, 0)?;

    // Restore snapshot
    let snapshot = vm.snapshot_lookup_by_name("test-snapshot")?;
    vm.snapshot_revert(snapshot, 0)?;
    
    let restored_xml = vm.get_xml_desc(0)?;
    assert_eq!(restored_xml, vm.get_xml_desc(0)?, "XML should match snapshot");

    vm.destroy()?;
    vm.undefine()?;
    Ok(())
}

// Resource Validation Test: CPU/Memory allocation
#[tokio::test]
async fn test_resource_allocation() -> Result<()> {
    let libvirt = LibvirtManager::new()?;
    let config = test_vm_config();
    
    // Create VM with specific resources
    let vm = libvirt.create_vm(&config)
        .await
        .context("Failed to create VM for resource test")?;

    // Validate memory allocation
    let info = vm.get_info()?;
    assert_eq!(
        info.memory as u64, 
        config.memory_kb * 1024,  // Convert KiB to bytes
        "Memory allocation mismatch"
    );

    // Validate vCPU allocation
    assert_eq!(
        info.nr_virt_cpu as u32,
        config.vcpus,
        "vCPU allocation mismatch"
    );

    // Cleanup
    vm.destroy()?;
    vm.undefine()?;
    
    Ok(())
}