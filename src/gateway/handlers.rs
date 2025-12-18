use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::pipeline::{OrderAction, SequencedOrder};

use super::state::AppState;
use super::types::{CancelOrderRequest, ClientOrder, ErrorResponse, OrderResponse};

/// POST /api/v1/create_order
pub async fn create_order(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<ClientOrder>,
) -> Result<(StatusCode, Json<OrderResponse>), (StatusCode, Json<ErrorResponse>)> {
    // 1. Extract user_id
    let user_id = extract_user_id(&headers)?;

    // 2. Validate and parse ClientOrder
    let validated = super::types::validate_client_order(req, &state.symbol_mgr).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse::new("INVALID_PARAMETER", e)),
        )
    })?;

    // 3. Generate order_id and timestamp
    let order_id = state.next_order_id();
    let now = now_ns();

    // 4. Convert to InternalOrder
    let cid = validated.cid.clone();
    let order = validated
        .into_internal_order(order_id, user_id, now)
        .map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse::new("INVALID_PARAMETER", e)),
            )
        })?;

    // 5. Construct OrderAction
    let action = OrderAction::Place(SequencedOrder::new(order_id, order, now));

    // 6. Push to queue
    state.order_queue.push(action).map_err(|_| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse::new(
                "SERVICE_UNAVAILABLE",
                "Order queue is full, please try again later",
            )),
        )
    })?;

    // 7. Return response
    Ok((
        StatusCode::ACCEPTED,
        Json(OrderResponse {
            order_id,
            cid,
            status: "ACCEPTED".to_string(),
            accepted_at: now_ms(),
        }),
    ))
}

/// POST /api/v1/cancel_order
pub async fn cancel_order(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<CancelOrderRequest>,
) -> Result<(StatusCode, Json<OrderResponse>), (StatusCode, Json<ErrorResponse>)> {
    // 1. 提取 user_id
    let user_id = extract_user_id(&headers)?;

    // 2. 构造 OrderAction::Cancel
    let action = OrderAction::Cancel {
        order_id: req.order_id,
        user_id,
        ingested_at_ns: now_ns(),
    };

    // 3. 推送到队列
    state.order_queue.push(action).map_err(|_| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse::new(
                "SERVICE_UNAVAILABLE",
                "Order queue is full, please try again later",
            )),
        )
    })?;

    // 4. 返回响应
    Ok((
        StatusCode::OK,
        Json(OrderResponse {
            order_id: req.order_id,
            cid: None,
            status: "CANCEL_PENDING".to_string(),
            accepted_at: now_ms(),
        }),
    ))
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Extract user_id from HTTP headers
fn extract_user_id(headers: &HeaderMap) -> Result<u64, (StatusCode, Json<ErrorResponse>)> {
    let user_id_str = headers
        .get("X-User-ID")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                Json(ErrorResponse::new(
                    "UNAUTHORIZED",
                    "Missing X-User-ID header",
                )),
            )
        })?;

    user_id_str.parse::<u64>().map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse::new(
                "INVALID_PARAMETER",
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
