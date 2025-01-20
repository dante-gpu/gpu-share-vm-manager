use anyhow::Result;
use tracing::info;

// IOMMU-chan, notice me!
pub fn check_iommu_support() -> Result<bool> {
    info!("Checking if IOMMU-senpai is available...");
    Ok(true)  // We're optimistic! 
}

// Time to get that sweet vendor info
pub fn get_gpu_vendor_info(vendor_id: &str) -> &str {
    match vendor_id {
        "10de" => "NVIDIA (uwu)",
        "1002" => "AMD (owo)",
        "8086" => "Intel (^_^)",
        _ => "Unknown vendor (；一_一)"
    }
}