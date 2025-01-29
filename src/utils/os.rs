#[cfg(target_os = "linux")]
pub fn current_platform() -> Platform {
    Platform::Linux
}

#[cfg(target_os = "macos")]
pub fn current_platform() -> Platform {
    Platform::MacOS
}

#[cfg(target_os = "windows")]
pub fn current_platform() -> Platform {
    Platform::Windows
}

pub enum Platform {
    Linux,
    MacOS,
    Windows,
    Unknown,
} 