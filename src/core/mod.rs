pub mod errors;
pub mod resource_manager;
pub mod vm;
pub mod libvirt;

// exports for lazy devs like us
pub use libvirt::LibvirtManager; 