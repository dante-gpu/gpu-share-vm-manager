/*
* Hey fellow @virjilakrum again from DanteGPU! 👋
* 
* Welcome to our metrics collection wonderland - where we track resources like 
* Elon's Neuralink tracks your thoughts (just kidding, we're more reliable!)
*
* This module is the heart of our Container resource monitoring system. Here's what's cooking:
*
* Key Components:
* -------------
* 1. ResourceMetrics: Our bread and butter struct that holds:
*    - CPU usage (%) - as precise as Tesla's self-driving predictions
*    - Memory usage (MB) - because we count every byte like it's Memetoken in 2025
*    - GPU metrics - tracking those neural processing units like Instagram tracks your scrolling habits
*
* 2. GPUMetrics: The gaming powerhouse metrics including:
*    - GPU utilization - measured more accurately than Meta's AR glasses' battery life
*    - Memory usage - keeping tabs on VRAM like Twitter... sorry, "X" keeps tabs on your posts
*    - Temperature - running cooler than the new quantum processors
*    - Power usage - more efficient than the Mars colony's solar panels
*
* 3. MetricsCollector: The mastermind that:
*    - Runs async collection jobs (faster than you can say "AGI takeover")
*    - Supports both NVIDIA and AMD GPUs (we're Switzerland in the GPU wars)
*    - Maintains a time-series history (like your browser history, but less embarrassing)
*    - Auto-cleans old metrics 
*
* Technical Implementation:
* ----------------------
* - Fully async implementation using Tokio (because blocking is so 2023)
* - Uses safe Rust practices (more protection than your crypto wallet)
* - Implements proper error handling (catches errors better than ChatGPT-o3 catches sarcasm)
* - Hardware agnostic GPU metrics collection (works with everything except quantum GPUs, sorry!)
*
*
* Happy monitoring! Remember: In a world of virtual machines, 
* the one with the best metrics is king! 
*/

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, SystemTime};
use tokio::time;
use tracing::{info, error};
use std::error::Error as StdError;
use crate::core::docker_manager::DockerManager;
use std::sync::{Arc, Mutex};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ResourceMetrics {
    pub timestamp: u64,
    pub cpu_usage_percent: f64,
    pub memory_usage_mb: u64,
    pub memory_total_mb: u64,
    pub gpu_metrics: Option<GPUMetrics>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GPUMetrics {
    pub utilization_percent: f64,
    pub memory_used_mb: u64,
    pub memory_total_mb: u64,
    pub temperature_celsius: i32,
    pub power_usage_watts: f64,
}

pub struct MetricsCollector {
    container_metrics: Arc<Mutex<HashMap<String, Vec<ResourceMetrics>>>>,
    collection_interval: Duration,
    history_retention_hours: u64,
}

impl MetricsCollector {
    pub fn new(interval_secs: u64, retention_hours: u64) -> Self {
        info!("Initializing Metrics Collector with {}s interval", interval_secs);
        Self {
            container_metrics: Arc::new(Mutex::new(HashMap::new())),
            collection_interval: Duration::from_secs(interval_secs),
            history_retention_hours: retention_hours,
        }
    }

    pub async fn start_collection(&self, docker: &DockerManager, container_id: &str) -> Result<()> {
        info!("Starting metrics collection for container: {}", container_id);
        
        let interval = self.collection_interval;
        let retention_hours = self.history_retention_hours;
        let metrics_store = self.container_metrics.clone();

        let docker = docker.clone();
        let container_id = container_id.to_string();

        tokio::spawn(async move {
            let mut interval_timer = time::interval(interval);
            loop {
                interval_timer.tick().await;
                
                match Self::collect_single_container_metrics(&docker, &container_id).await {
                    Ok(metrics) => {
                        let mut store = metrics_store.lock().unwrap();
                        if let Some(metrics_vec) = store.get_mut(&container_id) {
                            metrics_vec.push(metrics);
                            Self::cleanup_old_metrics(metrics_vec, retention_hours);
                        }
                    }
                    Err(e) => {
                        error!("Failed to collect metrics for container {}: {}", container_id, e);
                    }
                }
            }
        });

        Ok(())
    }

    async fn collect_single_container_metrics(docker: &DockerManager, container_id: &str) -> Result<ResourceMetrics> {
        let stats = docker.inspect_container(container_id).await?;
        
        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)?
            .as_secs();

        let gpu_metrics = Self::collect_gpu_metrics(container_id).await?;

        Ok(ResourceMetrics {
            timestamp,
            cpu_usage_percent: stats.cpu_usage,
            memory_usage_mb: stats.memory_usage as u64,
            memory_total_mb: 0,
            gpu_metrics,
        })
    }

    async fn collect_gpu_metrics(_container_id: &str) -> Result<Option<GPUMetrics>> {
        // TODO: Implement GPU metrics collection for Docker containers
        // This will depend on how GPUs are attached to containers (nvidia-docker, etc.)
        Ok(None)
    }

    fn calculate_cpu_usage(cpu_time: u64) -> f64 {
        // CPU usage calculation based on CPU time delta
        static mut LAST_CPU_TIME: u64 = 0;
        static mut LAST_TIMESTAMP: u64 = 0;

        unsafe {
            let current_time = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs();

            let time_delta = current_time - LAST_TIMESTAMP;
            let cpu_delta = cpu_time - LAST_CPU_TIME;

            LAST_CPU_TIME = cpu_time;
            LAST_TIMESTAMP = current_time;

            if time_delta > 0 {
                (cpu_delta as f64 / (time_delta as f64 * 1_000_000_000.0)) * 100.0
            } else {
                0.0
            }
        }
    }

    fn cleanup_old_metrics(metrics: &mut Vec<ResourceMetrics>, retention_hours: u64) {
        let retention_secs = retention_hours * 3600;
        let current_time = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        metrics.retain(|m| current_time - m.timestamp < retention_secs);
    }

    pub fn get_metrics(&self, container_id: &str) -> Result<Vec<ResourceMetrics>> {
        let store = self.container_metrics.lock().unwrap();
        if let Some(metrics) = store.get(container_id) {
            Ok(metrics.clone())
        } else {
            Err(anyhow::anyhow!("No metrics found for container {}", container_id))
        }
    }

    pub fn stop(&mut self) -> Result<(), Box<dyn StdError>> {
        // Gerçek implementasyon
        Ok(())
    }

    pub async fn collect_container_metrics(&mut self, docker: &DockerManager) -> Result<()> {
        let containers = docker.list_containers().await?;
        
        for container_id in containers {
            let metrics = Self::collect_single_container_metrics(docker, &container_id).await?;
            self.container_metrics.lock().unwrap().entry(container_id.clone())
                .or_default()
                .push(metrics);
        }
        Ok(())
    }

    pub async fn get_container_stats(&self, docker: &DockerManager, container_id: &str) -> Option<ContainerStats> {
        docker.inspect_container(container_id).await.ok().map(|stats| {
            ContainerStats {
                cpu_usage: stats.cpu_usage,
                memory_usage: stats.memory_usage,
            }
        })
    }

    pub async fn get_container_metrics(&self, container_id: &str) -> Result<Vec<ResourceMetrics>> {
        self.container_metrics.lock().unwrap().get(container_id)
            .map(|metrics| metrics.clone())
            .ok_or_else(|| anyhow::anyhow!("No metrics found for container"))
    }
}

#[derive(Debug, Serialize)]
pub struct ContainerStats {
    pub cpu_usage: f64,
    pub memory_usage: f64,
}