/*
* DanteGPU Configuration Management System
* ---------------------------------------
* @author: virjilakrum
* @project: gpu-share-vm-manager
* @status: works-on-my-machine-certified 
* 
* Welcome to the configuration wonderland! This is where we define how our 
* application should behave (or misbehave, depending on ur config values lol).
* 
* Technical Architecture:
* --------------------
* We're implementing a hierarchical configuration system with multiple layers
* of overrides (because one source of truth is too mainstream).
*
* Configuration Hierarchy (from lowest to highest priority):
* -----------------------------------------------------
* 1. Hardcoded defaults (for when everything else fails spectacularly)
* 2. default.toml (base configuration, like ur morning coffee - essential)
* 3. local.toml (environment-specific, like ur secret energy drink stash)
* 4. Environment variables (for DevOps people who love SCREAMING_SNAKE_CASE)
*
* Core Components:
* --------------
* 1. ServerSettings:
*    - host: Where we serve our API (localhost, because security first!)
*    - port: The magical number for network communication
*    - api_prefix: Because we might change our minds about /api/v1 later
*
* 2. LibvirtSettings:
*    - connection_uri: The mystical URI that connects us to the VM realm
*    - max_vms: Upper limit before ur CPU starts crying
*    - default_memory_mb: RAM allocation (chrome.exe has entered the chat)
*    - default_vcpus: Virtual CPUs (n+1 where n = ur actual core count)
*
* 3. MonitoringSettings:
*    - metrics_interval_seconds: How often we check if everything's on fire
*    - retention_hours: How long we keep the evidence
*    - enable_gpu_metrics: For when you want to know why ur GPU fans sound like a jet engine
*
* 4. StorageSettings:
*    - vm_image_path: Where VM images go to hibernate
*    - max_storage_gb: Because someone will try to store their entire Steam library
*
* 5. RateLimitSettings:
*    - api_requests_per_minute: Rate limit for general API requests
*    - gpu_requests_per_minute: Rate limit for GPU-related requests
*    - auth_requests_per_minute: Rate limit for authentication-related requests
*
* Implementation Details:
* --------------------
* - Using serde for serialization (because writing parsers is so 1990s)
* - Config builder pattern (more elegant than ur SQL queries)
* - Environment variable support (btw, did you commit that .env file to git?)
* - PathBuf for paths (because String is too mainstream for filesystem ops)
* - Proper error handling (more robust than ur weekend project)
*
* Error Handling Strategy:
* ---------------------
* - ConfigError propagation (errors go brrr...)
* - Graceful fallbacks to defaults (plan B, because plan A never works)
* - Comprehensive error messages (more descriptive than ur commit messages)
*
* Usage Example:
* ------------
* ```rust
* let config = Settings::new().expect("Config machine broke");
* // If this fails, try turning it off and on again
* ```
*
* Pro Tips:
* --------
* 1. Always check your config values (trust no one, especially yourself)
* 2. Keep sensitive data in environment variables (your API keys want privacy too)
* 3. Use reasonable defaults (because users never read documentation anyway)
*
* Remember: Configuration is like a box of chocolates - you never know what 
* environment variables you're gonna get.
*/

use serde::{Deserialize, Serialize};
use config::{Config, ConfigError, File};
use std::path::PathBuf;
use tracing::info;

#[derive(Debug, Serialize, Deserialize)]
pub struct Settings {
    pub server: ServerSettings,
    pub libvirt: LibvirtSettings,
    pub monitoring: MonitoringSettings,
    pub storage: StorageSettings,
    pub rate_limits: RateLimitSettings,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ServerSettings {
    pub host: String,
    pub port: u16,
    pub api_prefix: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LibvirtSettings {
    pub connection_uri: String,
    pub max_vms: u32,
    pub default_memory_mb: u64,
    pub default_vcpus: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MonitoringSettings {
    pub metrics_interval_seconds: u64,
    pub retention_hours: u64,
    pub enable_gpu_metrics: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StorageSettings {
    pub vm_image_path: PathBuf,
    pub max_storage_gb: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RateLimitSettings {
    pub api_requests_per_minute: u32,
    pub gpu_requests_per_minute: u32,
    pub auth_requests_per_minute: u32,
}

impl Default for RateLimitSettings {
    fn default() -> Self {
        Self {
            api_requests_per_minute: 100,
            gpu_requests_per_minute: 30,
            auth_requests_per_minute: 10,
        }
    }
}

impl Settings {
    pub fn new() -> Result<Self, ConfigError> {
        let config_path = std::env::var("CONFIG_PATH")
            .unwrap_or_else(|_| "config".to_string());

        info!("Loading configuration from path: {}", config_path);

        let config = Config::builder()
            // Start with default settings
            .set_default("server.host", "127.0.0.1")?
            .set_default("server.port", 3000)?
            .set_default("server.api_prefix", "/api/v1")?
            
            // Add configuration from files
            .add_source(File::with_name(&format!("{}/default", config_path)))
            .add_source(File::with_name(&format!("{}/local", config_path)).required(false))
            
            // Add environment variables with prefix "APP_"
            .add_source(config::Environment::with_prefix("APP"))
            .build()?;

        config.try_deserialize()
    }
}

pub fn generate_default_config() -> Settings {
    Settings {
        server: ServerSettings {
            host: "127.0.0.1".to_string(),
            port: 3000,
            api_prefix: "/api/v1".to_string(),
        },
        libvirt: LibvirtSettings {
            connection_uri: "qemu:///system".to_string(),
            max_vms: 10,
            default_memory_mb: 4096,
            default_vcpus: 2,
        },
        monitoring: MonitoringSettings {
            metrics_interval_seconds: 5,
            retention_hours: 24,
            enable_gpu_metrics: true,
        },
        storage: StorageSettings {
            vm_image_path: PathBuf::from("/var/lib/gpu-share/images"),
            max_storage_gb: 100,
        },
        rate_limits: RateLimitSettings {
            api_requests_per_minute: 100,
            gpu_requests_per_minute: 30,
            auth_requests_per_minute: 10,
        },
    }
}