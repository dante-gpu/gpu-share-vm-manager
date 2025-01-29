/*
Cross-platform OS detection utilities
Provides platform detection and feature flagging
across Linux, macOS, and Windows
*/

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Linux,
    MacOS,
    Windows,
    Unknown,
}

impl Platform {
    /// Get current platform as enum variant
    pub fn current() -> Self {
        #[cfg(target_os = "linux")]
        return Platform::Linux;
        
        #[cfg(target_os = "macos")]
        return Platform::MacOS;
        
        #[cfg(target_os = "windows")]
        return Platform::Windows;
        
        #[cfg(not(any(
            target_os = "linux",
            target_os = "macos",
            target_os = "windows"
        )))]
        Platform::Unknown
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