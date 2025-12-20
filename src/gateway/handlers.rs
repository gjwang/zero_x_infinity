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

use crate::symbol_manager::SymbolManager;

// ============================================================================
// Depth Formatter - Encapsulated to prevent parameter errors
// ============================================================================

/// Depth data formatter that encapsulates conversion logic
///
/// This prevents parameter errors by internally fetching the correct
/// decimals and display_decimals from symbol_mgr.
pub struct DepthFormatter<'a> {
    symbol_mgr: &'a SymbolManager,
}

impl<'a> DepthFormatter<'a> {
    pub fn new(symbol_mgr: &'a SymbolManager) -> Self {
        Self { symbol_mgr }
    }

    /// Format quantity for a given symbol
    ///
    /// Automatically fetches base_asset.decimals and display_decimals
    /// from symbol_mgr, preventing parameter errors.
    pub fn format_qty(&self, value: u64, symbol_id: u32) -> Result<String, String> {
        let symbol = self
            .symbol_mgr
            .get_symbol_info_by_id(symbol_id)
            .ok_or_else(|| format!("Symbol {} not found", symbol_id))?;

        let asset = self
            .symbol_mgr
            .assets
            .get(&symbol.base_asset_id)
            .ok_or_else(|| format!("Asset {} not found", symbol.base_asset_id))?;

        Ok(format_qty_internal(
            value,
            asset.decimals,
            asset.display_decimals,
        ))
    }

    /// Format price for a given symbol
    pub fn format_price(&self, value: u64, symbol_id: u32) -> Result<String, String> {
        let symbol = self
            .symbol_mgr
            .get_symbol_info_by_id(symbol_id)
            .ok_or_else(|| format!("Symbol {} not found", symbol_id))?;

        Ok(format_price_internal(value, symbol.price_display_decimal))
    }

    /// Format depth data (bids/asks) for a symbol
    pub fn format_depth_data(
        &self,
        bids: &[(u64, u64)],
        asks: &[(u64, u64)],
        symbol_id: u32,
    ) -> Result<(Vec<[String; 2]>, Vec<[String; 2]>), String> {
        let formatted_bids: Vec<[String; 2]> = bids
            .iter()
            .map(|(p, q)| {
                Ok([
                    self.format_price(*p, symbol_id)?,
                    self.format_qty(*q, symbol_id)?,
                ])
            })
            .collect::<Result<_, String>>()?;

        let formatted_asks: Vec<[String; 2]> = asks
            .iter()
            .map(|(p, q)| {
                Ok([
                    self.format_price(*p, symbol_id)?,
                    self.format_qty(*q, symbol_id)?,
                ])
            })
            .collect::<Result<_, String>>()?;

        Ok((formatted_bids, formatted_asks))
    }
}

// ============================================================================
// Internal Format Helpers (private)
// ============================================================================

/// Format price with display decimals (internal use only)
fn format_price_internal(value: u64, display_decimals: u32) -> String {
    let divisor = 10u64.pow(display_decimals);
    format!(
        "{:.prec$}",
        value as f64 / divisor as f64,
        prec = display_decimals as usize
    )
}

/// Format quantity with display decimals (internal use only)
fn format_qty_internal(value: u64, decimals: u32, display_decimals: u32) -> String {
    let divisor = 10u64.pow(decimals);
    format!(
        "{:.prec$}",
        value as f64 / divisor as f64,
        prec = display_decimals as usize
    )
}

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
/// Get order book depth
///
/// GET /api/v1/depth?symbol=BTC_USDT&limit=20
pub async fn get_depth(
    State(state): State<Arc<AppState>>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Result<
    (StatusCode, Json<ApiResponse<super::types::DepthApiData>>),
    (StatusCode, Json<ApiResponse<()>>),
> {
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
        .ok_or_else(|| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<()>::error(
                    error_codes::SERVICE_UNAVAILABLE,
                    "Symbol not found",
                )),
            )
        })?;

    // Get symbol info for decimals
    let symbol_info = state
        .symbol_mgr
        .get_symbol_info_by_id(state.active_symbol_id)
        .ok_or_else(|| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<()>::error(
                    error_codes::SERVICE_UNAVAILABLE,
                    "Symbol info not found",
                )),
            )
        })?;

    let base_asset = state
        .symbol_mgr
        .assets
        .get(&symbol_info.base_asset_id)
        .ok_or_else(|| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<()>::error(
                    error_codes::SERVICE_UNAVAILABLE,
                    "Base asset not found",
                )),
            )
        })?;

    let price_decimals = symbol_info.price_display_decimal;
    let qty_decimals = base_asset.display_decimals;

    // Format response
    let data = super::types::DepthApiData {
        symbol: symbol_name.to_string(),
        bids: snapshot
            .bids
            .iter()
            .map(|(p, q)| {
                [
                    format_price_internal(*p, price_decimals),
                    format_qty_internal(*q, base_asset.decimals, base_asset.display_decimals),
                ]
            })
            .collect(),
        asks: snapshot
            .asks
            .iter()
            .map(|(p, q)| {
                [
                    format_price_internal(*p, price_decimals),
                    format_qty_internal(*q, base_asset.decimals, base_asset.display_decimals),
                ]
            })
            .collect(),
        last_update_id: snapshot.update_id,
    };

    Ok((StatusCode::OK, Json(ApiResponse::success(data))))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_qty_normal_cases() {
        // BTC: decimals=8, display_decimals=6
        assert_eq!(format_qty_internal(100000000, 8, 6), "1.000000");
        assert_eq!(format_qty_internal(50000000, 8, 6), "0.500000");
        assert_eq!(format_qty_internal(123456789, 8, 6), "1.234568"); // 四舍五入

        // ETH: decimals=8, display_decimals=4
        assert_eq!(format_qty_internal(100000000, 8, 4), "1.0000");
        assert_eq!(format_qty_internal(50000000, 8, 4), "0.5000");

        // USDT: decimals=8, display_decimals=4
        assert_eq!(format_qty_internal(1000000000, 8, 4), "10.0000");
    }

    #[test]
    fn test_format_qty_boundary_cases() {
        // 零值
        assert_eq!(format_qty_internal(0, 8, 6), "0.000000");
        assert_eq!(format_qty_internal(0, 8, 4), "0.0000");

        // 最小值（1 unit）
        assert_eq!(format_qty_internal(1, 8, 6), "0.000000"); // 小于 display_decimals，显示为 0
        assert_eq!(format_qty_internal(1, 8, 8), "0.00000001");

        // 大数值
        assert_eq!(format_qty_internal(1000000000000, 8, 6), "10000.000000");
        assert_eq!(format_qty_internal(u64::MAX, 8, 6), "184467440737.095520"); // u64::MAX / 10^8 (浮点精度)
    }

    #[test]
    fn test_format_qty_precision_edge_cases() {
        // display_decimals < decimals（常见情况）
        assert_eq!(format_qty_internal(123456789, 8, 6), "1.234568");
        assert_eq!(format_qty_internal(123456789, 8, 4), "1.2346");
        assert_eq!(format_qty_internal(123456789, 8, 2), "1.23");

        // display_decimals == decimals
        assert_eq!(format_qty_internal(123456789, 8, 8), "1.23456789");

        // display_decimals > decimals（不常见，但应该正确处理）
        assert_eq!(format_qty_internal(12345, 4, 6), "1.234500");
        assert_eq!(format_qty_internal(12345, 4, 8), "1.23450000");
    }

    #[test]
    fn test_format_qty_rounding() {
        // 测试四舍五入
        assert_eq!(format_qty_internal(123456789, 8, 6), "1.234568"); // .789 -> .68
        assert_eq!(format_qty_internal(123454999, 8, 6), "1.234550"); // .4999 -> .50
        assert_eq!(format_qty_internal(123455000, 8, 6), "1.234550"); // .5000 -> .50
        assert_eq!(format_qty_internal(123455001, 8, 6), "1.234550"); // .5001 -> .50
    }

    #[test]
    fn test_format_qty_different_decimals() {
        // decimals=6 (如某些代币)
        assert_eq!(format_qty_internal(1000000, 6, 4), "1.0000");
        assert_eq!(format_qty_internal(500000, 6, 4), "0.5000");

        // decimals=18 (如 ETH 链上)
        assert_eq!(format_qty_internal(1000000000000000000, 18, 6), "1.000000");
        assert_eq!(format_qty_internal(500000000000000000, 18, 6), "0.500000");

        // decimals=2 (如某些稳定币)
        assert_eq!(format_qty_internal(100, 2, 2), "1.00");
        assert_eq!(format_qty_internal(50, 2, 2), "0.50");
    }

    #[test]
    fn test_format_qty_real_world_scenarios() {
        // BTC 交易场景
        // 0.1 BTC = 10000000 (decimals=8)
        assert_eq!(format_qty_internal(10000000, 8, 6), "0.100000");

        // 0.00123456 BTC
        assert_eq!(format_qty_internal(123456, 8, 6), "0.001235");

        // ETH 交易场景
        // 1.5 ETH = 150000000 (decimals=8)
        assert_eq!(format_qty_internal(150000000, 8, 4), "1.5000");

        // USDT 交易场景
        // 1000 USDT = 100000000000 (decimals=8)
        assert_eq!(format_qty_internal(100000000000, 8, 4), "1000.0000");
    }

    #[test]
    fn test_format_price_normal_cases() {
        // BTC/USDT: price_decimals=2
        assert_eq!(format_price_internal(3000000, 2), "30000.00");
        assert_eq!(format_price_internal(2990000, 2), "29900.00");

        // 小数价格
        assert_eq!(format_price_internal(12345, 2), "123.45");
        assert_eq!(format_price_internal(100, 2), "1.00");
    }

    #[test]
    fn test_format_price_boundary_cases() {
        // 零值
        assert_eq!(format_price_internal(0, 2), "0.00");

        // 最小值
        assert_eq!(format_price_internal(1, 2), "0.01");

        // 大数值
        assert_eq!(format_price_internal(10000000, 2), "100000.00");
    }
}
