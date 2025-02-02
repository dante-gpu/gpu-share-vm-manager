use thiserror::Error;

#[derive(Error, Debug)]
pub enum GpuShareError {
    #[error("Docker connection error: {0}")]
    ConnectionError(#[source] anyhow::Error),
    
    #[error("Container operation failed: {0}")]
    OperationFailed(String),
    
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