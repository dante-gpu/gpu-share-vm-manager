/*
 * DanteGPU API Routes Implementation 
 * -----------------------------------------------
 * Author: @virjilakrum
 * Project: gpu-share-vm-manager
 *
 * Bu dosya Docker tabanlı VM yönetim API'sini içerir.
 */

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
    Router,
};
use serde_json::json;
use serde::{Deserialize, Serialize};
use tracing::info;
use std::sync::Arc;
use tokio::sync::{oneshot, Mutex};

// Proje içi bağımlılıklar
use crate::core::docker_manager::DockerManager;
use crate::gpu::GPUManager;
use crate::monitoring::MetricsCollector;
use crate::gpu::virtual_gpu::GPUPool;
use crate::users::UserManager;
use crate::billing::BillingSystem;

/// Shared application state used by API route handlers.
pub struct AppState {
    pub docker: Arc<Mutex<DockerManager>>,
    pub gpu_manager: Arc<Mutex<GPUManager>>,
    pub metrics: Arc<Mutex<MetricsCollector>>,
    pub shutdown_signal: Arc<Mutex<Option<oneshot::Sender<()>>>>,
    pub shutdown_receiver: Arc<Mutex<Option<oneshot::Receiver<()>>>>,
    pub gpupool: Arc<Mutex<GPUPool>>,
    pub user_manager: Arc<Mutex<UserManager>>,
    pub billing_system: Arc<Mutex<BillingSystem>>,
}

/// Creates an Axum router with all endpoints.
pub fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/", axum::routing::get(root_handler))
        .route("/health", axum::routing::get(health_check))
        .route("/shutdown", axum::routing::post(shutdown_handler))
        .with_state(state)
}

/// Hata numaraları enum'u
#[derive(Clone, Debug, Serialize)]
pub enum ErrorNumber {
    ContainerNotFound,
    OperationFailed,
    InternalError,
    GPUTransferError,
}

/// Özelleştirilmiş hata yanıtı
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub code: u16,
}

impl ErrorResponse {
    pub fn new<T: ToString>(error_number: ErrorNumber, message: T) -> Self {
        let code = match error_number {
            ErrorNumber::ContainerNotFound => 404,
            ErrorNumber::OperationFailed => 400,
            ErrorNumber::InternalError => 500,
            ErrorNumber::GPUTransferError => 409,
        };
        Self {
            error: message.to_string(),
            code,
        }
    }
}

impl IntoResponse for ErrorResponse {
    fn into_response(self) -> axum::response::Response {
        (
            StatusCode::from_u16(self.code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            Json(self),
        )
            .into_response()
    }
}

/// Hata dönüştürücü
fn handle_error(e: impl Into<anyhow::Error>) -> ErrorResponse {
    let err = e.into();
    ErrorResponse::new(ErrorNumber::InternalError, format!("Docker hatası: {}", err))
}

/// VM Oluşturma İsteği
#[derive(Debug, Deserialize)]
pub struct CreateVMRequest {
    pub name: String,
    pub image: String,
    pub gpu_required: bool,
}

/// VM Detay Yanıtı
#[derive(Debug, Serialize)]
pub struct VMResponse {
    pub id: String,
    pub name: String,
    pub status: String,
    pub gpu_attached: bool,
}

/// VM Oluşturma Handler
#[axum::debug_handler]
pub async fn create_vm(
    State(state): State<Arc<AppState>>,
    Json(params): Json<CreateVMRequest>,
) -> Result<impl IntoResponse, ErrorResponse> {
    info!("🛠️ Yeni container oluşturuluyor: {}", params.name);
    
    let docker = state.docker.lock().await;
    docker.create_container(&params.image, &params.name)
        .await
        .map_err(handle_error)?;

    Ok(Json(json!({
        "status": "success",
        "message": format!("{} adlı container oluşturuldu", params.name)
    })))
}

/// Container Listeleme Handler
#[axum::debug_handler]
pub async fn list_containers(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let docker = state.docker.lock().await;
    let containers = docker.list_containers()
        .await
        .map_err(handle_error)?;

    let mut responses = Vec::new();
    for container in containers {
        responses.push(VMResponse {
            id: container.clone(),
            name: container,
            status: "running".to_string(),
            gpu_attached: false,
        });
    }
    
    Ok(Json(responses))
}

/// GPU Ekleme İsteği
#[derive(Debug, Deserialize)]
pub struct AttachGPURequest {
    pub gpu_id: String,
}

/// GPU Ekleme Handler
#[axum::debug_handler]
pub async fn attach_gpu(
    State(state): State<Arc<AppState>>,
    Path(container_id): Path<String>,
    Json(request): Json<AttachGPURequest>,
) -> Result<impl IntoResponse, ErrorResponse> {
    info!("🎮 GPU ekleniyor: {} -> {}", request.gpu_id, container_id);
    
    let mut gpu_manager = state.gpu_manager.lock().await;
    let docker = state.docker.lock().await;
    
    // Container'ı kontrol et
    let _ = docker.lookup_container(&container_id)
        .await
        .map_err(|e| ErrorResponse::new(
            ErrorNumber::ContainerNotFound,
            format!("Container hatası: {}", e)
        ))?;

    // GPU'yu ekle
    gpu_manager.attach_gpu(&container_id, &request.gpu_id)
        .await
        .map_err(|e| ErrorResponse::new(
            ErrorNumber::GPUTransferError,
            format!("GPU ekleme hatası: {}", e)
        ))?;

    Ok(Json(json!({"status": "GPU başarıyla eklendi"})))
}

/// Diğer handler'lar...
// (Docker işlemleri için gerekli diğer endpoint'ler)

/// Kök Handler
#[axum::debug_handler]
pub async fn root_handler() -> impl IntoResponse {
    Json(json!({
        "message": "DanteGPU Yönetim API'sine Hoş Geldiniz!",
        "endpoints": [
            "/containers - Container listesi",
            "/create - Yeni container oluştur",
            "/gpu/attach - GPU ekleme"
        ]
    }))
}

/// Health Check
#[axum::debug_handler]
pub async fn health_check() -> impl IntoResponse {
    Json(json!({"status": "active", "version": "0.4.2"}))
}

/// Shutdown Handler
#[axum::debug_handler]
pub async fn shutdown_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    info!("🛑 Sistem kapatılıyor...");
    if let Some(sender) = state.shutdown_signal.lock().await.take() {
        let _ = sender.send(());
    }
    Json(json!({"status": "shutdown_initiated"}))
}
