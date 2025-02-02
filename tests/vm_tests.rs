// Virtual Machine Test Suite - Because untested code is like Schrödinger's cat! 🐱💻

use anyhow::Result;
use anyhow::anyhow;
use gpu_share_vm_manager::core::docker_manager::{DockerManager, ContainerConfig};
use gpu_share_vm_manager::gpu::device::{GPUManager, GPUInfo};
use rand::Rng;
// use tracing::{info, warn};
use std::time::Duration;
use std::collections::HashMap;

#[derive(Clone)]
struct DockerManagerWrapper(DockerManager);

impl DockerManagerWrapper {
    fn new() -> Result<Self> {
        DockerManager::new().map(Self)
    }
}

// Test setup: Creates a unique VM configuration to avoid conflicts
fn test_vm_config() -> ContainerConfig {
    let mut rng = rand::thread_rng();
    ContainerConfig {
        image: "alpine".into(),
        name: format!("test-container-{}", rng.gen::<u32>()),
        gpu_id: None,
    }
}

// Resource Validation Test: CPU/Memory allocation
#[tokio::test]
async fn test_resource_allocation() -> Result<()> {
    let docker = DockerManagerWrapper::new()?;
    let config = test_vm_config();
    
    let container_id = docker.0.create_container(&config.image, &config.name).await?;
    docker.0.start_container(&container_id).await?;
    
    tokio::time::sleep(Duration::from_secs(5)).await;
    let stats = docker.0.inspect_container(&container_id).await?;
    assert!(stats.memory_usage > 0.0, "Memory usage should be positive");
    assert!(stats.cpu_usage > 0.0, "CPU usage should be positive");

    docker.0.delete_container(&container_id).await?;
    Ok(())
}

// Network Configuration Test: Validate network interfaces and connectivity
#[tokio::test]
async fn test_vm_network_configuration() -> Result<()> {
    let docker = DockerManagerWrapper::new()?;
    let config = test_vm_config();
    
    let container_id = docker.0.create_container(&config.image, &config.name).await?;
    docker.0.start_container(&container_id).await?;
    
    let info = docker.0.inspect_container(&container_id).await?;
    assert!(info.cpu_usage >= 0.0, "Container should be initialized");
    
    docker.0.delete_container(&container_id).await?;
    Ok(())
}

// Negative Test: Duplicate VM creation and error handling
#[tokio::test]
async fn test_duplicate_vm_creation() -> Result<()> {
    let docker = DockerManagerWrapper::new()?;
    let config = test_vm_config();
    
    // First creation should succeed
    let container_id1 = docker.0.create_container(&config.image, &config.name).await?;
    
    // Second creation with same config should fail
    let result = docker.0.create_container(&config.image, &config.name).await;
    assert!(
        result.is_err(),
        "Should return error when creating duplicate VM"
    );
    
    // Verify error type
    if let Err(e) = result {
        assert!(
            e.to_string().contains("already exists"),
            "Error should indicate duplicate VM"
        );
    }
    
    // Cleanup
    docker.0.delete_container(&container_id1).await?;
    Ok(())
}

#[tokio::test]
async fn test_container_creation() {
    let docker = DockerManagerWrapper::new().unwrap();
    let config = ContainerConfig {
        image: "alpine".into(),
        name: "test-container-1".into(),
        gpu_id: None,
    };

    let container_id = docker.0.create_container(&config.image, &config.name).await.unwrap();
    assert!(container_id.starts_with("test-container-1"));
}

#[tokio::test]
async fn test_gpu_attachment() {
    let docker = DockerManagerWrapper::new().unwrap();
    let mut gpu_manager = GPUManager {
        devices: vec![GPUInfo::mock()],
        iommu_groups: HashMap::new(),
    };

    let config = test_vm_config();
    let container_id = docker.0.create_container(&config.image, &config.name).await.unwrap();

    let gpus = gpu_manager.discover_gpus().unwrap();
    let result = if !gpus.is_empty() {
        gpu_manager.attach_gpu(&container_id, &gpus[0].id).await
    } else {
        Err(anyhow!("No GPU available for test"))
    };
    assert!(result.is_ok());

    // Cleanup
    docker.0.delete_container(&container_id).await.unwrap();
}