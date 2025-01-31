use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
pub use governor::{
    clock::QuantaClock,
    middleware::NoOpMiddleware,
    state::keyed::DashMapStateStore as DashMapStore,
    Quota, RateLimiter,
};
use std::{num::NonZeroU32, sync::Arc, time::Duration};
use tower::limit::RateLimitLayer;
use std::error::Error as StdError;
use std::fmt;

/// Rate limiting configuration for API endpoints
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    pub requests: NonZeroU32,
    pub per_seconds: u64,
}

impl RateLimitConfig {
    /// Creates a new rate limiter layer based on configuration
    pub fn layer(&self) -> RateLimitLayer {
        let rate = self.requests.get() as u64;
        let per = Duration::from_secs(self.per_seconds);
        RateLimitLayer::new(rate, per)
    }
}

/// Global rate limiting configuration
#[derive(Clone)]
pub struct GlobalRateLimit {
    /// General API rate limits
    pub api: Arc<RateLimiter<String, DashMapStore<String>, QuantaClock, NoOpMiddleware>>,
    /// Stricter limits for GPU operations
    pub gpu_operations: Arc<RateLimiter<String, DashMapStore<String>, QuantaClock, NoOpMiddleware>>,
    /// Authentication-specific limits
    pub auth: Arc<RateLimiter<String, DashMapStore<String>, QuantaClock, NoOpMiddleware>>,
}

impl Default for GlobalRateLimit {
    fn default() -> Self {
        let clock = QuantaClock::default();
        Self {
            api: Arc::new(
                RateLimiter::dashmap_with_clock(
                    Quota::per_second(NonZeroU32::new(5).unwrap()).allow_burst(NonZeroU32::new(10).unwrap()),
                    clock.clone(),
                )
            ),
            gpu_operations: Arc::new(
                RateLimiter::dashmap_with_clock(
                    Quota::per_minute(NonZeroU32::new(3).unwrap()).allow_burst(NonZeroU32::new(5).unwrap()),
                    clock.clone(),
                )
            ),
            auth: Arc::new(
                RateLimiter::dashmap_with_clock(
                    Quota::per_minute(NonZeroU32::new(10).unwrap()).allow_burst(NonZeroU32::new(15).unwrap()),
                    clock,
                )
            ),
        }
    }
}

impl GlobalRateLimit {
    pub fn api_quota(&self) -> Quota {
        Quota::per_second(NonZeroU32::new(5).unwrap()).allow_burst(NonZeroU32::new(10).unwrap())
    }

    pub fn gpu_quota(&self) -> Quota {
        Quota::per_minute(NonZeroU32::new(3).unwrap()).allow_burst(NonZeroU32::new(5).unwrap())
    }

    pub fn auth_quota(&self) -> Quota {
        Quota::per_minute(NonZeroU32::new(10).unwrap()).allow_burst(NonZeroU32::new(15).unwrap())
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
    _limiter: Arc<RateLimiter<String, DashMapStore<String>, QuantaClock, NoOpMiddleware>>,
) -> RateLimitLayer {
    // Sabit rate limit değerleri
    let rate = 100;
    let per = Duration::from_secs(1);
    RateLimitLayer::new(rate, per)
}

// Enhanced error handling for rate limits
impl StdError for RateLimitExceeded {}

impl fmt::Display for RateLimitExceeded {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Rate limit exceeded")
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::{Service, ServiceExt};

    #[tokio::test]
    async fn test_rate_limiting() {
        let config = RateLimitConfig {
            requests: NonZeroU32::new(2).unwrap(),
            per_seconds: 1,
        };

        let mut service = tower::ServiceBuilder::new()
            .layer(config.layer())
            .service(tower::service_fn(|_| async {
                Ok::<_, std::convert::Infallible>(Response::new(Body::empty()))
            }));


        let response = service
            .ready()
            .await
            .unwrap()
            .call(Request::new(Body::empty()))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);


        let response = service
            .ready()
            .await
            .unwrap()
            .call(Request::new(Body::empty()))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);


        let response = service
            .ready()
            .await
            .unwrap()
            .call(Request::new(Body::empty()))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    }
}