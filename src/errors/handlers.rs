use super::{GpuShareError, ErrorContext, ErrorRecovery};
use tracing::{error, info, warn};

pub struct ErrorHandler {
    max_retries: u32,
    current_retries: u32,
}

impl ErrorHandler {
    pub fn new(max_retries: u32) -> Self {
        Self {
            max_retries,
            current_retries: 0,
        }
    }

    pub fn handle<T, F>(&mut self, operation: F) -> Result<T, GpuShareError>
    where
        F: Fn() -> Result<T, GpuShareError>,
    {
        while self.current_retries < self.max_retries {
            match operation() {
                Ok(result) => {
                    if self.current_retries > 0 {
                        info!(
                            retries = self.current_retries,
                            "Operation succeeded after retries"
                        );
                    }
                    return Ok(result);
                }
                Err(e) => {
                    self.current_retries += 1;
                    warn!(
                        error = e.to_string(),
                        retry_count = self.current_retries,
                        max_retries = self.max_retries,
                        "Operation failed, retrying"
                    );

                    if self.current_retries >= self.max_retries {
                        error!(
                            error = e.to_string(),
                            "Maximum retries reached, operation failed"
                        );
                        return Err(e);
                    }
                }
            }
        }

        Err(GpuShareError::SystemError(SystemError::new(
            "Maximum retries reached",
        )))
    }
}

#[derive(Debug, Error)]
pub struct SystemError {
    message: String,
}

impl SystemError {
    pub fn new(message: &str) -> Self {
        Self {
            message: message.to_string(),
        }
    }
}

impl std::fmt::Display for SystemError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "System Error: {}", self.message)
    }
}