use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use std::{num::NonZeroU32, time::Duration};
use tower::{
    layer::util::{Stack, LayerFn},
    Limit, RateLimitLayer,
};

/// Rate limiting configuration for API endpoints
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    pub requests: NonZeroU32,
    pub per_seconds: u64,
}

impl RateLimitConfig {
    /// Creates a new rate limiter layer based on configuration
    pub fn layer(&self) -> RateLimitLayer {
        let window = Duration::from_secs(self.per_seconds);
        RateLimitLayer::new(self.requests.get(), window)
    }
}

/// Global rate limiting configuration
pub struct GlobalRateLimit {
    /// General API rate limits
    pub api: RateLimitConfig,
    /// Stricter limits for GPU operations
    pub gpu_operations: RateLimitConfig,
    /// Authentication-specific limits
    pub auth: RateLimitConfig,
}

impl Default for GlobalRateLimit {
    fn default() -> Self {
        Self {
            api: RateLimitConfig {
                requests: NonZeroU32::new(100).unwrap(),
                per_seconds: 60,
            },
            gpu_operations: RateLimitConfig {
                requests: NonZeroU32::new(30).unwrap(),
                per_seconds: 60,
            },
            auth: RateLimitConfig {
                requests: NonZeroU32::new(10).unwrap(),
                per_seconds: 60,
            },
        }
    }
}

/// Custom rate limit exceeded response
#[derive(Debug)]
pub struct RateLimitExceeded;

impl IntoResponse for RateLimitExceeded {
    fn into_response(self) -> Response {
        (
            StatusCode::TOO_MANY_REQUESTS,
            "Rate limit exceeded. Please try again later.",
        )
            .into_response()
    }
}

/// Layer factory for rate limiting with custom response
pub fn rate_limit_layer(
    config: RateLimitConfig,
) -> Stack<LayerFn<fn(Limit) -> Limit>, RateLimitLayer> {
    let layer = config.layer();
    tower::ServiceBuilder::new()
        .layer(layer)
        .map_err(|_| RateLimitExceeded)
        .into_inner()
} 