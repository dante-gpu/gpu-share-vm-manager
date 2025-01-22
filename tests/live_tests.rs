use std::sync::Arc;
use tokio;
use tracing::{info, error};

use crate::core::{LibvirtManager, VMConfig};
use crate::gpu::{GPUManager, GPUConfig};
use crate::monitoring::MetricsCollector;

async fn setup_libvirt() -> Result<LibvirtManager, Box<dyn std::error::Error>> {
    // Real libvirt connection
    let manager = LibvirtManager::new("qemu:///system")?;
    
    // Cleanup old test domains
    for domain in manager.list_all_domains()? {
        if domain.get_name()?.starts_with("test-") {
            info!("Cleaning up old test domain: {}", domain.get_name()?);
            if domain.is_active()? {
                domain.destroy()?;
            }
            domain.undefine()?;
        }
    }

    Ok(manager)
}

#[tokio::test]
async fn test_real_vm_creation() -> Result<(), Box<dyn std::error::Error>> {
    let libvirt = setup_libvirt().await?;
    
    let config = VMConfig {
        name: "test-vm-1".to_string(),
        memory_kb: 4 * 1024 * 1024, // 4GB
        vcpus: 2,
        disk_path: "/var/lib/gpu-share/images/test-vm-1.qcow2".into(),
        disk_size_gb: 20,
    };

    // VM creation
    let vm = libvirt.create_vm(&config).await?;
    assert!(vm.get_name()?.eq("test-vm-1"));
    
    // VM start
    vm.create()?;
    assert!(vm.is_active()?);
    
    // Memory and CPU control
    let mem_stats = vm.memory_stats(0)?;
    assert!(mem_stats.contains_key("available"));
    assert!(mem_stats.contains_key("unused"));

    let cpu_stats = vm.get_cpu_stats(0)?;
    assert!(!cpu_stats.is_empty());
    vm.destroy()?;
    vm.undefine()?;

    Ok(())
}

#[tokio::test]
async fn test_real_gpu_passthrough() -> Result<(), Box<dyn std::error::Error>> {
    let libvirt = setup_libvirt().await?;
    let mut gpu_manager = GPUManager::new()?;

    // Find available GPUs
    let gpus = gpu_manager.discover_gpus().await?;
    assert!(!gpus.is_empty(), "At least one GPU is required for testing");

    let test_gpu = &gpus[0];
    info!("Testing with GPU: {} (Vendor: {})", test_gpu.id, test_gpu.vendor_id);

    // IOMMU group control
    let iommu_group = gpu_manager.get_iommu_group(&test_gpu.id)?;
    assert!(iommu_group.is_some(), "GPU IOMMU group control failed");

    // VM creation
    let config = VMConfig {
        name: "test-gpu-vm".to_string(),
        memory_kb: 8 * 1024 * 1024, // 8GB for GPU VM
        vcpus: 4,
        disk_path: "/var/lib/gpu-share/images/test-gpu-vm.qcow2".into(),
        disk_size_gb: 40,
    };

    let vm = libvirt.create_vm(&config).await?;

    // GPU attach
    let gpu_config = GPUConfig {
        gpu_id: test_gpu.id.clone(),
        iommu_group: iommu_group.unwrap(),
    };

    gpu_manager.attach_gpu_to_vm(&vm, &gpu_config).await?;

    // VM XML configuration
    let xml = vm.get_xml_desc(0)?;
    assert!(xml.contains("hostdev"), "GPU device XML not found");
    assert!(xml.contains(&test_gpu.pci_address), "GPU PCI address not found in XML");

    //start vm
    vm.create()?;
    assert!(vm.is_active()?);

    // NVIDIA GPU specific control
    if test_gpu.vendor_id == "10de" { // NVIDIA vendor ID
        let nvidia_smi = std::process::Command::new("nvidia-smi")
            .arg("vgpu")
            .arg("-q")
            .output()?;
        
        let output = String::from_utf8_lossy(&nvidia_smi.stdout);
        assert!(output.contains(&vm.get_name()?), "VM'de GPU görünmüyor");
    }
    vm.destroy()?;
    vm.undefine()?;

    Ok(())
}

#[tokio::test]
async fn test_real_metrics_collection() -> Result<(), Box<dyn std::error::Error>> {
    let libvirt = setup_libvirt().await?;
    let mut metrics = MetricsCollector::new(1, 24); // 1 second intervals

    // Test VM creation
    let config = VMConfig {
        name: "test-metrics-vm".to_string(),
        memory_kb: 2 * 1024 * 1024,
        vcpus: 2,
        disk_path: "/var/lib/gpu-share/images/test-metrics.qcow2".into(),
        disk_size_gb: 20,
    };

    let vm = libvirt.create_vm(&config).await?;
    vm.create()?;

    metrics.start_collection(vm.get_uuid_string()?, vm.clone()).await?;
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
    let collected_metrics = metrics.get_metrics(&vm.get_uuid_string()?).await?;
    assert!(!collected_metrics.is_empty(), "Metrics not collected");

    for metric in collected_metrics {
        assert!(metric.cpu_usage_percent >= 0.0);
        assert!(metric.memory_usage_mb > 0);
        if let Some(gpu_metrics) = metric.gpu_metrics {
            assert!(gpu_metrics.utilization_percent >= 0.0);
            assert!(gpu_metrics.memory_used_mb >= 0);
        }
    }

    // cleanup
    vm.destroy()?;
    vm.undefine()?;

    Ok(())
}

// Kernel module and IOMMU tests
#[test]
fn test_system_requirements() -> Result<(), Box<dyn std::error::Error>> {
    // IOMMU control

    let dmesg = std::process::Command::new("dmesg")
        .output()?;
    let dmesg_output = String::from_utf8_lossy(&dmesg.stdout);
    assert!(
        dmesg_output.contains("IOMMU") || dmesg_output.contains("AMD-Vi"),
        "IOMMU is not active"
    );

    // Required kernel modules
    let modules = [
        "vfio",
        "vfio_pci",
        "vfio_iommu_type1",
        "kvm",
        "kvm_intel",  // or kvm_amd
    ];

    for module in modules {
        let lsmod = std::process::Command::new("lsmod")
            .output()?;
        let output = String::from_utf8_lossy(&lsmod.stdout);
        assert!(
            output.contains(module),
                "Kernel module {} not loaded", 
            module
        );
    }

    Ok(())
}