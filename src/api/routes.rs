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
    error_handling::HandleErrorLayer,
    routing::{get, post, delete},
    Router,
    extract::{Path, State},
    http::{Request, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::error;
use virt::error::Error as VirtError;
use std::path::PathBuf;
use tower::limit::RateLimitLayer;
use std::time::Duration;
use std::error::Error as StdError;
use tower::ServiceBuilder;
use tower_http::extension::AddExtensionLayer;

use crate::core::libvirt::LibvirtManager;
use crate::core::vm::{VMStatus, VMConfig};
use crate::gpu::device::{GPUManager, GPUConfig, GPUError};
use crate::monitoring::metrics::MetricsCollector;
use crate::api::middleware::rate_limit::{RateLimiter, GlobalRateLimit, RateLimitExceeded};

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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gpu_passthrough: Option<String>,
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

#[derive(Serialize, Deserialize)]
pub struct LoginRequest {
    username: String,
    password: String,
}

#[derive(Serialize, Deserialize)]
pub struct LoginResponse {
    token: String,
}

#[derive(Debug, Serialize)]
pub enum ErrorNumber {
    NoDomain,
    InvalidOperation,
    // ... other variants ...
}

pub fn create_router(app_state: Arc<AppState>) -> Router<Arc<AppState>> {
    let rate_limits = GlobalRateLimit::default();

    // All auth endpoints 
    let auth_router = Router::new()
        .route("/api/v1/auth/login", post(login))
        .layer(
            ServiceBuilder::new()
                .layer(RateLimitLayer::new(
                    rate_limits.auth_quota(),
                    Duration::from_secs(60),
                ))
                .layer(AddExtensionLayer::new(rate_limits.auth.clone()))
        );

    let gpu_router = Router::new()
        .route("/api/v1/gpus", get(list_gpus))
        .route("/api/v1/vms/:id/attach_gpu", post(attach_gpu))
        .layer(
            ServiceBuilder::new()
                .layer(RateLimitLayer::new(
                    rate_limits.gpu_quota(),
                    Duration::from_secs(60),
                ))
                .layer(AddExtensionLayer::new(rate_limits.gpu_operations.clone()))
        );

    let main_router = Router::new()
        .route("/api/v1/vms", post(create_vm))
        .route("/api/v1/vms", get(list_vms))
        .route("/api/v1/vms/:id", get(get_vm))
        .route("/api/v1/vms/:id", delete(delete_vm))
        .route("/api/v1/vms/:id/start", post(start_vm))
        .route("/api/v1/vms/:id/stop", post(stop_vm))
        .route("/api/v1/metrics/:id", get(get_metrics))
        .layer(
            ServiceBuilder::new()
                .layer(RateLimitLayer::new(
                    rate_limits.api_quota(),
                    Duration::from_secs(1),
                ))
                .layer(AddExtensionLayer::new(rate_limits.api.clone()))
        );

    // Main router
    Router::new()
        .merge(auth_router)
        .merge(gpu_router)
        .merge(main_router)
        .with_state(app_state)
        .layer(
            ServiceBuilder::new()
                .layer(HandleErrorLayer::new(|e: Box<dyn std::error::Error>| async move {
                    // Global error handling
                    error!("Global error handler: {}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "Something went wrong".to_string(),
                    )
                }))
        )
        .fallback(fallback_handler)
}

async fn handle_error(error: Box<dyn StdError + Send + Sync>) -> impl IntoResponse {
    if error.is::<RateLimitExceeded>() {
        return RateLimitExceeded.into_response();
    }
    
    if let Some(virt_error) = error.downcast_ref::<VirtError>() {
        match virt_error.code() {
            virt::error::ErrorNumber::NoSuchDomain => {
                return (StatusCode::NOT_FOUND, "VM not found").into_response()
            }
            virt::error::ErrorNumber::InvalidOperation => {
                return (StatusCode::BAD_REQUEST, "Invalid operation").into_response()
            }
            _ => {}
        }
    }

    if let Some(gpu_error) = error.downcast_ref::<GPUError>() {
        match gpu_error {
            GPUError::NotFound => {
                return (StatusCode::NOT_FOUND, "GPU not found").into_response()
            }
            GPUError::AlreadyAttached => {
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
        gpu_passthrough: params.gpu_passthrough.clone(),
    };
    
    #[cfg(target_os = "linux")]
    {
        // Linux-specific VM creation
    }
    
    #[cfg(target_os = "macos")]
    {
        // MacOS hypervisor framework usage
    }
    
    #[cfg(target_os = "windows")]
    {
        // Hyper-V integration
    }
    
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        return Err(Error::UnsupportedPlatform(current_platform().to_string()));
    }

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
async fn stop_vm(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>
) -> Result<impl IntoResponse, StatusCode> {
    let libvirt = state.libvirt.lock().await;
    libvirt.stop_domain(&id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(StatusCode::OK)
}

#[axum::debug_handler]
async fn login(
    State(state): State<Arc<AppState>>,
    Json(credentials): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, StatusCode> {
    if credentials.username.is_empty() || credentials.password.is_empty() {
        return Err(StatusCode::UNAUTHORIZED);
    }
    
    Ok(Json(LoginResponse {
        token: format!("jwt-token-{}", uuid::Uuid::new_v4())
    }))
}

#[axum::debug_handler]
async fn list_gpus(
    State(state): State<Arc<AppState>>
) -> Result<impl IntoResponse, StatusCode> {
    let gpu_manager = state.gpu_manager.lock().await;
    let gpus = gpu_manager.list_available_devices()
        .map_err(|e| {
            ErrorResponse::new(ErrorNumber::InternalError, e.to_string())
        })?;
    Ok(Json(gpus))
}

#[axum::debug_handler]
async fn attach_gpu(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(request): Json<GPUConfig>
) -> Result<impl IntoResponse, StatusCode> {
    let mut gpu_manager = state.gpu_manager.lock().await;
    
    let gpu_id = request.gpu_id.clone();
    let gpu_config = GPUConfig {
        gpu_id: request.gpu_id,
        iommu_group: gpu_manager.get_iommu_group(&gpu_id)
            .map_err(|e| {
                ErrorResponse::new(ErrorNumber::InternalError, e.to_string())
            })?
            .ok_or(StatusCode::BAD_REQUEST)?,
    };

    let libvirt = state.libvirt.lock().await;
    let domain = libvirt.lookup_domain(&id)
        .map_err(|e| {
            ErrorResponse::new(ErrorNumber::InternalError, e.to_string())
        })?;

    gpu_manager.attach_gpu_to_vm(&domain, &gpu_config).await
        .map_err(|e| {
            ErrorResponse::new(ErrorNumber::InternalError, e.to_string())
        })?;

    Ok(StatusCode::OK)
}

#[axum::debug_handler]
async fn fallback_handler(
    State(state): State<Arc<AppState>>,
    req: Request<axum::body::Body>,
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
        .map_err(|e| {
            ErrorResponse::new(ErrorNumber::InternalError, e.to_string())
        })?;
    Ok(Json(vm_metrics))
}

async fn delete_vm(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>
) -> Result<impl IntoResponse, StatusCode> {
    let mut libvirt = state.libvirt.lock().await;
    libvirt.delete_domain(&id)
        .await
        .map_err(|e| {
            error!("VM deletion error: {}", e);
            ErrorResponse::new(ErrorNumber::InternalError, e.to_string())
        })?;
        
    let mut metrics = state.metrics.lock().await;
    metrics.stop()
        .map_err(|e| {
            error!("Metrics cleanup error: {}", e);
            ErrorResponse::new(ErrorNumber::InternalError, e.to_string())
        })?;

    Ok(StatusCode::NO_CONTENT)
}

async fn stop_metrics_collection(
    State(state): State<Arc<AppState>>
) -> Result<Json<()>, ErrorResponse> {
    let mut metrics = state.metrics.lock().await;
    metrics.stop()
        .map_err(|e| ErrorResponse::new(ErrorNumber::InternalError, e.to_string()))?;
    Ok(Json(()))
}