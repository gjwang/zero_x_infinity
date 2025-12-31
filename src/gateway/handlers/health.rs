//! Health check handler

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use axum::{Json, extract::State, http::StatusCode};
use utoipa::ToSchema;

use super::super::state::AppState;
use super::super::types::ApiResponse;

/// Health check response data
#[derive(serde::Serialize, ToSchema)]
pub struct HealthResponse {
    /// Server timestamp in milliseconds
    #[schema(example = 1703494800000_u64)]
    pub timestamp_ms: u64,
}

/// Health check endpoint
///
/// Returns service health status with server timestamp.
/// Internally checks all dependencies (TDengine, etc.) but does NOT
/// expose any internal details in the response.
///
/// - Healthy: 200 OK + {code: 0, data: {timestamp_ms}}
/// - Unhealthy: 503 Service Unavailable + {code: 503, msg: "unavailable"}
#[utoipa::path(
    get,
    path = "/api/v1/health",
    responses(
        (status = 200, description = "Service healthy", body = HealthResponse, content_type = "application/json"),
        (status = 503, description = "Service unavailable")
    ),
    tag = "System"
)]
pub async fn health_check(
    State(state): State<Arc<AppState>>,
) -> (StatusCode, Json<ApiResponse<HealthResponse>>) {
    use taos::AsyncQueryable;

    // Rate limit: only ping DB once per interval
    static LAST_CHECK_MS: AtomicU64 = AtomicU64::new(0);
    const CHECK_INTERVAL_MS: u64 = 5000; // 5 seconds

    // Get current timestamp in milliseconds
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);

    // Check if we need to do actual DB ping (rate limited)
    let last_check = LAST_CHECK_MS.load(Ordering::Relaxed);
    let all_healthy = if now_ms - last_check > CHECK_INTERVAL_MS {
        // Interval expired, do actual DB check
        LAST_CHECK_MS.store(now_ms, Ordering::Relaxed);
        if let Some(ref db_client) = state.db_client {
            match db_client.taos().exec("SELECT 1").await {
                Ok(_) => true,
                Err(e) => {
                    tracing::error!("[HEALTH] TDengine ping failed: {}", e);
                    false
                }
            }
        } else {
            tracing::error!("[HEALTH] No db_client configured");
            false
        }
    } else {
        true // Within interval, assume healthy
    };

    if all_healthy {
        (
            StatusCode::OK,
            Json(ApiResponse::success(HealthResponse {
                timestamp_ms: now_ms,
            })),
        )
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiResponse {
                code: 503,
                msg: "unavailable".to_string(),
                data: None,
            }),
        )
    }
}
