use gpu_share_vm_manager::core::LibvirtManager;
use gpu_share_vm_manager::core::vm::VMConfig;
use gpu_share_vm_manager::gpu::device::{GPUManager, GPUConfig};
use gpu_share_vm_manager::monitoring::MetricsCollector;
use std::path::PathBuf;
use tracing::info;

// Time to set up our virtual playground! 
async fn setup_libvirt() -> anyhow::Result<LibvirtManager> {
    info!("Setting up our virtual circus - bring in the clowns! 🤡");
    let manager = LibvirtManager::new()?;
    
    // Clean up any leftover test VMs - like cleaning up after the party 🧹
    for domain in manager.list_domains()? {
        let name = domain.get_name()?;
        if name.starts_with("test-") {
            info!("Cleaning up old test domain: {} - goodbye old friend! 👋", name);
            if domain.is_active()? {
                domain.destroy()?;
            }
            domain.undefine()?;
        }
    }

    Ok(manager)
}

// Let's test our VM creation skills! 🎮
#[tokio::test]
async fn test_real_vm_creation() -> anyhow::Result<()> {
    let libvirt = setup_libvirt().await?;
    
    let config = VMConfig {
        name: "test-vm-1".to_string(),
        memory_kb: 4 * 1024 * 1024, // 4GB - because size matters! 
        vcpus: 2,                   // Dual-core power! ⚡
        disk_path: PathBuf::from("/var/lib/gpu-share/images/test-vm-1.qcow2"),
        disk_size_gb: 20,           // Room for activities! 
    };

    // Create and verify our new digital pet 🐕
    let vm = libvirt.create_vm(&config).await?;
    assert!(vm.get_name()?.eq("test-vm-1"));
    
    // Start it up - vroom vroom! 
    vm.create()?;
    assert!(vm.is_active()?);
    
    // Check its vital signs 🏥
    let mem_stats = vm.memory_stats(0)?;
    assert!(mem_stats.iter().any(|stat| stat.tag == 4)); // available
    assert!(mem_stats.iter().any(|stat| stat.tag == 6)); // unused

    // Clean up after ourselves - we're responsible VM parents! 👨‍👦
    vm.destroy()?;
    vm.undefine()?;

    Ok(())
}

// Time to test our GPU passthrough magic! ✨
#[tokio::test]
async fn test_real_gpu_passthrough() -> anyhow::Result<()> {
    let libvirt = setup_libvirt().await?;
    let mut gpu_manager = GPUManager::new()?;

    // Find our GPUs - like a digital treasure hunt! 🗺️
    let gpus = gpu_manager.discover_gpus()?;
    assert!(!gpus.is_empty(), "No GPUs found - did they go on vacation? 🏖️");

    let test_gpu = &gpus[0];
    info!("Testing with GPU: {} - our chosen one! ⚡", test_gpu.id);

    // Create a VM fit for a GPU king! 👑
    let config = VMConfig {
        name: "test-gpu-vm".to_string(),
        memory_kb: 8 * 1024 * 1024, // 8GB - because GPUs are memory hungry! 
        vcpus: 4,                   // Quad-core power for our GPU overlord! 
        disk_path: PathBuf::from("/var/lib/gpu-share/images/test-gpu-vm.qcow2"),
        disk_size_gb: 40,           // Extra space for those GPU drivers! 📦
    };

    let vm = libvirt.create_vm(&config).await?;

    // Prepare the GPU config - like preparing a throne! 
    let gpu_config = GPUConfig {
        gpu_id: test_gpu.id.clone(),
        iommu_group: "0".to_string(), // Default group for testing
    };

    // Attach the GPU - may the force be with us! 
    gpu_manager.attach_gpu_to_vm(&vm, &gpu_config).await?;

    // Verify our handiwork
    let xml = vm.get_xml_desc(0)?;
    assert!(xml.contains("hostdev"), "GPU XML not found - did it go stealth? 🥷");

    // Start the VM - launch sequence initiated! 
    vm.create()?;
    assert!(vm.is_active()?);

    // Clean up our mess - leave no trace! 
    vm.destroy()?;
    vm.undefine()?;

    Ok(())
}

// Let's test our metrics collection - time to get nerdy! 🤓
#[tokio::test]
async fn test_real_metrics_collection() -> anyhow::Result<()> {
    let libvirt = setup_libvirt().await?;
    let mut metrics = MetricsCollector::new(1, 24); // 1 second intervals, 24h retention

    // Create a test VM - our metrics guinea pig! 🐹
    let config = VMConfig {
        name: "test-metrics-vm".to_string(),
        memory_kb: 2 * 1024 * 1024,
        vcpus: 2,
        disk_path: PathBuf::from("/var/lib/gpu-share/images/test-metrics.qcow2"),
        disk_size_gb: 20,
    };

    let vm = libvirt.create_vm(&config).await?;
    vm.create()?;

    // Start collecting those sweet, sweet metrics! 
    metrics.start_collection(vm.get_uuid_string()?, vm.clone()).await?;
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
    
    let collected_metrics = metrics.get_vm_metrics(&vm.get_uuid_string()?)?;
    assert!(!collected_metrics.is_empty(), "No metrics collected - did our sensors fall asleep? 😴");

    // Verify our metrics - time for some number crunching! 
    for metric in collected_metrics {
        assert!(metric.cpu_usage_percent >= 0.0, "Negative CPU usage? What sorcery is this! 🧙‍♂️");
        assert!(metric.memory_usage_mb > 0, "Zero memory usage? Is this VM on a diet? 🥗");
        if let Some(gpu_metrics) = metric.gpu_metrics {
            assert!(gpu_metrics.utilization_percent >= 0.0, "GPU going backwards? That's new! 🔄");
            assert!(gpu_metrics.memory_used_mb > 0, "Zero GPU memory usage? Is this VM on a diet? 🥗");
        }
    }

    // Clean up - time to put our toys away! 
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

#[cfg(target_os = "macos")]
#[test]
fn test_macos_system_requirements() -> Result<(), Box<dyn std::error::Error>> {
    // Check if Hypervisor framework is available
    let hypervisor = std::process::Command::new("sysctl")
        .args(&["-n", "kern.hv_support"])
        .output()?;
    let hypervisor_output = String::from_utf8_lossy(&hypervisor.stdout);
    assert!(
        hypervisor_output.trim() == "1",
        "Hypervisor framework is not available on this Mac"
    );

    // Verify QEMU installation
    let qemu = std::process::Command::new("which")
        .arg("qemu-system-x86_64")
        .output()?;
    assert!(
        qemu.status.success(),
        "QEMU is not installed. Please install via Homebrew: brew install qemu"
    );

    // Check virtualization extensions
    let sysctl = std::process::Command::new("sysctl")
        .args(&["-n", "machdep.cpu.features"])
        .output()?;
    let cpu_features = String::from_utf8_lossy(&sysctl.stdout);
    assert!(
        cpu_features.contains("VMX"),
        "Intel VT-x/AMD-V virtualization extensions are not available"
    );

    Ok(())
}