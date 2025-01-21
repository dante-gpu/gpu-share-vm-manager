use std::fmt;
use thiserror::Error;
use tracing::error;

#[derive(Error, Debug)]
pub enum GpuShareError {
    #[error("VM Error: {0}")]
    VmError(#[from] VmError),
    
    #[error("GPU Error: {0}")]
    GpuError(#[from] GpuError),
    
    #[error("Configuration Error: {0}")]
    ConfigError(#[from] ConfigError),
    
    #[error("System Error: {0}")]
    SystemError(#[from] SystemError),
    
    #[error("Database Error: {0}")]
    DatabaseError(#[from] DatabaseError),
}

#[derive(Error, Debug)]
pub enum VmError {
    #[error("Failed to create VM: {message}")]
    CreationError {
        message: String,
        vm_name: String,
    },

    #[error("VM not found: {vm_id}")]
    NotFound {
        vm_id: String,
    },

    #[error("Failed to start VM: {message}")]
    StartError {
        message: String,
        vm_id: String,
    },

    #[error("Failed to stop VM: {message}")]
    StopError {
        message: String,
        vm_id: String,
    },

    #[error("Resource allocation failed: {message}")]
    ResourceError {
        message: String,
        resource_type: ResourceType,
    },
}

#[derive(Error, Debug)]
pub enum GpuError {
    #[error("GPU not found: {gpu_id}")]
    NotFound {
        gpu_id: String,
    },

    #[error("GPU already in use: {gpu_id}")]
    AlreadyInUse {
        gpu_id: String,
        vm_id: Option<String>,
    },

    #[error("IOMMU group error: {message}")]
    IommuError {
        message: String,
        group_id: Option<u32>,
    },

    #[error("Driver error: {message}")]
    DriverError {
        message: String,
        driver_name: String,
    },
}

#[derive(Debug)]
pub enum ResourceType {
    Memory,
    Cpu,
    Storage,
    Network,
}

impl fmt::Display for ResourceType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ResourceType::Memory => write!(f, "Memory"),
            ResourceType::Cpu => write!(f, "CPU"),
            ResourceType::Storage => write!(f, "Storage"),
            ResourceType::Network => write!(f, "Network"),
        }
    }
}

pub trait ErrorRecovery {
    /// Attempt to recover from an error
    fn recover(&self) -> Result<(), GpuShareError>;
    
    /// Rollback changes if recovery fails
    fn rollback(&self) -> Result<(), GpuShareError>;
    
    /// Log error and recovery attempt
    fn log_error(&self, error: &GpuShareError) {
        error!(
            error = error.to_string(),
            recovery_attempted = true,
            "Error occurred with recovery attempt"
        );
    }
}

// Error context for tracking error chain
#[derive(Debug)]
pub struct ErrorContext {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub source_location: &'static str,
    pub operation: String,
    pub user_id: Option<String>,
}

// Error recovery implementation for VM errors
pub struct VmErrorRecovery {
    pub vm_id: String,
    pub last_known_state: VmState,
    pub recovery_attempts: u32,
}

impl ErrorRecovery for VmErrorRecovery {
    fn recover(&self) -> Result<(), GpuShareError> {
        if self.recovery_attempts >= 3 {
            return Err(GpuShareError::VmError(VmError::CreationError {
                message: "Maximum recovery attempts reached".to_string(),
                vm_name: self.vm_id.clone(),
            }));
        }

        match self.last_known_state {
            VmState::Running => {
                // Attempt to restart VM
                self.restart_vm()?;
            }
            VmState::Stopped => {
                // Verify VM integrity and resources
                self.verify_vm_integrity()?;
            }
            VmState::Failed => {
                // Clean up resources and try recreation
                self.cleanup_and_recreate()?;
            }
        }

        Ok(())
    }

    fn rollback(&self) -> Result<(), GpuShareError> {
        // Implement rollback logic
        self.cleanup_resources()?;
        self.restore_snapshot()?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub enum VmState {
    Running,
    Stopped,
    Failed,
}

impl VmErrorRecovery {
    fn restart_vm(&self) -> Result<(), GpuShareError> {
        // Implement VM restart logic
        Ok(())
    }

    fn verify_vm_integrity(&self) -> Result<(), GpuShareError> {
        // Implement VM verification logic
        Ok(())
    }

    fn cleanup_and_recreate(&self) -> Result<(), GpuShareError> {
        // Implement cleanup and recreation logic
        Ok(())
    }

    fn cleanup_resources(&self) -> Result<(), GpuShareError> {
        // Implement resource cleanup logic
        Ok(())
    }

    fn restore_snapshot(&self) -> Result<(), GpuShareError> {
        // Implement snapshot restoration logic
        Ok(())
    }
}

// Result type alias for convenience
pub type GpuShareResult<T> = Result<T, GpuShareError>;

// Helper macro for context addition
#[macro_export]
macro_rules! with_context {
    ($result:expr, $operation:expr) => {
        $result.map_err(|e| {
            let context = ErrorContext {
                timestamp: chrono::Utc::now(),
                source_location: std::file!(),
                operation: $operation.to_string(),
                user_id: None,
            };
            error!(
                error = e.to_string(),
                context = ?context,
                "Operation failed"
            );
            e
        })
    };
}