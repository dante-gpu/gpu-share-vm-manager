/*
* Hey fellow @virjilakrum from DanteGPU! 👋
* 
* Welcome to our metrics collection wonderland - where we track resources like 
* Elon's Neuralink tracks your thoughts (just kidding, we're more reliable!)
*
* This module is the heart of our VM resource monitoring system. Here's what's cooking:
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
use tracing::{info, error, warn};

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
    vm_metrics: HashMap<String, Vec<ResourceMetrics>>,
    collection_interval: Duration,
    history_retention_hours: u64,
}

impl MetricsCollector {
    pub fn new(collection_interval_secs: u64, history_retention_hours: u64) -> Self {
        info!("Initializing Metrics Collector with {}s interval", collection_interval_secs);
        Self {
            vm_metrics: HashMap::new(),
            collection_interval: Duration::from_secs(collection_interval_secs),
            history_retention_hours,
        }
    }

    pub async fn start_collection(&mut self, vm_id: String, domain: virt::domain::Domain) -> Result<()> {
        info!("Starting metrics collection for VM: {}", vm_id);
        
        let interval = self.collection_interval;
        let retention_hours = self.history_retention_hours;
        let metrics_store = self.vm_metrics.clone();

        tokio::spawn(async move {
            let mut interval_timer = time::interval(interval);
            loop {
                interval_timer.tick().await;
                
                match Self::collect_vm_metrics(&domain).await {
                    Ok(metrics) => {
                        if let Some(metrics_vec) = metrics_store.get_mut(&vm_id) {
                            metrics_vec.push(metrics);
                            Self::cleanup_old_metrics(metrics_vec, retention_hours);
                        }
                    }
                    Err(e) => {
                        error!("Failed to collect metrics for VM {}: {}", vm_id, e);
                    }
                }
            }
        });

        Ok(())
    }

    async fn collect_vm_metrics(domain: &virt::domain::Domain) -> Result<ResourceMetrics> {
        let mem_stats = domain.memory_stats()?;
        let cpu_stats = domain.get_cpu_stats(true)?;

        // Calculate CPU usage
        let cpu_time = cpu_stats.iter()
            .map(|stats| stats.cpu_time)
            .sum::<u64>();
        let cpu_usage = Self::calculate_cpu_usage(cpu_time);

        // Get memory usage
        let memory_used = mem_stats.get("actual").unwrap_or(&0) / 1024; // Convert to MB
        let memory_total = mem_stats.get("available").unwrap_or(&0) / 1024;

        // Collect GPU metrics if available
        let gpu_metrics = Self::collect_gpu_metrics(domain).await?;

        Ok(ResourceMetrics {
            timestamp: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)?
                .as_secs(),
            cpu_usage_percent: cpu_usage,
            memory_usage_mb: memory_used,
            memory_total_mb: memory_total,
            gpu_metrics,
        })
    }

    async fn collect_gpu_metrics(domain: &virt::domain::Domain) -> Result<Option<GPUMetrics>> {
        // Check if VM has GPU attached
        let xml = domain.get_xml_desc(0)?;
        if !xml.contains("<hostdev") {
            return Ok(None);
        }

        // Try NVIDIA GPU metrics first
        if let Ok(metrics) = Self::collect_nvidia_metrics().await {
            return Ok(Some(metrics));
        }

        // Fallback to AMD GPU metrics
        if let Ok(metrics) = Self::collect_amd_metrics().await {
            return Ok(Some(metrics));
        }

        warn!("No GPU metrics available for domain {}", domain.get_name()?);
        Ok(None)
    }

    async fn collect_nvidia_metrics() -> Result<GPUMetrics> {
        let output = tokio::process::Command::new("nvidia-smi")
            .args(&[
                "--query-gpu=utilization.gpu,memory.used,memory.total,temperature.gpu,power.draw",
                "--format=csv,noheader,nounits"
            ])
            .output()
            .await?;

        if !output.status.success() {
            return Err(anyhow::anyhow!("nvidia-smi command failed"));
        }

        let output_str = String::from_utf8(output.stdout)?;
        let values: Vec<&str> = output_str.trim().split(',').collect();

        if values.len() != 5 {
            return Err(anyhow::anyhow!("Unexpected nvidia-smi output format"));
        }

        Ok(GPUMetrics {
            utilization_percent: values[0].trim().parse()?,
            memory_used_mb: values[1].trim().parse()?,
            memory_total_mb: values[2].trim().parse()?,
            temperature_celsius: values[3].trim().parse()?,
            power_usage_watts: values[4].trim().parse()?,
        })
    }

    async fn collect_amd_metrics() -> Result<GPUMetrics> {
        // Read metrics from sysfs for AMD GPUs
        let utilization = tokio::fs::read_to_string("/sys/class/drm/card0/device/gpu_busy_percent")
            .await?
            .trim()
            .parse()?;

        let memory_total = tokio::fs::read_to_string("/sys/class/drm/card0/device/mem_info_vram_total")
            .await?
            .trim()
            .parse::<u64>()? / (1024 * 1024);

        let memory_used = tokio::fs::read_to_string("/sys/class/drm/card0/device/mem_info_vram_used")
            .await?
            .trim()
            .parse::<u64>()? / (1024 * 1024);

        let temperature = tokio::fs::read_to_string("/sys/class/drm/card0/device/hwmon/hwmon0/temp1_input")
            .await?
            .trim()
            .parse::<i32>()? / 1000;

        Ok(GPUMetrics {
            utilization_percent: utilization,
            memory_used_mb: memory_used,
            memory_total_mb: memory_total,
            temperature_celsius: temperature,
            power_usage_watts: 0.0, // AMD doesn't expose power usage in sysfs
        })
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
}