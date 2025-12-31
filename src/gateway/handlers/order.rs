//! Order-related handlers (create, cancel, reduce, move)

use std::sync::Arc;

use axum::{
    Extension, Json,
    extract::{Path, State},
    http::StatusCode,
};

use crate::pipeline::OrderAction;

use super::super::state::AppState;
use super::super::types::{
    ApiError, ApiResult, CancelOrderRequest, ClientOrder, MoveOrderRequest, OrderResponseData,
    ReduceOrderRequest, accepted, decimal_to_u64, error_codes, ok,
};
use super::helpers::{now_ms, now_ns};

/// Create order endpoint
///
/// POST /api/v1/private/order
#[utoipa::path(
    post,
    path = "/api/v1/private/order",
    request_body(content = String, description = "Order request JSON", content_type = "application/json"),
    responses(
        (status = 202, description = "Order accepted", content_type = "application/json"),
        (status = 400, description = "Invalid parameters"),
        (status = 401, description = "Authentication failed"),
        (status = 503, description = "Service unavailable")
    ),
    security(("ed25519_auth" = [])),
    tag = "Trading"
)]
pub async fn create_order(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<crate::api_auth::AuthenticatedUser>,
    Json(req): Json<ClientOrder>,
) -> ApiResult<OrderResponseData> {
    // 1. Extract user_id from authenticated user
    let user_id = user.user_id as u64;
    tracing::info!("[TRACE] Create Order: Received from User {}", user_id);
    tracing::info!("[TRACE] Request Details: {:?}", req);

    // 2. Validate and parse ClientOrder
    let validated = super::super::types::validate_client_order(req.clone(), &state.symbol_mgr)
        .map_err(ApiError::bad_request)?;

    // 3. Generate order_id and timestamp
    let order_id = state.next_order_id();
    let timestamp = now_ns();

    // 4. Convert to InternalOrder
    let internal_order = validated
        .into_internal_order(order_id, user_id, timestamp)
        .map_err(ApiError::bad_request)?;

    // 5. Push to queue
    tracing::info!(
        "[TRACE] Create Order {}: Pushing to Ingestion Queue",
        order_id
    );
    let action = OrderAction::Place(crate::pipeline::SequencedOrder::new(
        order_id,
        internal_order,
        timestamp,
    ));
    if state.order_queue.push(action).is_err() {
        return ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            error_codes::SERVICE_UNAVAILABLE,
            "Order queue is full, please try again later",
        )
        .into_err();
    }
    tracing::info!(
        "[TRACE] Create Order {}: ✅ Pushed to Ingestion Queue",
        order_id
    );

    // 6. Return response
    accepted(OrderResponseData {
        order_id,
        cid: req.cid,
        order_status: "ACCEPTED".to_string(),
        accepted_at: now_ms(),
    })
}

/// Cancel order endpoint
///
/// POST /api/v1/private/cancel
#[utoipa::path(
    post,
    path = "/api/v1/private/cancel",
    request_body(content = String, description = "Cancel request with order_id", content_type = "application/json"),
    responses(
        (status = 200, description = "Cancel request accepted", content_type = "application/json"),
        (status = 400, description = "Invalid order ID"),
        (status = 401, description = "Authentication failed"),
        (status = 503, description = "Service unavailable")
    ),
    security(("ed25519_auth" = [])),
    tag = "Trading"
)]
pub async fn cancel_order(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<crate::api_auth::AuthenticatedUser>,
    Json(req): Json<CancelOrderRequest>,
) -> ApiResult<OrderResponseData> {
    // 1. Extract user_id from authenticated user
    let user_id = user.user_id as u64;
    tracing::info!(
        "[TRACE] Cancel Order {}: Received from User {}",
        req.order_id,
        user_id
    );

    // 2. Push cancel action to queue
    let action = OrderAction::Cancel {
        order_id: req.order_id,
        user_id,
        ingested_at_ns: now_ns(),
    };

    if state.order_queue.push(action).is_err() {
        tracing::error!(
            "[TRACE] Cancel Order {}: ❌ Queue Full in Gateway",
            req.order_id
        );
        return ApiError::service_unavailable("Order queue is full, please try again later")
            .into_err();
    }

    tracing::info!(
        "[TRACE] Cancel Order {}: ✅ Gateway -> Ingestion Queue (User {})",
        req.order_id,
        user_id
    );

    // 3. Return response
    ok(OrderResponseData {
        order_id: req.order_id,
        cid: None,
        order_status: "CANCEL_PENDING".to_string(),
        accepted_at: now_ms(),
    })
}

/// Reduce order quantity
#[utoipa::path(
    post,
    path = "/api/v1/private/order/reduce",
    request_body(content = ReduceOrderRequest, description = "Reduce order quantity", content_type = "application/json"),
    responses(
        (status = 202, description = "Reduce request accepted", content_type = "application/json"),
        (status = 400, description = "Invalid parameters"),
        (status = 401, description = "Authentication failed"),
        (status = 503, description = "Service unavailable")
    ),
    security(("ed25519_auth" = [])),
    tag = "Trading"
)]
pub async fn reduce_order(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<crate::api_auth::AuthenticatedUser>,
    Json(req): Json<ReduceOrderRequest>,
) -> ApiResult<OrderResponseData> {
    let user_id = user.user_id as u64;
    let symbol_info = state
        .symbol_mgr
        .get_symbol_info_by_id(state.active_symbol_id)
        .ok_or_else(|| ApiError::internal("Active symbol not found"))?;

    let reduce_qty_u64 = decimal_to_u64(req.reduce_qty.inner(), symbol_info.base_decimals)
        .map_err(ApiError::bad_request)?;

    tracing::info!(
        "[TRACE] Reduce Order {}: Received from User {}",
        req.order_id,
        user_id
    );

    let action = OrderAction::Reduce {
        order_id: req.order_id,
        user_id,
        reduce_qty: reduce_qty_u64,
        ingested_at_ns: now_ns(),
    };

    if state.order_queue.push(action).is_err() {
        return ApiError::service_unavailable("Order queue is full").into_err();
    }

    accepted(OrderResponseData {
        order_id: req.order_id,
        cid: None,
        order_status: "ACCEPTED".to_string(),
        accepted_at: now_ms(),
    })
}

/// Move order price
#[utoipa::path(
    post,
    path = "/api/v1/private/order/move",
    request_body(content = MoveOrderRequest, description = "Move order to new price", content_type = "application/json"),
    responses(
        (status = 202, description = "Move request accepted", content_type = "application/json"),
        (status = 400, description = "Invalid parameters"),
        (status = 401, description = "Authentication failed"),
        (status = 503, description = "Service unavailable")
    ),
    security(("ed25519_auth" = [])),
    tag = "Trading"
)]
pub async fn move_order(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<crate::api_auth::AuthenticatedUser>,
    Json(req): Json<MoveOrderRequest>,
) -> ApiResult<OrderResponseData> {
    let user_id = user.user_id as u64;
    let symbol_info = state
        .symbol_mgr
        .get_symbol_info_by_id(state.active_symbol_id)
        .ok_or_else(|| ApiError::internal("Active symbol not found"))?;

    let new_price_u64 = decimal_to_u64(req.new_price.inner(), symbol_info.price_decimal)
        .map_err(ApiError::bad_request)?;

    tracing::info!(
        "[TRACE] Move Order {}: Received from User {}",
        req.order_id,
        user_id
    );

    let action = OrderAction::Move {
        order_id: req.order_id,
        user_id,
        new_price: new_price_u64,
        ingested_at_ns: now_ns(),
    };

    if state.order_queue.push(action).is_err() {
        return ApiError::service_unavailable("Order queue is full").into_err();
    }

    accepted(OrderResponseData {
        order_id: req.order_id,
        cid: None,
        order_status: "ACCEPTED".to_string(),
        accepted_at: now_ms(),
    })
}

/// Cancel order by ID (DELETE method)
pub async fn cancel_order_by_id(
    state: State<Arc<AppState>>,
    user: Extension<crate::api_auth::AuthenticatedUser>,
    Path(order_id): Path<u64>,
) -> ApiResult<OrderResponseData> {
    cancel_order(state, user, Json(CancelOrderRequest { order_id })).await
}
