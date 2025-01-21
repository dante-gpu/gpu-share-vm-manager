
pub enum GpuShareError {
    VmError(VmErrorKind),
    GpuError(GpuErrorKind),
    ConfigError(ConfigErrorKind),
    SystemError(SystemErrorKind),
}

// Recovery procedures
pub trait ErrorRecovery {
    fn recover(&self) -> Result<(), GpuShareError>;
    fn rollback(&self) -> Result<(), GpuShareError>;
}