pub mod vm;
pub mod libvirt;

// exports for lazy devs like us
pub use libvirt::LibvirtManager;
pub use vm::{VirtualMachine, VMStatus, VMResources};