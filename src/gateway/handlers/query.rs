//! Query handlers (orders, trades)

use std::sync::Arc;

use axum::{
    Extension,
    extract::{Path, Query, State},
};

use super::super::state::AppState;
use super::super::types::{ApiError, ApiResult, ok};

/// Get single order by ID
///
/// GET /api/v1/private/order/{order_id}
#[utoipa::path(
    get,
    path = "/api/v1/private/order/{order_id}",
    params(
        ("order_id" = u64, Path, description = "Order ID")
    ),
    responses(
        (status = 200, description = "Order details", content_type = "application/json"),
        (status = 404, description = "Order not found"),
        (status = 503, description = "Service unavailable")
    ),
    security(("ed25519_auth" = [])),
    tag = "Account"
)]
pub async fn get_order(
    State(state): State<Arc<AppState>>,
    Path(order_id): Path<u64>,
) -> ApiResult<crate::persistence::queries::OrderApiData> {
    // Check if persistence is enabled
    let db_client = state
        .db_client
        .as_ref()
        .ok_or_else(|| ApiError::service_unavailable("Persistence not enabled"))?;

    // Query order from TDengine
    match crate::persistence::queries::query_order(
        db_client.taos(),
        order_id,
        state.active_symbol_id,
        &state.symbol_mgr,
    )
    .await
    {
        Ok(Some(order)) => ok(order),
        Ok(None) => ApiError::not_found("Order not found").into_err(),
        Err(e) => ApiError::db_error(format!("Query failed: {}", e)).into_err(),
    }
}

/// Get orders list
///
/// GET /api/v1/private/orders?limit=10
#[utoipa::path(
    get,
    path = "/api/v1/private/orders",
    params(
        ("limit" = Option<u32>, Query, description = "Number of orders (default: 10)")
    ),
    responses(
        (status = 200, description = "List of user orders", content_type = "application/json"),
        (status = 503, description = "Service unavailable")
    ),
    security(("ed25519_auth" = [])),
    tag = "Account"
)]
pub async fn get_orders(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<crate::api_auth::AuthenticatedUser>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> ApiResult<Vec<crate::persistence::queries::OrderApiData>> {
    // Check if persistence is enabled
    let db_client = state
        .db_client
        .as_ref()
        .ok_or_else(|| ApiError::service_unavailable("Persistence not enabled"))?;

    // 1. Use user_id from authenticated user
    let user_id = user.user_id as u64;

    let limit: usize = params
        .get("limit")
        .and_then(|s| s.parse().ok())
        .unwrap_or(10); // SAFE_DEFAULT: API documented default limit

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
        Ok(orders) => ok(orders),
        Err(e) => ApiError::db_error(format!("Query failed: {}", e)).into_err(),
    }
}

/// Get trades list
///
/// GET /api/v1/private/trades?limit=100
#[utoipa::path(
    get,
    path = "/api/v1/private/trades",
    params(
        ("limit" = Option<u32>, Query, description = "Number of trades (default: 100)")
    ),
    responses(
        (status = 200, description = "List of trades", content_type = "application/json"),
        (status = 503, description = "Service unavailable")
    ),
    security(("ed25519_auth" = [])),
    tag = "Account"
)]
pub async fn get_trades(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<crate::api_auth::AuthenticatedUser>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> ApiResult<Vec<crate::persistence::queries::TradeApiData>> {
    // Check if persistence is enabled
    let db_client = state
        .db_client
        .as_ref()
        .ok_or_else(|| ApiError::service_unavailable("Persistence not enabled"))?;

    // SEC-004 FIX: Extract user_id from authenticated user
    let user_id = user.user_id as u64;

    // Parse query parameters
    let limit: usize = params
        .get("limit")
        .and_then(|s| s.parse().ok())
        .unwrap_or(100); // SAFE_DEFAULT: API documented default limit

    // Query user-specific trades from TDengine (SEC-004: filter by user_id)
    match crate::persistence::queries::query_user_trades(
        db_client.taos(),
        user_id,
        state.active_symbol_id,
        limit,
        &state.symbol_mgr,
    )
    .await
    {
        Ok(trades) => ok(trades),
        Err(e) => ApiError::db_error(format!("Query failed: {}", e)).into_err(),
    }
}

/// Get public trades list (for public market data)
///
/// GET /api/v1/public/trades?symbol=BTC_USDT&limit=500&fromId=12345
#[utoipa::path(
    get,
    path = "/api/v1/public/trades",
    params(
        ("symbol" = Option<String>, Query, description = "Trading pair (e.g., BTC_USDT)"),
        ("limit" = Option<u32>, Query, description = "Number of trades (default: 500, max: 1000)"),
        ("fromId" = Option<i64>, Query, description = "Fetch trades with ID > fromId (pagination)")
    ),
    responses(
        (status = 200, description = "List of public trades", content_type = "application/json"),
        (status = 400, description = "Invalid parameters"),
        (status = 503, description = "Service unavailable")
    ),
    tag = "Market Data"
)]
pub async fn get_public_trades(
    State(state): State<Arc<AppState>>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> ApiResult<Vec<crate::persistence::queries::PublicTradeApiData>> {
    // Check if persistence is enabled
    let db_client = state
        .db_client
        .as_ref()
        .ok_or_else(|| ApiError::service_unavailable("Persistence not enabled"))?;

    // Parse query parameters
    let limit: usize = params
        .get("limit")
        .and_then(|s| s.parse().ok())
        .unwrap_or(500) // SAFE_DEFAULT: API documented default limit
        .min(1000); // Cap at 1000

    let from_id: Option<i64> = params.get("fromId").and_then(|s| s.parse().ok());

    // Query public trades from TDengine
    match crate::persistence::queries::query_public_trades(
        db_client.taos(),
        state.active_symbol_id,
        limit,
        from_id,
        &state.symbol_mgr,
    )
    .await
    {
        Ok(trades) => ok(trades),
        Err(e) => ApiError::db_error(format!("Query failed: {}", e)).into_err(),
    }
}
