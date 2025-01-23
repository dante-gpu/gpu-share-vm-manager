use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub metrics: MetricsConfig,
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