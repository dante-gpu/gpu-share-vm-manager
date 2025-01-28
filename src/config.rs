use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub metrics: MetricsConfig,
    pub rate_limits: RateLimitConfig,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MetricsConfig {
    pub collection_interval_secs: u64,
    pub retention_hours: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RateLimitConfig {
    pub api_requests_per_minute: u32,
    pub gpu_requests_per_minute: u32,
    pub auth_requests_per_minute: u32,
} 