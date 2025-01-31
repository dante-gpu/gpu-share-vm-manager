#[macro_export]
macro_rules! platform_require {
    (linux) => {
        #[cfg(not(target_os = "linux"))]
        compile_error!("This feature is only available on Linux");
    };
    (macos) => {
        #[cfg(not(target_os = "macos"))]
        compile_error!("This feature is only available on macOS");
    };
    (windows) => {
        #[cfg(not(target_os = "windows"))]
        compile_error!("This feature is only available on Windows");
    };
} 