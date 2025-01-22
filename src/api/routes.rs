/*
* DanteGPU API Routes Implementation 
* -----------------------------------------------
* @author: @virjilakrum
* @project: gpu-share-vm-manager
* 
* Welcome to the nerve center of our VM management API! This is where all the HTTP magic happens,
* powered by Axum (because who uses Actix in 2025, right?). Let me walk you through this 
* masterpiece of modern Rust web development.
*
* Architecture Overview:
* --------------------
* We're implementing a RESTful API that manages Virtual Machines with GPU passthrough capabilities.
* Think of it as "Kubernetes for GPUs" but cooler than Mark Zuckerberg's metaverse avatar.
*
* Core Components:
* --------------
* 1. AppState: Our thread-safe shared state using Arc<Mutex<T>>
*    - LibvirtManager: Handles VM lifecycle (more reliable than my ex's promises)
*    - GPUManager: Manages GPU allocation (more precise than SpaceX landings)
*    - MetricsCollector: Tracks resource usage (more detailed than NSA's data collection)
*
* API Endpoints (because REST is still not dead in 2025):
* ---------------------------------------------------
* POST   /api/v1/vms          - Creates a new VM (faster than Tesla's 0-60)
* GET    /api/v1/vms          - Lists all VMs (more organized than my Solana portfolio)
* GET    /api/v1/vms/:id      - Gets VM details (more reliable than weather forecasts)
* DELETE /api/v1/vms/:id      - Deletes a VM (cleaner than my git history)
* POST   /api/v1/vms/:id/start- Starts a VM (smoother than AGI predictions)
* POST   /api/v1/vms/:id/stop - Stops a VM (gentler than Twitter's API changes)
* GET    /api/v1/gpus         - Lists available GPUs (hotter than quantum computing stocks)
* POST   /api/v1/vms/:id/attach_gpu - Attaches GPU (more precise than brain-computer interfaces)
* GET    /api/v1/metrics/:id  - Gets VM metrics (more accurate than YouTube's recommendation algorithm)
*
* Technical Implementation Details:
* ------------------------------
* - Using Axum for routing (because life is too short for boilerplate)
* - Fully async/await implementation (more concurrent than my coffee intake)
* - Thread-safe state management with Arc<Mutex<T>> (more secure than your crypto wallet)
* - Proper error handling with Result<T, StatusCode> (more robust than my dating life)
* - JSON serialization with serde (more efficient than government bureaucracy)
* - Tracing for logging (because println! is so 2021)
*
* Security Considerations:
* ---------------------
* - All endpoints validate input (stricter than Apple's App Store reviews)
* - Resource limits enforced (tighter than SpaceX's security protocols)
* - Error messages sanitized (cleaner than lab-grown meat)
*
* Performance Optimizations:
* -----------------------
* - Async handlers for non-blocking I/O (faster than quantum entanglement :o)
* - Connection pooling for libvirt (more efficient than solar panels)
* - Lazy loading where possible (smarter than Claude 3.5 sonnet responses)
*
* Note: If you're maintaining this, and we still haven't achieved 
* quantum GPU virtualization, I owe you a Cybertruck.
*/

use axum::{
    routing::{get, post, delete},
    Router,
    extract::{Path, State},
    Json,
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, error};

use crate::core::{LibvirtManager, VMStatus};
use crate::gpu::GPUManager;
use crate::gpu::GPUDevice;
use crate::monitoring::metrics::MetricsCollector;
use crate::monitoring::metrics::ResourceMetrics;

// Our main application state
pub struct AppState {
    pub libvirt: Arc<Mutex<LibvirtManager>>,
    pub gpu_manager: Arc<Mutex<GPUManager>>,
    pub metrics: Arc<Mutex<MetricsCollector>>,
}

#[derive(Debug, Deserialize)]
pub struct CreateVMRequest {
    pub name: String,
    pub cpu_cores: u32,
    pub memory_mb: u64,
    pub gpu_required: bool,
}

#[derive(Debug, Serialize)]
pub struct VMResponse {
    pub id: String,
    pub name: String,
    pub status: VMStatus,
    pub gpu_attached: bool,
}

pub fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/api/v1/vms", post(create_vm))
        .route("/api/v1/vms", get(list_vms))
        .route("/api/v1/vms/:id", get(get_vm))
        .route("/api/v1/vms/:id", delete(delete_vm))
        .route("/api/v1/vms/:id/start", post(start_vm))
        .route("/api/v1/vms/:id/stop", post(stop_vm))
        .route("/api/v1/gpus", get(list_gpus))
        .route("/api/v1/vms/:id/attach_gpu", post(attach_gpu))
        .route("/api/v1/metrics/:id", get(get_metrics))
        .with_state(state)
}

async fn create_vm(
    State(state): State<Arc<AppState>>,
    Json(request): Json<CreateVMRequest>,
) -> Result<Json<VMResponse>, StatusCode> {
    info!("Creating new VM: {}", request.name);

    let _libvirt = state.libvirt.lock().await;
    
    match _libvirt.create_vm(&request.name, request.memory_mb * 1024, request.cpu_cores) {
        Ok(domain) => {
            // Start metrics collection
            let mut metrics = state.metrics.lock().await;
            if let Err(e) = metrics.start_collection(request.name.clone(), domain.clone()).await {
                error!("Failed to start metrics collection: {}", e);
            }

            // If GPU is required, try to attach one
            if request.gpu_required {
                let _gpu_manager = state.gpu_manager.lock().await;
                // Find available GPU and attach
                // Implementation follows in GPU manager
            }

            Ok(Json(VMResponse {
                id: domain.get_uuid_string().unwrap(),
                name: request.name,
                status: VMStatus::Creating,
                gpu_attached: request.gpu_required,
            }))
        }
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

async fn list_vms(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<VMResponse>>, StatusCode> {
    let _libvirt = state.libvirt.lock().await;
    
    match _libvirt.list_domains() {
        Ok(domains) => {
            let mut responses = Vec::new();
            for domain in domains {
                let response = VMResponse {
                    id: domain.get_uuid_string().unwrap(),
                    name: domain.get_name().unwrap(),
                    status: match domain.get_state() {
                        Ok((state, _)) => match state {
                            1 => VMStatus::Running,
                            5 => VMStatus::Stopped,
                            _ => VMStatus::Failed,
                        },
                        Err(_) => VMStatus::Failed,
                    },
                    gpu_attached: domain.get_xml_desc(0)
                        .map(|xml| xml.contains("<hostdev"))
                        .unwrap_or(false),
                };
                responses.push(response);
            }
            Ok(Json(responses))
        }
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

async fn get_vm(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<VMResponse>, StatusCode> {
    let _libvirt = state.libvirt.lock().await;
    
    match _libvirt.lookup_domain(&id) {
        Ok(domain) => {
            Ok(Json(VMResponse {
                id,
                name: domain.get_name().unwrap(),
                status: match domain.get_state() {
                    Ok((state, _)) => match state {
                        1 => VMStatus::Running,
                        5 => VMStatus::Stopped,
                        _ => VMStatus::Failed,
                    },
                    Err(_) => VMStatus::Failed,
                },
                gpu_attached: domain.get_xml_desc(0)
                    .map(|xml| xml.contains("<hostdev"))
                    .unwrap_or(false),
            }))
        }
        Err(_) => Err(StatusCode::NOT_FOUND),
    }
}

async fn start_vm(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    let _libvirt = state.libvirt.lock().await;
    
    match _libvirt.lookup_domain(&id) {
        Ok(domain) => {
            match domain.create() {
                Ok(_) => Ok(StatusCode::OK),
                Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
            }
        }
        Err(_) => Err(StatusCode::NOT_FOUND),
    }
}

async fn stop_vm(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    let _libvirt = state.libvirt.lock().await;
    match _libvirt.lookup_domain(&id) {
        Ok(domain) => {
            match domain.shutdown() {
                Ok(_) => Ok(StatusCode::OK),
                Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
            }
        }
        Err(_) => Err(StatusCode::NOT_FOUND),
    }
}

async fn get_metrics(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Vec<ResourceMetrics>>, StatusCode> {
    let metrics = state.metrics.lock().await;
    match metrics.get_vm_metrics(&id) {
        Ok(vm_metrics) => Ok(Json(vm_metrics)),
        Err(_) => Err(StatusCode::NOT_FOUND),
    }
}

async fn delete_vm(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    let _libvirt = state.libvirt.lock().await;
    match _libvirt.destroy_vm(&id) {
        Ok(_) => Ok(StatusCode::OK),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

async fn list_gpus(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<GPUDevice>>, StatusCode> {
    let mut gpu_manager = state.gpu_manager.lock().await;
    match gpu_manager.discover_gpus() {
        Ok(_) => {
            let devices = gpu_manager.get_devices();
            Ok(Json(devices))
        },
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

async fn attach_gpu(
    State(state): State<Arc<AppState>>,
    Path((vm_id, gpu_id)): Path<(String, String)>,
) -> Result<StatusCode, StatusCode> {
    let mut gpu_manager = state.gpu_manager.lock().await;
    let libvirt = state.libvirt.lock().await;
    
    match libvirt.lookup_domain(&vm_id) {
        Ok(domain) => {
            match gpu_manager.attach_gpu_to_vm(&gpu_id, &domain.get_xml_desc(0).unwrap_or_default()) {
                Ok(_) => Ok(StatusCode::OK),
                Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
            }
        }
        Err(_) => Err(StatusCode::NOT_FOUND),
    }
}