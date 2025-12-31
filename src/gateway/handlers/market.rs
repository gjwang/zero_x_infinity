//! Market data handlers (klines, depth)

use std::sync::Arc;

use axum::extract::{Query, State};

use super::super::state::AppState;
use super::super::types::{ApiError, ApiResult, DepthApiData, ok};
use super::helpers::DepthFormatter;

/// Get K-Line data
///
/// GET /api/v1/klines?interval=1m&limit=100
#[utoipa::path(
    get,
    path = "/api/v1/public/klines",
    params(
        ("interval" = Option<String>, Query, description = "K-line interval: 1m, 5m, 15m, 30m, 1h, 1d"),
        ("limit" = Option<u32>, Query, description = "Number of K-lines (default: 100, max: 1000)")
    ),
    responses(
        (status = 200, description = "K-line candlestick data", content_type = "application/json"),
        (status = 400, description = "Invalid interval parameter"),
        (status = 503, description = "Service unavailable")
    ),
    tag = "Market Data"
)]
pub async fn get_klines(
    State(state): State<Arc<AppState>>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> ApiResult<Vec<crate::persistence::queries::KLineApiData>> {
    // Check if persistence is enabled
    let db_client = state
        .db_client
        .as_ref()
        .ok_or_else(|| ApiError::service_unavailable("Persistence not enabled"))?;

    // Parse query parameters
    let interval = params.get("interval").map(|s| s.as_str()).unwrap_or("1m");

    // Validate interval
    let valid_intervals = ["1m", "5m", "15m", "30m", "1h", "1d"];
    if !valid_intervals.contains(&interval) {
        return ApiError::bad_request("Invalid interval. Valid values: 1m, 5m, 15m, 30m, 1h, 1d")
            .into_err();
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
        Ok(klines) => ok(klines),
        Err(e) => ApiError::db_error(format!("Query failed: {}", e)).into_err(),
    }
}

/// Get order book depth
///
/// GET /api/v1/depth?symbol=BTC_USDT&limit=20
#[utoipa::path(
    get,
    path = "/api/v1/public/depth",
    params(
        ("symbol" = Option<String>, Query, description = "Trading pair (e.g., BTC_USDT)"),
        ("limit" = Option<u32>, Query, description = "Depth levels (default: 20, max: 100)")
    ),
    responses(
        (status = 200, description = "Order book depth", body = DepthApiData, content_type = "application/json"),
        (status = 503, description = "Service unavailable")
    ),
    tag = "Market Data"
)]
pub async fn get_depth(
    State(state): State<Arc<AppState>>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> ApiResult<DepthApiData> {
    // Parse limit (default 20, max 100)
    let limit = params
        .get("limit")
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(20)
        .min(100);

    // Get snapshot from DepthService (not OrderBook directly!)
    let snapshot = state.depth_service.get_snapshot(limit);

    // Get symbol name
    let symbol_name = state
        .symbol_mgr
        .get_symbol(state.active_symbol_id)
        .ok_or_else(|| ApiError::internal("Symbol not found"))?;

    // Use DepthFormatter for type-safe formatting
    let formatter = DepthFormatter::new(&state.symbol_mgr);

    let (formatted_bids, formatted_asks) = formatter
        .format_depth_data(&snapshot.bids, &snapshot.asks, state.active_symbol_id)
        .map_err(ApiError::internal)?;

    let data = DepthApiData {
        symbol: symbol_name.to_string(),
        bids: formatted_bids,
        asks: formatted_asks,
        last_update_id: snapshot.update_id,
    };

    ok(data)
}
