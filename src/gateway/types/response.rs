//! API Response types and error codes
//!
//! - `ApiResponse<T>`: Unified response wrapper
//! - `error_codes`: Standard error code constants
//! - Various response DTOs

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
    pub balances: Vec<crate::funding::service::BalanceInfo>,
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
