use anyhow::{anyhow, Result};
use bollard::Docker;
use bollard::container::{Config, CreateContainerOptions, StartContainerOptions, Stats};
use futures_util::StreamExt;
use tracing::info;
use serde::Serialize;
use crate::gpu::device::GPUConfig;

#[derive(Clone)]
pub struct DockerManager {
    docker: Docker,
}

impl DockerManager {
    pub fn new() -> Result<Self> {
        let docker = Docker::connect_with_local_defaults()?;
        Ok(Self { docker })
    }

    pub async fn create_container(&self, image: &str, name: &str) -> Result<String> {
        info!("🐳 Creating container: {} with image {}", name, image);
        
        let config = Config {
            image: Some(image),
            ..Default::default()
        };

        let options = CreateContainerOptions {
            name: name.to_string(),
            platform: None,
        };

        let container = self.docker.create_container(Some(options), config).await?;
        self.docker.start_container(&container.id, None::<StartContainerOptions<String>>).await?;
        
        Ok(container.id)
    }

    pub async fn list_containers(&self) -> Result<Vec<String>> {
        let containers = self.docker.list_containers::<String>(None).await?;
        Ok(containers.iter()
            .filter_map(|c| c.names.as_ref().and_then(|n| n.first().cloned()))
            .collect())
    }

    pub async fn lookup_container(&self, id: &str) -> Result<String> {
        let container = self.docker.inspect_container(id, None).await?;
        Ok(container.id.ok_or_else(|| anyhow!("Container ID not found for: {}", id))?)
    }
    
    pub async fn start_container(&self, id: &str) -> Result<()> {
        self.docker.start_container(id, None::<StartContainerOptions<String>>).await?;
        Ok(())
    }
    
    pub async fn stop_container(&self, id: &str) -> Result<()> {
        self.docker.stop_container(id, None).await?;
        Ok(())
    }
    
    pub async fn delete_container(&self, id: &str) -> Result<()> {
        self.docker.remove_container(id, None).await?;
        Ok(())
    }

    pub async fn inspect_container(&self, container_id: &str) -> Result<ContainerStats> {
        let mut stats_stream = self.docker.stats(container_id, None);
        let stats = stats_stream.next().await.ok_or(anyhow!("No stats available"))??;
        
        let cpu_percent = calculate_cpu_percent(&stats);
        let memory_usage = stats.memory_stats.usage
            .unwrap_or(0) as f64 / 1024.0 / 1024.0;

        Ok(ContainerStats {
            cpu_usage: cpu_percent,
            memory_usage,
        })
    }

    pub async fn is_container_active(&self, container_id: &str) -> Result<bool> {
        let container = self.docker.inspect_container(container_id, None).await?;
        Ok(container.state.and_then(|s| s.running).unwrap_or(false))
    }
}

fn calculate_cpu_percent(stats: &Stats) -> f64 {
    let cpu_delta = stats.cpu_stats.cpu_usage.total_usage
        .saturating_sub(stats.precpu_stats.cpu_usage.total_usage);

    let system_delta = match (stats.cpu_stats.system_cpu_usage, stats.precpu_stats.system_cpu_usage) {
        (Some(current), Some(previous)) => current.saturating_sub(previous),
        _ => 0,
    };
    
    if system_delta > 0 && cpu_delta > 0 {
        (cpu_delta as f64 / system_delta as f64) * 100.0 * 
            stats.cpu_stats.online_cpus.unwrap_or(1) as f64
    } else {
        0.0
    }
}

#[derive(Debug, Serialize)]
pub struct ContainerStats {
    pub cpu_usage: f64,
    pub memory_usage: f64,
}

#[derive(Debug, Clone)]
pub struct ContainerConfig {
    pub image: String,
    pub name: String,
    pub gpu_id: Option<GPUConfig>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn test_container_lifecycle() {
        let manager = DockerManager::new().unwrap();
        let container_name = "integration-test-container";

        // Create
        manager.create_container("alpine", container_name)
            .await
            .unwrap();
        
        // Start
        manager.start_container(container_name).await.unwrap();
        
        // Verify running
        let containers = manager.list_containers().await.unwrap();
        assert!(containers.contains(&container_name.to_string()));

        // Stop
        manager.stop_container(container_name).await.unwrap();
        
        // Delete
        manager.delete_container(container_name).await.unwrap();
        
        // Verify deletion
        sleep(Duration::from_secs(1)).await; // Wait for Docker API sync 
        let containers = manager.list_containers().await.unwrap();
        assert!(!containers.contains(&container_name.to_string()));
    }
} 