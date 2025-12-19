use axum::{
    Json,
    extract::{Path, Query, State},
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
    tracing::info!("[TRACE] Create Order: Received from User {}", user_id);

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
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiResponse::<()>::error(
                error_codes::SERVICE_UNAVAILABLE,
                "Order queue is full, please try again later",
            )),
        ));
    }
    tracing::info!(
        "[TRACE] Create Order {}: ✅ Pushed to Ingestion Queue",
        order_id
    );

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
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiResponse::<()>::error(
                error_codes::SERVICE_UNAVAILABLE,
                "Order queue is full, please try again later",
            )),
        ));
    }

    tracing::info!(
        "[TRACE] Cancel Order {}: ✅ Gateway -> Ingestion Queue (User {})",
        req.order_id,
        user_id
    );

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

// ============================================================================
// Query Endpoints (Placeholder implementations)
// ============================================================================

/// Get single order by ID
///
/// GET /api/v1/order/:order_id
pub async fn get_order(
    State(state): State<Arc<AppState>>,
    Path(order_id): Path<u64>,
) -> Result<
    (
        StatusCode,
        Json<ApiResponse<crate::persistence::queries::OrderApiData>>,
    ),
    (StatusCode, Json<ApiResponse<()>>),
> {
    // Check if persistence is enabled
    let db_client = state.db_client.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiResponse::<()>::error(
                error_codes::SERVICE_UNAVAILABLE,
                "Persistence not enabled",
            )),
        )
    })?;

    // Query order from TDengine
    match crate::persistence::queries::query_order(
        db_client.taos(),
        order_id,
        state.active_symbol_id,
        &state.symbol_mgr,
    )
    .await
    {
        Ok(Some(order)) => Ok((StatusCode::OK, Json(ApiResponse::success(order)))),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<()>::error(
                error_codes::ORDER_NOT_FOUND,
                "Order not found",
            )),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<()>::error(
                error_codes::SERVICE_UNAVAILABLE,
                &format!("Query failed: {}", e),
            )),
        )),
    }
}

/// Get orders list
///
/// GET /api/v1/orders?user_id=1001&limit=10
pub async fn get_orders(
    State(state): State<Arc<AppState>>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Result<
    (
        StatusCode,
        Json<ApiResponse<Vec<crate::persistence::queries::OrderApiData>>>,
    ),
    (StatusCode, Json<ApiResponse<()>>),
> {
    // Check if persistence is enabled
    let db_client = state.db_client.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiResponse::<()>::error(
                error_codes::SERVICE_UNAVAILABLE,
                "Persistence not enabled",
            )),
        )
    })?;

    // Parse query parameters
    let user_id: u64 = params
        .get("user_id")
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<()>::error(
                    error_codes::INVALID_PARAMETER,
                    "Missing or invalid user_id parameter",
                )),
            )
        })?;

    let limit: usize = params
        .get("limit")
        .and_then(|s| s.parse().ok())
        .unwrap_or(10);

    // Query orders from TDengine
    match crate::persistence::queries::query_orders(
        db_client.taos(),
        user_id,
        state.active_symbol_id,
        limit,
        &state.symbol_mgr,
    )
    .await
    {
        Ok(orders) => Ok((StatusCode::OK, Json(ApiResponse::success(orders)))),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<()>::error(
                error_codes::SERVICE_UNAVAILABLE,
                &format!("Query failed: {}", e),
            )),
        )),
    }
}

/// Get trades list
///
/// GET /api/v1/trades?limit=100
pub async fn get_trades(
    State(state): State<Arc<AppState>>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Result<
    (
        StatusCode,
        Json<ApiResponse<Vec<crate::persistence::queries::TradeApiData>>>,
    ),
    (StatusCode, Json<ApiResponse<()>>),
> {
    // Check if persistence is enabled
    let db_client = state.db_client.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiResponse::<()>::error(
                error_codes::SERVICE_UNAVAILABLE,
                "Persistence not enabled",
            )),
        )
    })?;

    // Parse query parameters
    let limit: usize = params
        .get("limit")
        .and_then(|s| s.parse().ok())
        .unwrap_or(100);

    // Query trades from TDengine
    match crate::persistence::queries::query_trades(
        db_client.taos(),
        state.active_symbol_id,
        limit,
        &state.symbol_mgr,
    )
    .await
    {
        Ok(trades) => Ok((StatusCode::OK, Json(ApiResponse::success(trades)))),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<()>::error(
                error_codes::SERVICE_UNAVAILABLE,
                &format!("Query failed: {}", e),
            )),
        )),
    }
}

/// Get user balance
///
/// GET /api/v1/balances?user_id=1001&asset_id=1
pub async fn get_balances(
    State(state): State<Arc<AppState>>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Result<
    (
        StatusCode,
        Json<ApiResponse<crate::persistence::queries::BalanceApiData>>,
    ),
    (StatusCode, Json<ApiResponse<()>>),
> {
    // Check if persistence is enabled
    let db_client = state.db_client.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiResponse::<()>::error(
                error_codes::SERVICE_UNAVAILABLE,
                "Persistence not enabled",
            )),
        )
    })?;

    // Parse query parameters
    let user_id: u64 = params
        .get("user_id")
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<()>::error(
                    error_codes::INVALID_PARAMETER,
                    "Missing or invalid user_id parameter",
                )),
            )
        })?;

    let asset_id: u32 = params
        .get("asset_id")
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<()>::error(
                    error_codes::INVALID_PARAMETER,
                    "Missing or invalid asset_id parameter",
                )),
            )
        })?;

    // Query balance from TDengine
    match crate::persistence::queries::query_balance(
        db_client.taos(),
        user_id,
        asset_id,
        &state.symbol_mgr,
    )
    .await
    {
        Ok(Some(balance)) => Ok((StatusCode::OK, Json(ApiResponse::success(balance)))),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(ApiResponse::<()>::error(
                error_codes::ORDER_NOT_FOUND,
                "Balance not found",
            )),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<()>::error(
                error_codes::SERVICE_UNAVAILABLE,
                &format!("Query failed: {}", e),
            )),
        )),
    }
}

/// Get K-Line data
///
/// GET /api/v1/klines?interval=1m&limit=100
pub async fn get_klines(
    State(state): State<Arc<AppState>>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Result<
    (
        StatusCode,
        Json<ApiResponse<Vec<crate::persistence::queries::KLineApiData>>>,
    ),
    (StatusCode, Json<ApiResponse<()>>),
> {
    // Check if persistence is enabled
    let db_client = state.db_client.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiResponse::<()>::error(
                error_codes::SERVICE_UNAVAILABLE,
                "Persistence not enabled",
            )),
        )
    })?;

    // Parse query parameters
    let interval = params.get("interval").map(|s| s.as_str()).unwrap_or("1m");

    // Validate interval
    let valid_intervals = ["1m", "5m", "15m", "30m", "1h", "1d"];
    if !valid_intervals.contains(&interval) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<()>::error(
                error_codes::INVALID_PARAMETER,
                "Invalid interval. Valid values: 1m, 5m, 15m, 30m, 1h, 1d",
            )),
        ));
    }

    let limit: usize = params
        .get("limit")
        .and_then(|s| s.parse().ok())
        .unwrap_or(100)
        .min(1000); // Cap at 1000

    // Query K-Lines from TDengine
    match crate::persistence::queries::query_klines(
        db_client.taos(),
        state.active_symbol_id,
        interval,
        limit,
        &state.symbol_mgr,
    )
    .await
    {
        Ok(klines) => Ok((StatusCode::OK, Json(ApiResponse::success(klines)))),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<()>::error(
                error_codes::SERVICE_UNAVAILABLE,
                &format!("Query failed: {}", e),
            )),
        )),
    }
}
