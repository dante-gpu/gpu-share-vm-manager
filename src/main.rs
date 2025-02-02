// src/main.rs
//! Main entry point for the GPU Share VM Manager application.

use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::info;
use tracing_subscriber;
use tokio::net::TcpListener;
use anyhow::Result;
use clap::Parser;

// Local imports
use gpu_share_vm_manager::{
    utils::cli::{Cli, Commands, list_gpus, rent_gpu, show_status},
    dashboard::start_dashboard,
    api::routes::{create_router, AppState},
    core::docker_manager::DockerManager,
    gpu::{GPUManager, virtual_gpu::GPUPool},
    monitoring::MetricsCollector,
    users::UserManager,
    billing::BillingSystem
};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();
    info!("🏗️ Starting DanteGPU Server..");

    let cli = Cli::parse();
    
    // State initialization
    let app_state = Arc::new(AppState {
        docker: Arc::new(Mutex::new(DockerManager::new()?)),
        gpu_manager: Arc::new(Mutex::new(GPUManager::new()?)),
        metrics: Arc::new(Mutex::new(MetricsCollector::new(5, 24))),
        shutdown_signal: Arc::new(Mutex::new(None)),
        shutdown_receiver: Arc::new(Mutex::new(None)),
        gpupool: Arc::new(Mutex::new(GPUPool::new())),
        user_manager: Arc::new(Mutex::new(UserManager::new())),
        billing_system: Arc::new(Mutex::new(BillingSystem::new())),
    });

    // Server setup
    let app = create_router(app_state.clone());
    let addr: SocketAddr = "0.0.0.0:3000".parse()?;
    info!("Server listening on {}", addr);

    let listener = TcpListener::bind(addr).await?;
    let app_state_clone = app_state.clone();
    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            if let Some(receiver) = app_state_clone.shutdown_receiver.lock().await.take() {
                let _ = receiver.await;
            }
            info!("🛑 Server stopped gracefully");
        })
        .await?;

    // CLI command handling
    match cli.command {
        Commands::List => {
            list_gpus(app_state.gpupool.clone()).await?;
            Ok(())
        },
        Commands::Rent { gpu_id, user, duration } => {
            rent_gpu(
                app_state.gpupool.clone(),
                app_state.user_manager.clone(),
                app_state.billing_system.clone(),
                gpu_id,
                &user,
                duration
            ).await?;
            Ok(())
        },
        Commands::Release { gpu_id, user: _ } => {
            app_state.gpupool.lock().await.release(gpu_id)?;
            Ok(())
        },
        Commands::Status => {
            show_status(app_state.gpupool.clone()).await?;
            Ok(())
        },
        Commands::Dashboard => {
            start_dashboard(
                app_state.gpupool.clone(),
                app_state.user_manager.clone(),
                app_state.billing_system.clone()
            ).await?;
            Ok(())
        },
    }
}
