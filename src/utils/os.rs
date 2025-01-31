/*
Cross-platform OS detection utilities
Provides platform detection and feature flagging
across Linux, macOS, and Windows
*/

use std::env::consts;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Platform {
    Linux,
    MacOS,
    Windows,
    Unknown,
}

impl Platform {
    /// Get current platform as enum variant
    pub fn current() -> Self {
        match consts::OS {
            "linux" => Platform::Linux,
            "macos" => Platform::MacOS,
            "windows" => Platform::Windows,
            _ => Platform::Unknown,
        }
    }

    /// Check if current platform supports hardware virtualization
    pub fn supports_hardware_virtualization(&self) -> bool {
        match self {
            Platform::Linux => true,
            Platform::MacOS => {
                // Check for Apple Hypervisor framework support
                #[cfg(target_os = "macos")]
                return macos_has_hypervisor_support();
                
                #[cfg(not(target_os = "macos"))]
                false
            }
            Platform::Windows => {
                // Check for Hyper-V or Windows Hypervisor Platform
                #[cfg(target_os = "windows")]
                return windows_has_hyperv();
                
                #[cfg(not(target_os = "windows"))]
                false
            }
            Platform::Unknown => false,
        }
    }
}

#[cfg(target_os = "macos")]
fn macos_has_hypervisor_support() -> bool {
    use std::path::Path;
    Path::new("/System/Library/Extensions/AppleHV.kext").exists()
}

#[cfg(target_os = "windows")]
fn windows_has_hyperv() -> bool {
    use winreg::enums::HKEY_LOCAL_MACHINE;
    use winreg::RegKey;
    
    let key = RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Virtualization");
    
    key.is_ok()
}

pub fn current_platform() -> Platform {
    // --Platform detection logic--
    Platform::current()
} 