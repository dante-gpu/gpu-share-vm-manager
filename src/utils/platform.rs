use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum Platform {
    Linux,
    MacOS,
    Windows,
    Unknown,
} 