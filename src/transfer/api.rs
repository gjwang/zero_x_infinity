//! Transfer API Layer
//!
//! HTTP handlers for internal transfer operations.
//! Implements Defense-in-Depth with validation at API layer.

use axum::http::StatusCode;
use serde::{Deserialize, Serialize};

use super::coordinator::TransferCoordinator;
use super::error::TransferError;
use super::state::TransferState;
use super::types::{RequestId, ServiceId, TransferRequest as CoreTransferRequest};

// ============================================================================
// API Request/Response Types
// ============================================================================

/// API request for creating a transfer
#[derive(Debug, Deserialize)]
pub struct TransferApiRequest {
    /// Source account type: "funding" or "spot"
    pub from: String,
    /// Target account type: "funding" or "spot"
    pub to: String,
    /// Asset symbol (e.g., "BTC", "USDT")
    pub asset: String,
    /// Amount as string (to avoid float precision issues)
    pub amount: String,
    /// Optional client idempotency key
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cid: Option<String>,
}

/// API response for transfer operations
#[derive(Debug, Serialize)]
pub struct TransferApiResponse {
    /// Unique request ID (Snowflake)
    pub req_id: String,
    /// Current transfer state
    pub status: String,
    /// Source account type
    pub from: String,
    /// Target account type
    pub to: String,
    /// Asset symbol
    pub asset: String,
    /// Amount as string
    pub amount: String,
    /// Timestamp (milliseconds)
    pub timestamp: i64,
    /// Client idempotency key (if provided)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cid: Option<String>,
}

/// API wrapper for standard response format
#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub code: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub msg: Option<String>,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            code: 0,
            data: Some(data),
            msg: None,
        }
    }

    pub fn error(code: i32, msg: impl ToString) -> Self {
        Self {
            code,
            data: None,
            msg: Some(msg.to_string()),
        }
    }
}

// ============================================================================
// Error Codes (matching design doc)
// ============================================================================

pub mod error_codes {
    pub const INVALID_PARAMETER: i32 = -1001;
    pub const INVALID_AMOUNT: i32 = -1002;
    pub const ASSET_NOT_FOUND: i32 = -1003;
    pub const SAME_ACCOUNT: i32 = -1004;
    pub const INSUFFICIENT_BALANCE: i32 = -2001;
    pub const ACCOUNT_NOT_FOUND: i32 = -2002;
    pub const DUPLICATE_REQUEST: i32 = -3001;
    pub const UNAUTHORIZED: i32 = -4001;
    pub const FORBIDDEN: i32 = -4003;
    pub const SERVICE_UNAVAILABLE: i32 = -5001;
    pub const TRANSFER_NOT_FOUND: i32 = -6001;
    pub const TRANSFER_FAILED: i32 = -6002;
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Parse account type from string
fn parse_account_type(s: &str) -> Result<ServiceId, TransferError> {
    match s.to_lowercase().as_str() {
        "funding" | "main" => Ok(ServiceId::Funding),
        "spot" | "trading" => Ok(ServiceId::Trading),
        _ => Err(TransferError::InvalidAccountType(format!(
            "Invalid account type: {}. Use 'funding' or 'spot'",
            s
        ))),
    }
}

/// Parse amount from string to u64 with decimals
fn parse_amount(s: &str, decimals: u32) -> Result<u64, TransferError> {
    // Remove any whitespace
    let s = s.trim();

    if s.is_empty() {
        return Err(TransferError::InvalidAmount);
    }

    // Parse as decimal
    let parts: Vec<&str> = s.split('.').collect();

    let (whole, frac) = match parts.len() {
        1 => (parts[0], ""),
        2 => (parts[0], parts[1]),
        _ => return Err(TransferError::InvalidAmount),
    };

    // Parse whole part
    let whole_num: u64 = whole.parse().map_err(|_| TransferError::InvalidAmount)?;

    // Parse fractional part (pad or truncate to decimals)
    let frac_str = format!("{:0<width$}", frac, width = decimals as usize);
    let frac_num: u64 = frac_str[..decimals as usize]
        .parse()
        .map_err(|_| TransferError::InvalidAmount)?;

    // Combine
    let multiplier = 10u64.pow(decimals);
    let amount = whole_num
        .checked_mul(multiplier)
        .and_then(|v| v.checked_add(frac_num))
        .ok_or(TransferError::InvalidAmount)?;

    if amount == 0 {
        return Err(TransferError::InvalidAmount);
    }

    Ok(amount)
}

/// Format amount from u64 to string with decimals
fn format_amount(amount: u64, decimals: u32) -> String {
    let divisor = 10u64.pow(decimals);
    let whole = amount / divisor;
    let frac = amount % divisor;
    format!("{}.{:0>width$}", whole, frac, width = decimals as usize)
}

/// Map TransferError to (StatusCode, error_code, message)
fn map_error(e: &TransferError) -> (StatusCode, i32, String) {
    let status = match e.http_status() {
        400 => StatusCode::BAD_REQUEST,
        401 => StatusCode::UNAUTHORIZED,
        403 => StatusCode::FORBIDDEN,
        404 => StatusCode::NOT_FOUND,
        409 => StatusCode::CONFLICT,
        422 => StatusCode::UNPROCESSABLE_ENTITY,
        503 => StatusCode::SERVICE_UNAVAILABLE,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    };

    // Map error code to numeric code
    let code = match e.code() {
        "UNAUTHORIZED" => error_codes::UNAUTHORIZED,
        "FORBIDDEN" => error_codes::FORBIDDEN,
        "SAME_ACCOUNT" => error_codes::SAME_ACCOUNT,
        "INVALID_AMOUNT" | "PRECISION_OVERFLOW" | "AMOUNT_TOO_SMALL" | "AMOUNT_TOO_LARGE" => {
            error_codes::INVALID_AMOUNT
        }
        "INVALID_ASSET" => error_codes::ASSET_NOT_FOUND,
        "INSUFFICIENT_BALANCE" => error_codes::INSUFFICIENT_BALANCE,
        "SOURCE_ACCOUNT_NOT_FOUND" | "TARGET_ACCOUNT_NOT_FOUND" => error_codes::ACCOUNT_NOT_FOUND,
        "DUPLICATE_REQUEST" => error_codes::DUPLICATE_REQUEST,
        "TRANSFER_NOT_FOUND" => error_codes::TRANSFER_NOT_FOUND,
        _ => error_codes::INVALID_PARAMETER,
    };

    let msg = e.to_string();

    (status, code, msg)
}

// ============================================================================
// Handler (for integration with existing Gateway)
// ============================================================================

/// Create transfer using FSM coordinator
///
/// This is a standalone function that can be called from the existing Gateway handler.
/// It provides the Defense-in-Depth API validation layer.
pub async fn create_transfer_fsm(
    coordinator: &TransferCoordinator,
    user_id: u64,
    req: TransferApiRequest,
    asset_id: u32,
    asset_decimals: u32,
) -> Result<TransferApiResponse, (StatusCode, ApiResponse<()>)> {
    // === Defense-in-Depth Layer 1: API Validation ===

    // 1. Parse source and target
    let from = parse_account_type(&req.from).map_err(|e| {
        let (status, code, msg) = map_error(&e);
        (status, ApiResponse::error(code, msg))
    })?;

    let to = parse_account_type(&req.to).map_err(|e| {
        let (status, code, msg) = map_error(&e);
        (status, ApiResponse::error(code, msg))
    })?;

    // 2. Same account check
    if from == to {
        return Err((
            StatusCode::BAD_REQUEST,
            ApiResponse::error(
                error_codes::SAME_ACCOUNT,
                "Cannot transfer to the same account",
            ),
        ));
    }

    // 3. Parse amount
    let amount = parse_amount(&req.amount, asset_decimals).map_err(|e| {
        let (status, code, msg) = map_error(&e);
        (status, ApiResponse::error(code, msg))
    })?;

    // 4. Create core request
    let mut core_req = CoreTransferRequest::new(from, to, user_id, asset_id, amount);
    core_req.cid = req.cid.clone();

    // 5. Submit to coordinator
    let req_id = coordinator.create(core_req).await.map_err(|e| {
        let (status, code, msg) = map_error(&e);
        (status, ApiResponse::error(code, msg))
    })?;

    // 6. Get initial state
    let state = coordinator
        .get_state(req_id)
        .await
        .map_err(|e| {
            let (status, code, msg) = map_error(&e);
            (status, ApiResponse::error(code, msg))
        })?
        .unwrap_or(TransferState::Init);

    // 7. Build response
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;

    Ok(TransferApiResponse {
        req_id: req_id.to_string(),
        status: state.to_string(),
        from: req.from,
        to: req.to,
        asset: req.asset,
        amount: format_amount(amount, asset_decimals),
        timestamp: now,
        cid: req.cid,
    })
}

/// Get transfer status
pub async fn get_transfer_status(
    coordinator: &TransferCoordinator,
    req_id: RequestId,
    asset_decimals: u32,
) -> Result<TransferApiResponse, (StatusCode, ApiResponse<()>)> {
    let record = coordinator
        .get(req_id)
        .await
        .map_err(|e| {
            let (status, code, msg) = map_error(&e);
            (status, ApiResponse::error(code, msg))
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                ApiResponse::error(error_codes::TRANSFER_NOT_FOUND, "Transfer not found"),
            )
        })?;

    Ok(TransferApiResponse {
        req_id: record.req_id.to_string(),
        status: record.state.to_string(),
        from: record.source.to_string().to_lowercase(),
        to: record.target.to_string().to_lowercase(),
        asset: "unknown".to_string(), // Would need asset lookup
        amount: format_amount(record.amount, asset_decimals),
        timestamp: record.updated_at,
        cid: record.cid,
    })
}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_account_type() {
        assert_eq!(parse_account_type("funding").unwrap(), ServiceId::Funding);
        assert_eq!(parse_account_type("FUNDING").unwrap(), ServiceId::Funding);
        assert_eq!(parse_account_type("main").unwrap(), ServiceId::Funding);
        assert_eq!(parse_account_type("spot").unwrap(), ServiceId::Trading);
        assert_eq!(parse_account_type("SPOT").unwrap(), ServiceId::Trading);
        assert_eq!(parse_account_type("trading").unwrap(), ServiceId::Trading);

        assert!(parse_account_type("invalid").is_err());
    }

    #[test]
    fn test_parse_amount() {
        // Normal cases
        assert_eq!(parse_amount("1.0", 8).unwrap(), 100_000_000);
        assert_eq!(parse_amount("0.5", 8).unwrap(), 50_000_000);
        assert_eq!(parse_amount("100", 8).unwrap(), 10_000_000_000);
        assert_eq!(parse_amount("0.00000001", 8).unwrap(), 1);

        // Edge cases
        assert_eq!(parse_amount("1", 8).unwrap(), 100_000_000);
        assert_eq!(parse_amount("0.1", 8).unwrap(), 10_000_000);

        // Invalid cases
        assert!(parse_amount("0", 8).is_err());
        assert!(parse_amount("", 8).is_err());
        assert!(parse_amount("-1", 8).is_err());
        assert!(parse_amount("abc", 8).is_err());
    }

    #[test]
    fn test_format_amount() {
        assert_eq!(format_amount(100_000_000, 8), "1.00000000");
        assert_eq!(format_amount(50_000_000, 8), "0.50000000");
        assert_eq!(format_amount(1, 8), "0.00000001");
        assert_eq!(format_amount(0, 8), "0.00000000");
    }
}
