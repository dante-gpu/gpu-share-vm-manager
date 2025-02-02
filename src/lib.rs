pub mod api;
pub mod core;
pub mod gpu;
pub mod monitoring;
pub mod utils;
pub mod config;
pub mod users;
pub mod billing;
pub mod dashboard;

// Re-exports
pub use gpu::virtual_gpu::GPUPool;
pub use users::UserManager;
pub use billing::BillingSystem;
pub use api::routes::{create_router, AppState};
pub use dashboard::start_dashboard;
pub type AsyncMutex<T> = tokio::sync::Mutex<T>;


