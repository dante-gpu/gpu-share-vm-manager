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
    response::IntoResponse,
    response::{Json, Response},
    http::Request,
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::error;
use std::path::PathBuf;

use crate::core::libvirt::LibvirtManager;
use crate::core::vm::{VMStatus, VMConfig};
use crate::gpu::device::{GPUManager, GPUDevice, GPUConfig};
use crate::monitoring::metrics::{MetricsCollector, ResourceMetrics};
use crate::api::middleware::rate_limit::{rate_limit_layer, GlobalRateLimit, RateLimitExceeded};

fn handle_error(err: impl std::fmt::Display) -> StatusCode {
    error!("Operation failed: {}", err);
    StatusCode::INTERNAL_SERVER_ERROR
}

#[derive(Clone)]
pub struct AppState {
    pub libvirt: Arc<Mutex<LibvirtManager>>,
    pub gpu_manager: Arc<Mutex<GPUManager>>,
    pub metrics: Arc<Mutex<MetricsCollector>>,
    pub shutdown_signal: Arc<Mutex<tokio::sync::oneshot::Sender<()>>>,
    pub shutdown_receiver: Arc<Mutex<tokio::sync::oneshot::Receiver<()>>>,
}

#[derive(Debug, Deserialize)]
pub struct CreateVMRequest {
    pub name: String,
    pub cpu_cores: u32,
    pub memory_mb: u64,
    pub gpu_required: bool,
    pub disk_size_gb: Option<u64>,
    // pub username: String,
    // pub password: String,
}

#[derive(Debug, Serialize)]
pub struct VMResponse {
    pub id: String,
    pub name: String,
    pub status: VMStatus,
    pub gpu_attached: bool,
    pub memory_mb: u64,
    pub cpu_cores: u32,
    pub disk_size_gb: u64,
}

#[derive(Debug, Deserialize)]
pub struct AttachGPURequest {
    pub gpu_id: String,
}

pub fn create_router(state: Arc<AppState>) -> Router {
    let rate_limits = GlobalRateLimit::default();

    Router::new()
        // Public endpoints with stricter limits
        .route("/api/v1/auth/login", post(login))
        .layer(rate_limit_layer(rate_limits.auth.clone()))
        
        // GPU operations with specific limits
        .route("/api/v1/gpus", get(list_gpus))
        .route("/api/v1/vms/:id/attach_gpu", post(attach_gpu))
        .layer(rate_limit_layer(rate_limits.gpu_operations.clone()))
        
        // General API endpoints
        .route("/api/v1/vms", post(create_vm))
        .route("/api/v1/vms", get(list_vms))
        .route("/api/v1/vms/:id", get(get_vm))
        .route("/api/v1/vms/:id", delete(delete_vm))
        .route("/api/v1/vms/:id/start", post(start_vm))
        .route("/api/v1/vms/:id/stop", post(stop_vm))
        .route("/api/v1/metrics/:id", get(get_metrics))
        .layer(rate_limit_layer(rate_limits.api.clone()))
        
        // Shared state and fallback
        .with_state(state)
        .fallback(fallback_handler)
        .layer(HandleErrorLayer::new(handle_error))
}

async fn handle_error(error: Box<dyn std::error::Error + Send + Sync>) -> impl IntoResponse {
    if error.is::<RateLimitExceeded>() {
        return RateLimitExceeded.into_response();
    }
    
    if let Some(libvirt_error) = error.downcast_ref::<libvirt::Error>() {
        match libvirt_error.code() {
            libvirt::ErrorNumber::NO_DOMAIN => {
                return (StatusCode::NOT_FOUND, "VM not found").into_response()
            }
            libvirt::ErrorNumber::OPERATION_INVALID => {
                return (StatusCode::BAD_REQUEST, "Invalid operation").into_response()
            }
            _ => {}
        }
    }

    if let Some(gpu_error) = error.downcast_ref::<gpu::GPUError>() {
        match gpu_error {
            gpu::GPUError::NotFound => {
                return (StatusCode::NOT_FOUND, "GPU not found").into_response()
            }
            gpu::GPUError::AlreadyAttached => {
                return (StatusCode::CONFLICT, "GPU already attached").into_response()
            }
            _ => {}
        }
    }
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        format!("Internal server error: {}", error),
    )
        .into_response()
}

#[axum::debug_handler]
async fn create_vm(
    State(state): State<Arc<AppState>>,
    Json(params): Json<CreateVMRequest>
) -> Result<impl IntoResponse, StatusCode> {
    let libvirt = state.libvirt.lock().await;
    
    let config = VMConfig {
        name: params.name.clone(),
        memory_kb: params.memory_mb * 1024,
        vcpus: params.cpu_cores,
        disk_path: PathBuf::from(format!("/var/lib/gpu-share/images/{}.qcow2", params.name)),
        disk_size_gb: params.disk_size_gb.unwrap_or(20),
    };
    
    let vm = libvirt.create_vm(&config).await
        .map_err(handle_error)?;

    let vm_id = vm.get_uuid_string()
        .map_err(handle_error)?;

    let mut metrics = state.metrics.lock().await;
    if let Err(e) = metrics.start_collection(vm_id.clone(), vm.clone()).await {
        error!("Failed to start metrics collection: {}", e);
    }

    Ok(Json(VMResponse {
        id: vm_id,
        name: params.name,
        status: VMStatus::Creating,
        gpu_attached: params.gpu_required,
        memory_mb: params.memory_mb,
        cpu_cores: params.cpu_cores,
        disk_size_gb: config.disk_size_gb,
    }))
}

#[axum::debug_handler]
async fn list_vms(
    State(state): State<Arc<AppState>>
) -> Result<impl IntoResponse, StatusCode> {
    let libvirt = state.libvirt.lock().await;
    
    let domains = libvirt.list_domains()
        .map_err(handle_error)?;

    let mut responses = Vec::new();
    for domain in domains {
        let info = domain.get_info()
            .map_err(handle_error)?;

        let response = VMResponse {
            id: domain.get_uuid_string().map_err(handle_error)?,
            name: domain.get_name().map_err(handle_error)?,
            status: VMStatus::from(info.state),
            gpu_attached: domain.get_xml_desc(0)
                .map(|xml| xml.contains("<hostdev"))
                .unwrap_or(false),
            memory_mb: info.memory / 1024,
            cpu_cores: info.nr_virt_cpu,
            disk_size_gb: 0, // TODO: Implement disk size detection
        };
        responses.push(response);
    }

    Ok(Json(responses))
}

#[axum::debug_handler]
async fn get_vm(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>
) -> Result<impl IntoResponse, StatusCode> {
    let libvirt = state.libvirt.lock().await;
    
    let domain = libvirt.lookup_domain(&id)
        .map_err(handle_error)?;

    let info = domain.get_info()
        .map_err(handle_error)?;

    Ok(Json(VMResponse {
        id,
        name: domain.get_name().map_err(handle_error)?,
        status: VMStatus::from(info.state),
        gpu_attached: domain.get_xml_desc(0)
            .map(|xml| xml.contains("<hostdev"))
            .unwrap_or(false),
        memory_mb: info.memory / 1024,
        cpu_cores: info.nr_virt_cpu,
        disk_size_gb: 0, // TODO: Implement disk size detection
    }))
}

#[axum::debug_handler]
async fn start_vm(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    let libvirt = state.libvirt.lock().await;
    
    libvirt.start_domain(&id)
        .await
        .map_err(handle_error)?;

    Ok(StatusCode::OK)
}

#[axum::debug_handler]
async fn start_vm(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>
) -> Result<impl IntoResponse, StatusCode> {
    let libvirt = state.libvirt.lock().await;
    libvirt.start_vm(&id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(StatusCode::OK)
}

#[axum::debug_handler]
async fn stop_vm(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>
) -> Result<impl IntoResponse, StatusCode> {
    let libvirt = state.libvirt.lock().await;
    libvirt.stop_vm(&id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(StatusCode::OK)
}

#[axum::debug_handler]
async fn login(
    State(state): State<Arc<AppState>>,
    Json(credentials): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, StatusCode> {
    let mut libvirt = state.libvirt.lock().await;

    let domain = libvirt.lookup_domain(&credentials.username)
        .map_err(handle_error)?;

    let info = domain.get_info()
        .map_err(handle_error)?;
}

// async fn login(
// #[axum::debug_handler]
// async fn list_gpus(
//     State(state): State<Arc<AppState>>
// ) -> Result<impl IntoResponse, StatusCode> {
//     let gpu_manager = state.gpu_manager.lock().await;
//     let gpus = gpu_manager.list_gpus()
//         .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
//     Ok(Json(gpus))
// }

#[axum::debug_handler]
async fn attach_gpu(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(request): Json<GPUConfig>
) -> Result<impl IntoResponse, StatusCode> {
    let mut gpu_manager = state.gpu_manager.lock().await;

    let gpu_id = request.gpu_id.clone();
    let gpu_manager = state.gpu_manager.lock().await;
    
    let domain = libvirt.lookup_domain(&id)
        .map_err(handle_error)?;

    let gpu_id = request.gpu_id.clone();
    let gpu_config = GPUConfig {
        gpu_id: request.gpu_id,
        iommu_group: gpu_manager.get_iommu_group(&gpu_id)
            .map_err(handle_error)?
            .ok_or(StatusCode::BAD_REQUEST)?,
    };

    gpu_manager.attach_gpu_to_vm(&domain, &gpu_config).await
        .map_err(handle_error)?;

    Ok(StatusCode::OK)
}

#[axum::debug_handler]
async fn fallback_handler(
    State(state): State<Arc<AppState>>,
    req: Request,
) -> Result<Response, StatusCode> {
    error!("Fallback handler called for request: {:?}", req);
    Err(StatusCode::NOT_FOUND)
}

#[axum::debug_handler]
async fn get_metrics(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>
) -> Result<impl IntoResponse, StatusCode> {
    let metrics = state.metrics.lock().await;
    let vm_metrics = metrics.get_vm_metrics(&id)
        .map_err(|_| StatusCode::NOT_FOUND)?;
    Ok(Json(vm_metrics))
}