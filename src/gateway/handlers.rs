use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::pipeline::OrderAction;

use super::state::AppState;
use super::types::{ApiResponse, CancelOrderRequest, ClientOrder, OrderResponseData, error_codes};

/// Create order endpoint
///
/// POST /api/v1/create_order
pub async fn create_order(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<ClientOrder>,
) -> Result<(StatusCode, Json<ApiResponse<OrderResponseData>>), (StatusCode, Json<ApiResponse<()>>)>
{
    // 1. Extract user_id
    let user_id = extract_user_id(&headers)?;

    // 2. Validate and parse ClientOrder
    let validated =
        super::types::validate_client_order(req.clone(), &state.symbol_mgr).map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<()>::error(error_codes::INVALID_PARAMETER, e)),
            )
        })?;

    // 3. Generate order_id and timestamp
    let order_id = state.next_order_id();
    let timestamp = now_ns();

    // 4. Convert to InternalOrder
    let internal_order = validated
        .into_internal_order(order_id, user_id, timestamp)
        .map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<()>::error(error_codes::INVALID_PARAMETER, e)),
            )
        })?;

    // 5. Push to queue
    let action = OrderAction::Place(crate::pipeline::SequencedOrder::new(
        order_id,
        internal_order,
        timestamp,
    ));
    if state.order_queue.push(action).is_err() {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiResponse::<()>::error(
                error_codes::SERVICE_UNAVAILABLE,
                "Order queue is full, please try again later",
            )),
        ));
    }

    // 6. Return response
    Ok((
        StatusCode::ACCEPTED,
        Json(ApiResponse::success(OrderResponseData {
            order_id,
            cid: req.cid,
            order_status: "ACCEPTED".to_string(),
            accepted_at: now_ms(),
        })),
    ))
}

/// Cancel order endpoint
///
/// POST /api/v1/cancel_order
pub async fn cancel_order(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<CancelOrderRequest>,
) -> Result<(StatusCode, Json<ApiResponse<OrderResponseData>>), (StatusCode, Json<ApiResponse<()>>)>
{
    // 1. Extract user_id
    let user_id = extract_user_id(&headers)?;

    // 2. Push cancel action to queue
    let action = OrderAction::Cancel {
        order_id: req.order_id,
        user_id,
        ingested_at_ns: now_ns(),
    };

    if state.order_queue.push(action).is_err() {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiResponse::<()>::error(
                error_codes::SERVICE_UNAVAILABLE,
                "Order queue is full, please try again later",
            )),
        ));
    }

    // 3. Return response
    Ok((
        StatusCode::OK,
        Json(ApiResponse::success(OrderResponseData {
            order_id: req.order_id,
            cid: None,
            order_status: "CANCEL_PENDING".to_string(),
            accepted_at: now_ms(),
        })),
    ))
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Extract user_id from HTTP headers
fn extract_user_id(headers: &HeaderMap) -> Result<u64, (StatusCode, Json<ApiResponse<()>>)> {
    let user_id_str = headers
        .get("X-User-ID")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                Json(ApiResponse::<()>::error(
                    error_codes::MISSING_AUTH,
                    "Missing X-User-ID header",
                )),
            )
        })?;

    user_id_str.parse::<u64>().map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<()>::error(
                error_codes::INVALID_PARAMETER,
                "Invalid X-User-ID format",
            )),
        )
    })
}

/// Get current time in nanoseconds
fn now_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64
}

/// Get current time in milliseconds
fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}
