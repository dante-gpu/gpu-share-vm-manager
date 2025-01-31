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
        gpu_passthrough: None,
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
        gpu_passthrough: Some(GPUConfig {
            gpu_id: test_gpu.id.clone(),
            iommu_group: "0".to_string(), // Default group for testing
        }),
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
        gpu_passthrough: None,
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

// Platform-specific system requirement checks
#[test]
fn test_system_requirements() -> Result<(), Box<dyn std::error::Error>> {
    // Common checks for all platforms
    #[cfg(target_os = "linux")] {
        // Check IOMMU support through kernel messages
        let dmesg = std::process::Command::new("dmesg").output()?;
        let dmesg_output = String::from_utf8_lossy(&dmesg.stdout);
        assert!(
            dmesg_output.contains("IOMMU") || dmesg_output.contains("AMD-Vi"),
            "IOMMU not enabled in kernel parameters"
        );

        // Verify required kernel modules using /proc/modules
        let modules_file = std::fs::read_to_string("/proc/modules")?;
        let required_modules = ["vfio", "vfio_pci", "vfio_iommu_type1", "kvm"];
        for module in required_modules {
            assert!(
                modules_file.contains(module),
                "Required kernel module {} not loaded",
                module
            );
        }
    }

    #[cfg(target_os = "macos")] {
        // Verify macOS hypervisor capabilities
        let hypervisor = std::process::Command::new("sysctl")
            .args(["-n", "kern.hv_support"])
            .output()?;
        assert!(
            String::from_utf8_lossy(&hypervisor.stdout).trim() == "1",
            "Hypervisor framework not available"
        );

        // Check QEMU installation
        let qemu_check = std::process::Command::new("which")
            .arg("qemu-system-x86_64")
            .status()?;
        assert!(
            qemu_check.success(),
            "QEMU not found in PATH, install via 'brew install qemu'"
        );
    }

    #[cfg(target_os = "windows")] {
        // Verify Hyper-V capabilities
        let hyperv = std::process::Command::new("powershell")
            .args(["-Command", "Get-WindowsOptionalFeature -Online -FeatureName Microsoft-Hyper-V"])
            .output()?;
        let output = String::from_utf8_lossy(&hyperv.stdout);
        assert!(
            output.contains("Enabled"),
            "Hyper-V not enabled on Windows system"
        );
    }

    Ok(())
}

// Cross-platform virtualization extension check
#[test]
fn test_virtualization_support() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_os = "linux")] {
        let cpuinfo = std::fs::read_to_string("/proc/cpuinfo")?;
        assert!(
            cpuinfo.contains("vmx") || cpuinfo.contains("svm"),
            "Hardware virtualization extensions not detected"
        );
    }

    #[cfg(target_os = "macos")] {
        let sysctl = std::process::Command::new("sysctl")
            .args(["-n", "machdep.cpu.features"])
            .output()?;
        let features = String::from_utf8_lossy(&sysctl.stdout);
        assert!(
            features.contains("VMX"),
            "Intel VT-x virtualization extensions not available"
        );
    }

    #[cfg(target_os = "windows")] {
        let systeminfo = std::process::Command::new("systeminfo")
            .output()?;
        let info = String::from_utf8_lossy(&systeminfo.stdout);
        assert!(
            info.contains("Virtualization Enabled In Firmware: Yes"),
            "Virtualization not enabled in BIOS/UEFI"
        );
    }

    Ok(())
}

// Platform-agnostic VM lifecycle test
#[tokio::test]
async fn test_cross_platform_vm_operations() -> anyhow::Result<()> {
    let libvirt = setup_libvirt().await?;
    
    // Common VM configuration
    let config = VMConfig {
        name: "cross-platform-test".to_string(),
        memory_kb: 2 * 1024 * 1024,
        vcpus: 2,
        disk_path: PathBuf::from("/var/lib/gpu-share/images/cross-platform-test.qcow2"),
        disk_size_gb: 20,
        gpu_passthrough: None,
    };

    // Basic VM operations
    let vm = libvirt.create_vm(&config).await?;
    vm.create()?;
    assert!(vm.is_active()?, "VM failed to start");
    
    // Platform-specific resource checks
    #[cfg(target_os = "linux")] {
        let mem_stats = vm.memory_stats(0)?;
        assert!(mem_stats.iter().any(|s| s.tag == 4), "Memory stats incomplete");
    }
    
    #[cfg(target_os = "macos")] {
        let xml = vm.get_xml_desc(0)?;
        assert!(xml.contains("qemu:commandline"), "QEMU specific configuration missing");
    }

    vm.destroy()?;
    vm.undefine()?;
    Ok(())
}

// Test 1: Basic VM Creation
async fn create_basic_vm() -> VMConfig {
    VMConfig {
        name: "test-vm-basic".into(),
        memory_kb: 2048 * 1024,
        vcpus: 2,
        disk_path: PathBuf::from("/var/lib/libvirt/images/test-vm-basic.qcow2"),
        disk_size_gb: 20,
        gpu_passthrough: None,
    }
}

// Test 2: GPU Passthrough Test
async fn create_gpu_vm() -> VMConfig {
    VMConfig {
        name: "test-vm-gpu".into(),
        memory_kb: 4096 * 1024,
        vcpus: 4,
        disk_path: PathBuf::from("/var/lib/libvirt/images/test-vm-gpu.qcow2"),
        disk_size_gb: 40,
        gpu_passthrough: Some("0000:01:00.0".into()),
    }
}

// Test 3: Big Scale VM
async fn create_large_vm() -> VMConfig {
    VMConfig {
        name: "test-vm-large".into(),
        memory_kb: 16384 * 1024,
        vcpus: 8,
        disk_path: PathBuf::from("/var/lib/libvirt/images/test-vm-large.qcow2"),
        disk_size_gb: 100,
        gpu_passthrough: None,
    }
}

// Test 4: Edge Case - Minimum Resources
async fn create_minimal_vm() -> VMConfig {
    VMConfig {
        name: "test-vm-minimal".into(),
        memory_kb: 512 * 1024,
        vcpus: 1,
        disk_path: PathBuf::from("/var/lib/libvirt/images/test-vm-minimal.qcow2"),
        disk_size_gb: 10,
        gpu_passthrough: None,
    }
}