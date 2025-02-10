pub mod device;
pub mod virtual_gpu;

// exports cuz ain't nobody got time for full paths
pub use device::GPUManager;
pub use virtual_gpu::GPUPool;
// pub use device::GPU;