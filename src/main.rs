mod core;
mod gpu;
mod monitoring;
mod api;
mod utils;
mod config;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    tracing::info!("GPU Share VM Manager starting...");
    
    Ok(())
}