use thiserror::Error;

#[derive(Error, Debug)]
pub enum GpuShareError {
    #[error("Libvirt connection error: {0}")]
    ConnectionError(String),
    
    #[error("VM operation failed: {0}")]
    VmOperationError(String),
    
    #[error("Resource allocation error: {0}")]
    ResourceAllocationError(String),
    
    #[error("Configuration error: {0}")]
    ConfigError(String),
    
    #[error("Unknown error: {0}")]
    UnknownError(String),
}

impl From<std::io::Error> for GpuShareError {
    fn from(err: std::io::Error) -> Self {
        GpuShareError::ConfigError(err.to_string())
    }
}

impl From<virt::error::Error> for GpuShareError {
    fn from(error: virt::error::Error) -> Self {
        GpuShareError::ConnectionError(error.to_string())
    }
}