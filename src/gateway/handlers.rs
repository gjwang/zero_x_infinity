use axum::{
    Extension, Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
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
    #[allow(clippy::type_complexity)]
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
    Extension(user): Extension<crate::api_auth::AuthenticatedUser>,
    Json(req): Json<ClientOrder>,
) -> Result<(StatusCode, Json<ApiResponse<OrderResponseData>>), (StatusCode, Json<ApiResponse<()>>)>
{
    // 1. Extract user_id from authenticated user
    let user_id = user.user_id as u64;
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
    Extension(user): Extension<crate::api_auth::AuthenticatedUser>,
    Json(req): Json<CancelOrderRequest>,
) -> Result<(StatusCode, Json<ApiResponse<OrderResponseData>>), (StatusCode, Json<ApiResponse<()>>)>
{
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

/// Create internal transfer endpoint
///
/// POST /api/v1/private/transfer
///
/// Uses FSM-based transfer when coordinator is available, otherwise falls back to legacy.
pub async fn create_transfer(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<crate::api_auth::AuthenticatedUser>,
    Json(req): Json<crate::funding::transfer::TransferRequest>,
) -> Result<
    (
        StatusCode,
        Json<ApiResponse<crate::funding::transfer::TransferResponse>>,
    ),
    (StatusCode, Json<ApiResponse<()>>),
> {
    // 1. Extract user_id from authenticated user
    let user_id = user.user_id as u64;
    tracing::info!("[TRACE] Transfer Request: From User {}", user_id);

    // 2. Check if FSM coordinator is available
    if let Some(ref coordinator) = state.transfer_coordinator {
        // Use new FSM-based transfer
        return create_transfer_fsm_handler(state.clone(), coordinator, user_id, req).await;
    }

    // 3. Fallback to legacy transfer service
    let db = state.pg_db.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiResponse::<()>::error(
                error_codes::SERVICE_UNAVAILABLE,
                "Database not available",
            )),
        )
    })?;

    match crate::funding::service::TransferService::execute(db, user_id as i64, req).await {
        Ok(resp) => Ok((StatusCode::OK, Json(ApiResponse::success(resp)))),
        Err(e) => {
            tracing::error!("Transfer failed: {:?}", e);
            Err((
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<()>::error(
                    error_codes::INVALID_PARAMETER,
                    e.to_string(),
                )),
            ))
        }
    }
}

/// FSM-based transfer handler (internal)
async fn create_transfer_fsm_handler(
    state: Arc<AppState>,
    coordinator: &std::sync::Arc<crate::transfer::TransferCoordinator>,
    user_id: u64,
    req: crate::funding::transfer::TransferRequest,
) -> Result<
    (
        StatusCode,
        Json<ApiResponse<crate::funding::transfer::TransferResponse>>,
    ),
    (StatusCode, Json<ApiResponse<()>>),
> {
    // Convert legacy request to FSM request
    let fsm_req = crate::transfer::TransferApiRequest {
        from: req.from.clone(),
        to: req.to.clone(),
        asset: req.asset.clone(),
        amount: req.amount.clone(),
        cid: None, // Legacy API doesn't have cid
    };

    // Lookup asset to get decimals
    let asset = state
        .pg_assets
        .iter()
        .find(|a| a.asset.to_lowercase() == req.asset.to_lowercase())
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<()>::error(
                    error_codes::INVALID_PARAMETER,
                    format!("Asset not found: {}", req.asset),
                )),
            )
        })?;

    // Call FSM transfer
    match crate::transfer::create_transfer_fsm(
        coordinator,
        user_id,
        fsm_req,
        asset.asset_id as u32,
        asset.decimals as u32,
    )
    .await
    {
        Ok(fsm_resp) => {
            // Convert FSM response to legacy response
            let legacy_resp = crate::funding::transfer::TransferResponse {
                transfer_id: fsm_resp.req_id,
                status: fsm_resp.status,
                from: fsm_resp.from,
                to: fsm_resp.to,
                asset: fsm_resp.asset,
                amount: fsm_resp.amount,
                timestamp: fsm_resp.timestamp,
            };
            Ok((StatusCode::OK, Json(ApiResponse::success(legacy_resp))))
        }
        Err((status, err_resp)) => Err((
            status,
            Json(ApiResponse::<()>::error(
                err_resp.code,
                err_resp.msg.unwrap_or_default(),
            )),
        )),
    }
}

/// Get transfer status endpoint
///
/// GET /api/v1/private/transfer/:req_id
pub async fn get_transfer(
    State(state): State<Arc<AppState>>,
    Path(req_id): Path<u64>,
) -> Result<
    (
        StatusCode,
        Json<ApiResponse<crate::transfer::TransferApiResponse>>,
    ),
    (StatusCode, Json<ApiResponse<()>>),
> {
    // Check if FSM coordinator is available
    let coordinator = state.transfer_coordinator.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiResponse::<()>::error(
                error_codes::SERVICE_UNAVAILABLE,
                "Transfer service not available (FSM not enabled)",
            )),
        )
    })?;

    // Get decimals from first asset (default 8) - in production, would lookup from transfer record
    let decimals = state
        .pg_assets
        .first()
        .map(|a| a.decimals as u32)
        .unwrap_or(8);

    // Query transfer status
    match crate::transfer::get_transfer_status(coordinator, req_id, decimals).await {
        Ok(resp) => Ok((StatusCode::OK, Json(ApiResponse::success(resp)))),
        Err((status, err_resp)) => Err((
            status,
            Json(ApiResponse::<()>::error(
                err_resp.code,
                err_resp.msg.unwrap_or_default(),
            )),
        )),
    }
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
                format!("Query failed: {}", e),
            )),
        )),
    }
}

/// Get orders list
///
/// GET /api/v1/orders?user_id=1001&limit=10
pub async fn get_orders(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<crate::api_auth::AuthenticatedUser>,
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

    // 1. Use user_id from authenticated user
    let user_id = user.user_id as u64;

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
                format!("Query failed: {}", e),
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
                format!("Query failed: {}", e),
            )),
        )),
    }
}

/// Get user balance
///
/// GET /api/v1/balances?user_id=1001&asset_id=1
pub async fn get_balances(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<crate::api_auth::AuthenticatedUser>,
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

    // 1. Use user_id from authenticated user
    let user_id = user.user_id as u64;

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
                format!("Query failed: {}", e),
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
                format!("Query failed: {}", e),
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

    // Use DepthFormatter for type-safe formatting
    let formatter = DepthFormatter::new(&state.symbol_mgr);

    let (formatted_bids, formatted_asks) = formatter
        .format_depth_data(&snapshot.bids, &snapshot.asks, state.active_symbol_id)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<()>::error(
                    error_codes::SERVICE_UNAVAILABLE,
                    &e,
                )),
            )
        })?;

    let data = super::types::DepthApiData {
        symbol: symbol_name.to_string(),
        bids: formatted_bids,
        asks: formatted_asks,
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
        assert_eq!(format_qty_internal(123456789, 8, 6), "1.234568"); // rounding

        // ETH: decimals=8, display_decimals=4
        assert_eq!(format_qty_internal(100000000, 8, 4), "1.0000");
        assert_eq!(format_qty_internal(50000000, 8, 4), "0.5000");

        // USDT: decimals=8, display_decimals=4
        assert_eq!(format_qty_internal(1000000000, 8, 4), "10.0000");
    }

    #[test]
    fn test_format_qty_boundary_cases() {
        // Zero value
        assert_eq!(format_qty_internal(0, 8, 6), "0.000000");
        assert_eq!(format_qty_internal(0, 8, 4), "0.0000");

        // Minimum value (1 unit)
        assert_eq!(format_qty_internal(1, 8, 6), "0.000000"); // less than display_decimals, shows as 0
        assert_eq!(format_qty_internal(1, 8, 8), "0.00000001");

        // Large value
        assert_eq!(format_qty_internal(1000000000000, 8, 6), "10000.000000");
        assert_eq!(format_qty_internal(u64::MAX, 8, 6), "184467440737.095520"); // u64::MAX / 10^8 (float precision)
    }

    #[test]
    fn test_format_qty_precision_edge_cases() {
        // display_decimals < decimals (common case)
        assert_eq!(format_qty_internal(123456789, 8, 6), "1.234568");
        assert_eq!(format_qty_internal(123456789, 8, 4), "1.2346");
        assert_eq!(format_qty_internal(123456789, 8, 2), "1.23");

        // display_decimals == decimals
        assert_eq!(format_qty_internal(123456789, 8, 8), "1.23456789");

        // display_decimals > decimals (uncommon, but should handle correctly)
        assert_eq!(format_qty_internal(12345, 4, 6), "1.234500");
        assert_eq!(format_qty_internal(12345, 4, 8), "1.23450000");
    }

    #[test]
    fn test_format_qty_rounding() {
        // Test rounding
        assert_eq!(format_qty_internal(123456789, 8, 6), "1.234568"); // .789 -> .68
        assert_eq!(format_qty_internal(123454999, 8, 6), "1.234550"); // .4999 -> .50
        assert_eq!(format_qty_internal(123455000, 8, 6), "1.234550"); // .5000 -> .50
        assert_eq!(format_qty_internal(123455001, 8, 6), "1.234550"); // .5001 -> .50
    }

    #[test]
    fn test_format_qty_different_decimals() {
        // decimals=6 (like some tokens)
        assert_eq!(format_qty_internal(1000000, 6, 4), "1.0000");
        assert_eq!(format_qty_internal(500000, 6, 4), "0.5000");

        // decimals=18 (like ETH on-chain)
        assert_eq!(format_qty_internal(1000000000000000000, 18, 6), "1.000000");
        assert_eq!(format_qty_internal(500000000000000000, 18, 6), "0.500000");

        // decimals=2 (like some stablecoins)
        assert_eq!(format_qty_internal(100, 2, 2), "1.00");
        assert_eq!(format_qty_internal(50, 2, 2), "0.50");
    }

    #[test]
    fn test_format_qty_real_world_scenarios() {
        // BTC trading scenario
        // 0.1 BTC = 10000000 (decimals=8)
        assert_eq!(format_qty_internal(10000000, 8, 6), "0.100000");

        // 0.00123456 BTC
        assert_eq!(format_qty_internal(123456, 8, 6), "0.001235");

        // ETH trading scenario
        // 1.5 ETH = 150000000 (decimals=8)
        assert_eq!(format_qty_internal(150000000, 8, 4), "1.5000");

        // USDT trading scenario
        // 1000 USDT = 100000000000 (decimals=8)
        assert_eq!(format_qty_internal(100000000000, 8, 4), "1000.0000");
    }

    #[test]
    fn test_format_price_normal_cases() {
        // BTC/USDT: price_decimals=2
        assert_eq!(format_price_internal(3000000, 2), "30000.00");
        assert_eq!(format_price_internal(2990000, 2), "29900.00");

        // Decimal price
        assert_eq!(format_price_internal(12345, 2), "123.45");
        assert_eq!(format_price_internal(100, 2), "1.00");
    }

    #[test]
    fn test_format_price_boundary_cases() {
        // Zero value
        assert_eq!(format_price_internal(0, 2), "0.00");

        // Minimum value
        assert_eq!(format_price_internal(1, 2), "0.01");

        // Large value
        assert_eq!(format_price_internal(10000000, 2), "100000.00");
    }
}

// ============================================================================
// Health Check API
// ============================================================================

/// Health check response data
#[derive(serde::Serialize)]
pub struct HealthResponse {
    /// Server timestamp in milliseconds
    pub timestamp_ms: u64,
}

/// Health check endpoint
///
/// Returns service health status with server timestamp.
/// Internally checks all dependencies (TDengine, etc.) but does NOT
/// expose any internal details in the response.
///
/// - Healthy: 200 OK + {code: 0, data: {timestamp_ms}}
/// - Unhealthy: 503 Service Unavailable + {code: 503, msg: "unavailable"}
pub async fn health_check(
    State(state): State<Arc<AppState>>,
) -> (StatusCode, Json<ApiResponse<HealthResponse>>) {
    use taos::AsyncQueryable;

    // Rate limit: only ping DB once per interval
    static LAST_CHECK_MS: AtomicU64 = AtomicU64::new(0);
    const CHECK_INTERVAL_MS: u64 = 5000; // 5 seconds

    // Get current timestamp in milliseconds
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);

    // Check if we need to do actual DB ping (rate limited)
    let last_check = LAST_CHECK_MS.load(Ordering::Relaxed);
    let all_healthy = if now_ms - last_check > CHECK_INTERVAL_MS {
        // Interval expired, do actual DB check
        LAST_CHECK_MS.store(now_ms, Ordering::Relaxed);
        if let Some(ref db_client) = state.db_client {
            match db_client.taos().exec("SELECT 1").await {
                Ok(_) => true,
                Err(e) => {
                    tracing::error!("[HEALTH] TDengine ping failed: {}", e);
                    false
                }
            }
        } else {
            tracing::error!("[HEALTH] No db_client configured");
            false
        }
    } else {
        true // Within interval, assume healthy
    };

    if all_healthy {
        (
            StatusCode::OK,
            Json(ApiResponse::success(HealthResponse {
                timestamp_ms: now_ms,
            })),
        )
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiResponse {
                code: 503,
                msg: "unavailable".to_string(),
                data: None,
            }),
        )
    }
}

// ============================================================================
// Phase 0x0A: Account Management Endpoints
// ============================================================================

/// Asset API response data
#[derive(serde::Serialize)]
pub struct AssetApiData {
    pub asset_id: i32,
    pub asset: String,
    pub name: String,
    pub decimals: i16,
    pub can_deposit: bool,
    pub can_withdraw: bool,
    pub can_trade: bool,
}

/// Symbol API response data
#[derive(serde::Serialize)]
pub struct SymbolApiData {
    pub symbol_id: i32,
    pub symbol: String,
    pub base_asset: String,
    pub quote_asset: String,
    pub price_decimals: i16,
    pub qty_decimals: i16,
    pub is_tradable: bool,
    pub is_visible: bool,
}

/// Get all assets
///
/// GET /api/v1/assets
pub async fn get_assets(
    State(state): State<Arc<AppState>>,
) -> Result<(StatusCode, Json<ApiResponse<Vec<AssetApiData>>>), (StatusCode, Json<ApiResponse<()>>)>
{
    let assets: Vec<AssetApiData> = state
        .pg_assets
        .iter()
        .map(|a| AssetApiData {
            asset_id: a.asset_id,
            asset: a.asset.clone(),
            name: a.name.clone(),
            decimals: a.decimals,
            can_deposit: a.can_deposit(),
            can_withdraw: a.can_withdraw(),
            can_trade: a.can_trade(),
        })
        .collect();

    Ok((StatusCode::OK, Json(ApiResponse::success(assets))))
}

/// Get all symbols (trading pairs)
///
/// GET /api/v1/symbols
pub async fn get_symbols(
    State(state): State<Arc<AppState>>,
) -> Result<(StatusCode, Json<ApiResponse<Vec<SymbolApiData>>>), (StatusCode, Json<ApiResponse<()>>)>
{
    // Build asset lookup map
    let asset_map: std::collections::HashMap<i32, &crate::account::Asset> =
        state.pg_assets.iter().map(|a| (a.asset_id, a)).collect();

    let symbols: Vec<SymbolApiData> = state
        .pg_symbols
        .iter()
        .map(|s| {
            let base_asset = asset_map
                .get(&s.base_asset_id)
                .map(|a| a.asset.clone())
                .unwrap_or_else(|| format!("UNKNOWN_{}", s.base_asset_id));
            let quote_asset = asset_map
                .get(&s.quote_asset_id)
                .map(|a| a.asset.clone())
                .unwrap_or_else(|| format!("UNKNOWN_{}", s.quote_asset_id));

            SymbolApiData {
                symbol_id: s.symbol_id,
                symbol: s.symbol.clone(),
                base_asset,
                quote_asset,
                price_decimals: s.price_decimals,
                qty_decimals: s.qty_decimals,
                is_tradable: s.is_tradable(),
                is_visible: s.is_visible(),
            }
        })
        .collect();

    Ok((StatusCode::OK, Json(ApiResponse::success(symbols))))
}

// ============================================================================
// Exchange Info API (Combined Assets + Symbols)
// ============================================================================

/// Exchange info response data
#[derive(serde::Serialize)]
pub struct ExchangeInfoData {
    pub assets: Vec<AssetApiData>,
    pub symbols: Vec<SymbolApiData>,
    pub server_time: u64,
}

/// Get exchange info (combined assets and symbols)
///
/// GET /api/v1/exchange_info
/// Returns all assets and symbols in a single response.
pub async fn get_exchange_info(
    State(state): State<Arc<AppState>>,
) -> Result<(StatusCode, Json<ApiResponse<ExchangeInfoData>>), (StatusCode, Json<ApiResponse<()>>)>
{
    // Build asset list
    let assets: Vec<AssetApiData> = state
        .pg_assets
        .iter()
        .map(|a| AssetApiData {
            asset_id: a.asset_id,
            asset: a.asset.clone(),
            name: a.name.clone(),
            decimals: a.decimals,
            can_deposit: a.can_deposit(),
            can_withdraw: a.can_withdraw(),
            can_trade: a.can_trade(),
        })
        .collect();

    // Build asset lookup map for symbols
    let asset_map: std::collections::HashMap<i32, &crate::account::Asset> =
        state.pg_assets.iter().map(|a| (a.asset_id, a)).collect();

    // Build symbol list
    let symbols: Vec<SymbolApiData> = state
        .pg_symbols
        .iter()
        .map(|s| {
            let base_asset = asset_map
                .get(&s.base_asset_id)
                .map(|a| a.asset.clone())
                .unwrap_or_else(|| format!("UNKNOWN_{}", s.base_asset_id));
            let quote_asset = asset_map
                .get(&s.quote_asset_id)
                .map(|a| a.asset.clone())
                .unwrap_or_else(|| format!("UNKNOWN_{}", s.quote_asset_id));

            SymbolApiData {
                symbol_id: s.symbol_id,
                symbol: s.symbol.clone(),
                base_asset,
                quote_asset,
                price_decimals: s.price_decimals,
                qty_decimals: s.qty_decimals,
                is_tradable: s.is_tradable(),
                is_visible: s.is_visible(),
            }
        })
        .collect();

    let exchange_info = ExchangeInfoData {
        assets,
        symbols,
        server_time: now_ms(),
    };

    Ok((StatusCode::OK, Json(ApiResponse::success(exchange_info))))
}
