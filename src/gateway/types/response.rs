//! API Response types and error codes
//!
//! - `ApiResponse<T>`: Unified response wrapper
//! - `ApiResult<T>`: Type alias for handler return types
//! - `ApiError`: Unified error type with IntoResponse
//! - `error_codes`: Standard error code constants

use axum::{Json, http::StatusCode, response::IntoResponse};
use serde::Serialize;
use utoipa::ToSchema;

// ============================================================================
// Unified API Response Format
// ============================================================================

/// Unified API response wrapper
///
/// All API responses follow this structure:
/// - code: 0 = success, non-zero = error code
/// - msg: short message description
/// - data: actual data (success) or null (error)
#[derive(Debug, Serialize, ToSchema)]
pub struct ApiResponse<T> {
    /// Response code: 0 for success, non-zero for errors
    #[schema(example = 0)]
    pub code: i32,
    /// Response message
    #[schema(example = "ok")]
    pub msg: String,
    /// Response data (only present when code == 0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
}

impl<T> ApiResponse<T> {
    /// Create success response
    pub fn success(data: T) -> Self {
        Self {
            code: 0,
            msg: "ok".to_string(),
            data: Some(data),
        }
    }

    /// Create error response
    pub fn error(code: i32, msg: impl Into<String>) -> ApiResponse<()> {
        ApiResponse {
            code,
            msg: msg.into(),
            data: None,
        }
    }
}

// ============================================================================
// ApiResult: DRY Type Alias for Handlers
// ============================================================================

/// Type alias for handler return types - reduces boilerplate
///
/// Before:
/// ```ignore
/// Result<(StatusCode, Json<ApiResponse<T>>), (StatusCode, Json<ApiResponse<()>>)>
/// ```
///
/// After:
/// ```ignore
/// ApiResult<T>
/// ```
pub type ApiResult<T> =
    Result<(StatusCode, Json<ApiResponse<T>>), (StatusCode, Json<ApiResponse<()>>)>;

/// Helper to create success response (200 OK)
#[inline]
pub fn ok<T: Serialize>(data: T) -> ApiResult<T> {
    Ok((StatusCode::OK, Json(ApiResponse::success(data))))
}

/// Helper to create accepted response (202 ACCEPTED)
#[inline]
pub fn accepted<T: Serialize>(data: T) -> ApiResult<T> {
    Ok((StatusCode::ACCEPTED, Json(ApiResponse::success(data))))
}

// ============================================================================
// ApiError: Unified Error Type
// ============================================================================

/// Unified API error type with automatic IntoResponse
///
/// Usage:
/// ```ignore
/// fn my_handler() -> Result<ApiResult<Data>, ApiError> {
///     Err(ApiError::bad_request("Invalid input"))?;
/// }
/// ```
#[derive(Debug)]
pub struct ApiError {
    pub status: StatusCode,
    pub code: i32,
    pub message: String,
}

impl ApiError {
    /// Create a new ApiError
    pub fn new(status: StatusCode, code: i32, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
        }
    }

    /// 400 Bad Request with INVALID_PARAMETER code
    pub fn bad_request(msg: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, error_codes::INVALID_PARAMETER, msg)
    }

    /// 404 Not Found with ORDER_NOT_FOUND code
    pub fn not_found(msg: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, error_codes::ORDER_NOT_FOUND, msg)
    }

    /// 401 Unauthorized with AUTH_FAILED code
    pub fn unauthorized(msg: impl Into<String>) -> Self {
        Self::new(StatusCode::UNAUTHORIZED, error_codes::AUTH_FAILED, msg)
    }

    /// 500 Internal Server Error
    pub fn internal(msg: impl Into<String>) -> Self {
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            error_codes::INTERNAL_ERROR,
            msg,
        )
    }

    /// 503 Service Unavailable (queue full, etc.)
    pub fn service_unavailable(msg: impl Into<String>) -> Self {
        Self::new(
            StatusCode::SERVICE_UNAVAILABLE,
            error_codes::SERVICE_UNAVAILABLE,
            msg,
        )
    }

    /// Database error (500)
    pub fn db_error(msg: impl Into<String>) -> Self {
        Self::internal(format!("Database error: {}", msg.into()))
    }

    /// Rate limited (429)
    pub fn rate_limited(msg: impl Into<String>) -> Self {
        Self::new(
            StatusCode::TOO_MANY_REQUESTS,
            error_codes::RATE_LIMITED,
            msg,
        )
    }

    /// Convert to handler error tuple
    pub fn into_err<T>(self) -> ApiResult<T> {
        Err((
            self.status,
            Json(ApiResponse::<()>::error(self.code, self.message)),
        ))
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let body = Json(ApiResponse::<()>::error(self.code, self.message));
        (self.status, body).into_response()
    }
}

/// Enable ? operator for ApiError in handler functions
impl From<ApiError> for (StatusCode, Json<ApiResponse<()>>) {
    fn from(err: ApiError) -> Self {
        (
            err.status,
            Json(ApiResponse::<()>::error(err.code, err.message)),
        )
    }
}

/// Convert OrderError to ApiError for seamless use in handlers
impl From<crate::gateway::services::OrderError> for ApiError {
    fn from(err: crate::gateway::services::OrderError) -> Self {
        match err {
            crate::gateway::services::OrderError::InvalidParameter(msg) => {
                ApiError::bad_request(msg)
            }
            crate::gateway::services::OrderError::QueueFull => {
                ApiError::service_unavailable("Order queue is full, please try again later")
            }
            crate::gateway::services::OrderError::Internal(msg) => ApiError::internal(msg),
        }
    }
}

// ============================================================================
// Response DTOs
// ============================================================================

/// Order response data
#[derive(Debug, Serialize)]
pub struct OrderResponseData {
    pub order_id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cid: Option<String>,
    pub order_status: String,
    pub accepted_at: u64,
}

/// Account response data (for legacy /account endpoint)
#[derive(Debug, Serialize, ToSchema)]
pub struct AccountResponseData {
    pub balances: Vec<crate::funding::transfer_service::BalanceInfo>,
}

/// Market depth API response data
#[derive(Debug, Serialize, ToSchema)]
pub struct DepthApiData {
    /// Trading symbol name
    #[schema(example = "BTC_USDT")]
    pub symbol: String,
    /// Bid levels [[price, qty], ...]
    #[schema(example = json!([["85000.00", "0.5"], ["84999.00", "1.2"]]))]
    pub bids: Vec<[String; 2]>,
    /// Ask levels [[price, qty], ...]
    #[schema(example = json!([["85001.00", "0.3"], ["85002.00", "0.8"]]))]
    pub asks: Vec<[String; 2]>,
    /// Last update sequence ID
    #[schema(example = 12345)]
    pub last_update_id: u64,
}

// ============================================================================
// Error Codes
// ============================================================================

/// Standard API error codes
pub mod error_codes {
    // Success
    pub const SUCCESS: i32 = 0;

    // Client errors (1xxx)
    pub const INVALID_PARAMETER: i32 = 1001;
    pub const INSUFFICIENT_BALANCE: i32 = 1002;
    pub const INVALID_PRICE_QTY: i32 = 1003;

    // Auth errors (2xxx)
    pub const MISSING_AUTH: i32 = 2001;
    pub const AUTH_FAILED: i32 = 2002;

    // Resource errors (4xxx)
    pub const ORDER_NOT_FOUND: i32 = 4001;
    pub const RATE_LIMITED: i32 = 4291;

    // Server errors (5xxx)
    pub const INTERNAL_ERROR: i32 = 5000;
    pub const SERVICE_UNAVAILABLE: i32 = 5001;
}
