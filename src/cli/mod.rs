/*
* DanteGPU Command Line Interface
* ------------------------------
* @author: virjilakrum
* @project: gpu-share-vm-manager
* @status: it-aint-much-but-its-honest-work 🚜
* 
* Welcome to the command-line paradise! This is where we turn your terminal 
* commands into VM magic (and occasional chaos).
*
* Architecture Overview:
* -------------------
* We're implementing a modern CLI architecture using clap (because life's too short
* for getopt). Our command structure follows the git-style subcommand pattern
* (because if it's good enough for Linus, it's good enough for us).
*
* Command Structure:
* ---------------
* gpu-share
* ├── serve [--port]          // For when you're feeling like a sysadmin
* ├── vm                      // VM management (like herding cats, but virtual)
* │   ├── list               // Shows all VMs (the good, the bad, and the zombie)
* │   ├── create             // Spawns a new VM (mkdir -p /dev/hopes_and_dreams)
* │   ├── start              // Wakes up your VM (better than your morning alarm)
* │   ├── stop               // Puts VM to sleep (no sedatives required)
* │   └── delete             // rm -rf but for VMs (handle with care!)
* ├── gpu                     // GPU management (because sharing is caring)
* │   ├── list               // Shows available GPUs (and their relationship status)
* │   ├── attach             // GPU marriage ceremony with VM
* │   └── detach             // GPU divorce proceedings
* └── init                    // Generates config (mkdir -p /etc/good_intentions)
*
* Technical Implementation:
* ----------------------
* - Built on clap (because real devs don't parse --help manually)
* - Async command handling (because waiting is for Windows updates)
* - Colored output (because monochrome is so mainframe)
* - Error propagation that would make Rust evangelists proud
* - Runtime management smoother than your deployment pipeline
*
* Error Handling Strategy:
* ---------------------
* - Custom error types (because Error: Error is not helpful)
* - Colored error messages (red = bad, green = good, simple as that)
* - Graceful failures (fails faster than your last relationship)
* - Comprehensive error messages (more detailed than your code reviews)
*
* State Management:
* --------------
* - Settings passed through like hot potato
* - Libvirt connections managed like your AWS budget :')
* - GPU management more precise than a surgeon with OCD
* - Resource cleanup better than your git history
*
* Usage Examples:
* -------------
* ```bash
* # Start the server (and your journey to VM enlightenment)
* gpu-share serve --port 1337
*
* # Create a VM (may require sacrifice to the RAM gods)
* gpu-share vm create --name totally-not-mining-crypto
*
* # Attach GPU (please sign the EULA with your soul)
* gpu-share gpu attach --vm-name vm01 --gpu-id 0
* ```
*
* Pro Tips:
* --------
* 1. Always check VM status before panic
* 2. GPUs are like parking spots - always taken when you need one
* 3. Keep your config clean (unlike your browser history)
* 4. When in doubt, turn it off and on again (ancient IT wisdom)
*
* Remember: With great GPU power comes great electricity bills.
* May your latency be low and your uptime high! 
*/

use clap::{Parser, Subcommand};
use colored::Colorize;
use tokio::runtime::Runtime;
use std::path::PathBuf;
use tracing::{info, error};

use crate::core::{LibvirtManager, VirtualMachine};
use crate::gpu::GPUManager;
use crate::config::Settings;

#[derive(Parser)]
#[command(name = "gpu-share")]
#[command(about = "GPU Share VM Manager CLI", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    #[arg(short, long, value_name = "CONFIG")]
    config: Option<PathBuf>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the API server
    Serve {
        #[arg(short, long)]
        port: Option<u16>,
    },
    /// Manage virtual machines
    VM {
        #[command(subcommand)]
        command: VMCommands,
    },
    /// Manage GPU devices
    GPU {
        #[command(subcommand)]
        command: GPUCommands,
    },
    /// Generate default configuration
    Init {
        #[arg(short, long)]
        force: bool,
    },
}

#[derive(Subcommand)]
enum VMCommands {
    /// List all virtual machines
    List,
    /// Create a new virtual machine
    Create {
        #[arg(short, long)]
        name: String,
        #[arg(short, long)]
        memory: Option<u64>,
        #[arg(short, long)]
        vcpus: Option<u32>,
        #[arg(short, long)]
        gpu: bool,
    },
    /// Start a virtual machine
    Start {
        #[arg(short, long)]
        name: String,
    },
    /// Stop a virtual machine
    Stop {
        #[arg(short, long)]
        name: String,
    },
    /// Delete a virtual machine
    Delete {
        #[arg(short, long)]
        name: String,
    },
}

#[derive(Subcommand)]
enum GPUCommands {
    /// List available GPUs
    List,
    /// Attach GPU to VM
    Attach {
        #[arg(short, long)]
        vm_name: String,
        #[arg(short, long)]
        gpu_id: String,
    },
    /// Detach GPU from VM
    Detach {
        #[arg(short, long)]
        vm_name: String,
    },
}

pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    
    // Load configuration
    let settings = match &cli.config {
        Some(path) => Settings::new_from_file(path)?,
        None => Settings::new()?,
    };

    match cli.command {
        Commands::Serve { port } => {
            let server_port = port.unwrap_or(settings.server.port);
            info!("Starting server on port {}", server_port);
            crate::run_server(settings, server_port).await?;
        }
        Commands::VM { command } => handle_vm_command(command, &settings).await?,
        Commands::GPU { command } => handle_gpu_command(command, &settings).await?,
        Commands::Init { force } => {
            handle_init_command(force)?;
        }
    }

    Ok(())
}

async fn handle_vm_command(command: VMCommands, settings: &Settings) -> Result<(), Box<dyn std::error::Error>> {
    let libvirt = LibvirtManager::new()?;

    match command {
        VMCommands::List => {
            println!("{}", "Virtual Machines:".bold());
            let vms = libvirt.list_all_vms().await?;
            for vm in vms {
                let status = match vm.status {
                    VMStatus::Running => "Running".green(),
                    VMStatus::Stopped => "Stopped".red(),
                    _ => "Unknown".yellow(),
                };
                println!("- {} ({})", vm.name, status);
            }
        }
        VMCommands::Create { name, memory, vcpus, gpu } => {
            let mem = memory.unwrap_or(settings.libvirt.default_memory_mb);
            let cpus = vcpus.unwrap_or(settings.libvirt.default_vcpus);
            
            info!("Creating VM: {} (Memory: {}MB, vCPUs: {})", name, mem, cpus);
            libvirt.create_vm(&name, mem * 1024, cpus)?;
            
            if gpu {
                let mut gpu_manager = GPUManager::new()?;
                if let Err(e) = gpu_manager.attach_first_available_gpu(&name) {
                    error!("Failed to attach GPU: {}", e);
                }
            }
            
            println!("{} VM '{}' created successfully", "✓".green(), name);
        }
        // TODO: Implement other VM commands... - @virjilakrum
    }

    Ok(())
}

async fn handle_gpu_command(command: GPUCommands, settings: &Settings) -> Result<(), Box<dyn std::error::Error>> {
    let mut gpu_manager = GPUManager::new()?;

    match command {
        GPUCommands::List => {
            println!("{}", "Available GPUs:".bold());
            let gpus = gpu_manager.discover_gpus()?;
            for gpu in gpus {
                let status = if gpu.is_available {
                    "Available".green()
                } else {
                    "In Use".red()
                };
                println!("- {} ({}) [{}]", gpu.id, gpu.vendor_id, status);
            }
        }
        // TODO: Implement other GPU commands... - @virjilakrum
    }

    Ok(())
}

fn handle_init_command(force: bool) -> Result<(), Box<dyn std::error::Error>> {
    let config_dir = PathBuf::from("config");
    if config_dir.exists() && !force {
        error!("Configuration directory already exists. Use --force to overwrite.");
        return Ok(());
    }

    std::fs::create_dir_all(&config_dir)?;
    let default_config = crate::config::generate_default_config();
    let config_str = toml::to_string_pretty(&default_config)?;
    std::fs::write(config_dir.join("default.toml"), config_str)?;

    println!("{} Default configuration generated", "✓".green());
    Ok(())
}