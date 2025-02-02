use clap::{Parser, Subcommand};
use crate::gpu::virtual_gpu::GPUPool;
use crate::users::UserManager;
use crate::billing::{BillingSystem, Transaction};
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Parser)]
#[command(name = "GPUShare")]
#[command(version = "1.0")]
#[command(about = "Decentralized GPU Sharing Platform", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// List available GPUs
    List,
    
    /// Rent a GPU
    Rent {
        #[arg(short, long)]
        gpu_id: u32,
        
        #[arg(short, long)]
        user: String,
        
        #[arg(short, long)]
        duration: u64,
    },
    
    /// Release a GPU
    Release {
        #[arg(short, long)]
        gpu_id: u32,
        
        #[arg(short, long)]
        user: String,
    },
    
    /// Show system status
    Status,
    
    /// Start interactive dashboard
    Dashboard,
}

pub async fn list_gpus(gpupool: Arc<Mutex<GPUPool>>) -> anyhow::Result<()> {
    let gpupool = gpupool.lock().await;
    println!("Available GPUs:");
    for (id, gpu) in &gpupool.gpus {
        println!("GPU {}: {}MB VRAM - {} Cores", 
            id, gpu.vram_mb, gpu.compute_units);
    }
    Ok(())
}

pub async fn rent_gpu(
    gpupool: Arc<Mutex<GPUPool>>,
    user_manager: Arc<Mutex<UserManager>>,
    billing: Arc<Mutex<BillingSystem>>,
    gpu_id: u32,
    user: &str,
    duration_minutes: u64
) -> anyhow::Result<()> {
    let mut gpupool = gpupool.lock().await;
    let mut user_manager = user_manager.lock().await;
    
    let cost = gpupool.allocate(user, gpu_id)?;
    user_manager.deduct_credits(user, cost)?;
    
    billing.lock().await.add_transaction(Transaction {
        user_id: user_manager.get_user(user)?.id,
        gpu_id,
        start_time: chrono::Utc::now(),
        duration: std::time::Duration::from_secs(duration_minutes * 60),
        cost,
    });
    
    Ok(())
}

pub async fn show_status(gpupool: Arc<Mutex<GPUPool>>) -> anyhow::Result<()> {
    let gpupool = gpupool.lock().await;
    println!("System Status:");
    println!("Total GPUs: {}", gpupool.gpus.len());
    Ok(())
}