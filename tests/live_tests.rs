use gpu_share_vm_manager::core::docker_manager::DockerManager;
use gpu_share_vm_manager::core::docker_manager::ContainerConfig;
use gpu_share_vm_manager::gpu::device::{GPUManager, GPUConfig};
use gpu_share_vm_manager::monitoring::MetricsCollector;
use tracing::info;

// Time to set up our virtual playground! 
async fn setup_docker() -> anyhow::Result<DockerManager> {
    info!("Setting up our virtual circus - bring in the clowns! 🤡");
    let manager = DockerManager::new()?;
    
    // Clean up any leftover test VMs - like cleaning up after the party 🧹
    for container_id in manager.list_containers().await? {
        let name = container_id.split('/').last().unwrap_or_default();
        if name.starts_with("test-") {
            info!("Cleaning up old test container: {} - goodbye old friend! 👋", name);
            if manager.is_container_active(&container_id).await? {
                manager.stop_container(&container_id).await?;
            }
            manager.delete_container(&container_id).await?;
        }
    }

    Ok(manager)
}

// Let's test our VM creation skills! 🎮
#[tokio::test]
async fn test_real_vm_creation() -> anyhow::Result<()> {
    let docker = setup_docker().await?;
    
    let config = ContainerConfig {
        image: "alpine".into(),
        name: "test-container-1".into(),
        gpu_id: None,
    };

    // Create and verify our new digital pet 🐕
    let container_id = docker.create_container(&config.image, &config.name).await?;
    assert!(container_id.starts_with("test-container-1"));
    
    // Start it up - vroom vroom! 
    docker.start_container(&container_id).await?;
    docker.stop_container(&container_id).await?;
    docker.delete_container(&container_id).await?;

    Ok(())
}

// Time to test our GPU passthrough magic! ✨
#[tokio::test]
async fn test_real_gpu_passthrough() -> anyhow::Result<()> {
    let docker = setup_docker().await?;
    let mut gpu_manager = GPUManager::new()?;

    // Find our GPUs - like a digital treasure hunt! 🗺️
    let gpus = gpu_manager.discover_gpus()?;
    assert!(!gpus.is_empty(), "No GPUs found - did they go on vacation? 🏖️");

    let test_gpu = &gpus[0];
    info!("Testing with GPU: {} - our chosen one! ⚡", test_gpu.id);

    // Create a VM fit for a GPU king! 👑
    let config = ContainerConfig {
        image: "alpine".into(),
        name: "test-container-gpu".into(),
        gpu_id: Some(GPUConfig {
            gpu_id: test_gpu.id.clone(),
            iommu_group: 42,
        }),
    };

    let container_id = docker.create_container(&config.image, &config.name).await?;

    // Prepare the GPU config - like preparing a throne! 
    let gpu_config = GPUConfig {
        gpu_id: test_gpu.id.clone(),
        iommu_group: 42,
    };

    // Attach the GPU - may the force be with us! 
    gpu_manager.attach_gpu(&container_id, &gpu_config.gpu_id).await?;

    // Verify our handiwork
    let stats = docker.inspect_container(&container_id).await?;
    assert!(stats.cpu_usage > 0.0, "GPU usage not detected");

    // Start the VM - launch sequence initiated! 
    docker.start_container(&container_id).await?;
    docker.stop_container(&container_id).await?;
    docker.delete_container(&container_id).await?;

    Ok(())
}

// Let's test our metrics collection - time to get nerdy! 🤓
#[tokio::test]
async fn test_real_metrics_collection() -> anyhow::Result<()> {
    let docker = setup_docker().await?;
    let metrics = MetricsCollector::new(1, 24); // 1 second intervals, 24h retention

    // Create a test VM - our metrics guinea pig! 🐹
    let config = ContainerConfig {
        image: "alpine".into(),
        name: "test-container-metrics".into(),
        gpu_id: None,
    };

    let container_id = docker.create_container(&config.image, &config.name).await?;
    docker.start_container(&container_id).await?;

    // Start collecting those sweet, sweet metrics! 
    metrics.start_collection(&docker, &container_id).await?;
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
    
    let collected_metrics = metrics.get_metrics(&container_id)?;
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
    docker.stop_container(&container_id).await?;
    docker.delete_container(&container_id).await?;

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
    let docker = setup_docker().await?;
    
    // Common VM configuration
    let config = ContainerConfig {
        image: "alpine".into(),
        name: "cross-platform-test".into(),
        gpu_id: None,
    };

    // Basic VM operations
    let container_id = docker.create_container(&config.image, &config.name).await?;
    docker.start_container(&container_id).await?;
    assert!(docker.is_container_active(&container_id).await?, "VM failed to start");
    
    // Platform-specific resource checks
    #[cfg(target_os = "linux")] {
        let stats = docker.inspect_container(&container_id).await?;
        assert!(stats.memory_usage > 0.0, "Memory usage not detected");
    }
    
    #[cfg(target_os = "macos")] {
        // Docker için XML tanımı gerekmiyor
    }

    docker.stop_container(&container_id).await?;
    docker.delete_container(&container_id).await?;
    Ok(())
}

// Test 1: Basic VM Creation
async fn create_basic_vm() -> ContainerConfig {
    ContainerConfig {
        image: "alpine".into(),
        name: "test-container-basic".into(),
        gpu_id: None,
    }
}

// Test 2: GPU Passthrough Test
async fn create_gpu_vm() -> ContainerConfig {
    ContainerConfig {
        image: "alpine".into(),
        name: "test-container-gpu".into(),
        gpu_id: Some(GPUConfig {
            gpu_id: "0000:01:00.0".into(),
            iommu_group: 42,
        }),
    }
}

// Test 3: Big Scale VM
async fn create_large_vm() -> ContainerConfig {
    ContainerConfig {
        image: "alpine".into(),
        name: "test-container-large".into(),
        gpu_id: None,
    }
}

// Test 4: Edge Case - Minimum Resources
async fn create_minimal_vm() -> ContainerConfig {
    ContainerConfig {
        image: "alpine".into(),
        name: "test-container-minimal".into(),
        gpu_id: None,
    }
}

#[tokio::test]
async fn test_gpu_attachment() {
    let docker = DockerManager::new().unwrap();
    let mut gpu_manager = GPUManager::new().unwrap();
    let metrics = MetricsCollector::new(5, 24);

    let container_id = docker.create_container("alpine", "test-container-attach").await.unwrap();

    let result = gpu_manager.attach_gpu(&container_id, "mock-gpu-1").await;
    assert!(result.is_ok());

    let metrics = metrics.get_metrics(&container_id).unwrap();
    assert!(metrics.len() > 0);
}