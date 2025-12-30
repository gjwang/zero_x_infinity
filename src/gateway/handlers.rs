use axum::{
    Extension, Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use utoipa::ToSchema;

use crate::pipeline::OrderAction;

use super::state::AppState;
use super::types::{
    AccountResponseData, ApiResponse, CancelOrderRequest, ClientOrder, DepthApiData,
    OrderResponseData, error_codes,
};

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
#[utoipa::path(
    post,
    path = "/api/v1/private/transfer",
    request_body(content = String, description = "Transfer request: from, to, asset, amount", content_type = "application/json"),
    responses(
        (status = 200, description = "Transfer completed", content_type = "application/json"),
        (status = 400, description = "Invalid parameters or insufficient balance"),
        (status = 401, description = "Authentication failed"),
        (status = 503, description = "Service unavailable")
    ),
    security(("ed25519_auth" = [])),
    tag = "Transfer"
)]
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
            let err_msg = e.to_string();
            tracing::error!("Transfer failed: {}", err_msg);
            Err((
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<()>::error(
                    error_codes::INVALID_PARAMETER,
                    err_msg,
                )),
            ))
        }
    }
}

/// FSM-based transfer handler (internal)
async fn create_transfer_fsm_handler(
    state: Arc<AppState>,
    coordinator: &std::sync::Arc<crate::internal_transfer::TransferCoordinator>,
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
    tracing::info!(
        "[DEBUG] FSM Handler Raw Request: from='{}' to='{}' asset='{}' amount='{}' cid={:?}",
        req.from,
        req.to,
        req.asset,
        req.amount,
        req.cid
    );

    let fsm_req = crate::internal_transfer::TransferApiRequest {
        from: req.from.clone(),
        to: req.to.clone(),
        asset: req.asset.clone(),
        amount: req.amount.clone(),
        cid: req.cid.clone(), // Pass through client idempotency key
    };

    // Lookup asset and create validation info
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

    // Create AssetValidationInfo for security checks
    let asset_info = crate::internal_transfer::AssetValidationInfo::from_asset(asset);

    // Call FSM transfer with full asset validation
    match crate::internal_transfer::create_transfer_fsm(coordinator, user_id, fsm_req, asset_info)
        .await
    {
        Ok(fsm_resp) => {
            // Convert FSM response to legacy response
            let legacy_resp = crate::funding::transfer::TransferResponse {
                transfer_id: fsm_resp.transfer_id,
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
/// GET /api/v1/private/transfer/{req_id}
#[utoipa::path(
    get,
    path = "/api/v1/private/transfer/{req_id}",
    params(
        ("req_id" = String, Path, description = "Transfer request ID (ULID format)")
    ),
    responses(
        (status = 200, description = "Transfer status", content_type = "application/json"),
        (status = 400, description = "Invalid request ID format"),
        (status = 404, description = "Transfer not found"),
        (status = 503, description = "Service unavailable")
    ),
    security(("ed25519_auth" = [])),
    tag = "Transfer"
)]
pub async fn get_transfer(
    State(state): State<Arc<AppState>>,
    Path(req_id_str): Path<String>,
) -> Result<
    (
        StatusCode,
        Json<ApiResponse<crate::internal_transfer::TransferApiResponse>>,
    ),
    (StatusCode, Json<ApiResponse<()>>),
> {
    // Parse req_id from string (ULID format)
    let req_id: crate::internal_transfer::InternalTransferId =
        req_id_str.parse().map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<()>::error(
                    error_codes::INVALID_PARAMETER,
                    "Invalid request ID format",
                )),
            )
        })?;

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
    match crate::internal_transfer::get_transfer_status(coordinator, req_id, decimals).await {
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

    // SEC-004 FIX: Extract user_id from authenticated user
    let user_id = user.user_id as u64;

    // Parse query parameters
    let limit: usize = params
        .get("limit")
        .and_then(|s| s.parse().ok())
        .unwrap_or(100);

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
) -> Result<
    (
        StatusCode,
        Json<ApiResponse<Vec<crate::persistence::queries::PublicTradeApiData>>>,
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
        .unwrap_or(500)
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
/// GET /api/v1/private/balances?asset_id=1
#[utoipa::path(
    get,
    path = "/api/v1/private/balances",
    params(
        ("asset_id" = u32, Query, description = "Asset ID")
    ),
    responses(
        (status = 200, description = "Balance details", content_type = "application/json"),
        (status = 400, description = "Missing asset_id"),
        (status = 404, description = "Balance not found"),
        (status = 503, description = "Service unavailable")
    ),
    security(("ed25519_auth" = [])),
    tag = "Account"
)]
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

/// Get all user balances (all account types: Spot, Funding)
///
/// GET /api/v1/private/balances/all
/// Returns balances from PostgreSQL (real source of truth)
#[utoipa::path(
    get,
    path = "/api/v1/private/balances/all",
    responses(
        (status = 200, description = "All user balances", content_type = "application/json"),
        (status = 503, description = "Service unavailable")
    ),
    security(("ed25519_auth" = [])),
    tag = "Account"
)]
pub async fn get_all_balances(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<crate::api_auth::AuthenticatedUser>,
) -> Result<
    (
        StatusCode,
        Json<ApiResponse<Vec<crate::funding::service::BalanceInfo>>>,
    ),
    (StatusCode, Json<ApiResponse<()>>),
> {
    // Extract user_id from authenticated user
    let user_id = user.user_id;

    // Check if PostgreSQL is available
    let pg_db = state.pg_db.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiResponse::<()>::error(
                error_codes::SERVICE_UNAVAILABLE,
                "Account database not available",
            )),
        )
    })?;

    // Query all balances
    match crate::funding::service::TransferService::get_all_balances(pg_db.pool(), user_id).await {
        Ok(balances) => Ok((StatusCode::OK, Json(ApiResponse::success(balances)))),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<()>::error(
                error_codes::SERVICE_UNAVAILABLE,
                format!("Query failed: {}", e),
            )),
        )),
    }
}

/// Get account information (balances) for legacy test script compatibility
///
/// GET /api/v1/private/account
/// Returns balances wrapped in a 'balances' field.
#[utoipa::path(
    get,
    path = "/api/v1/private/account",
    responses(
        (status = 200, description = "User account info", body = AccountResponseData, content_type = "application/json"),
        (status = 503, description = "Service unavailable")
    ),
    security(("ed25519_auth" = [])),
    tag = "Account"
)]
pub async fn get_account(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<crate::api_auth::AuthenticatedUser>,
) -> Result<(StatusCode, Json<ApiResponse<AccountResponseData>>), (StatusCode, Json<ApiResponse<()>>)>
{
    let user_id = user.user_id;
    tracing::info!("DEBUG: get_account called for user_id: {}", user_id);

    // 1. Get Funding balances from Postgres (Source of truth for funding)
    let pg_db = state.pg_db.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiResponse::<()>::error(
                error_codes::SERVICE_UNAVAILABLE,
                "Account database not available",
            )),
        )
    })?;

    let mut balances = match crate::funding::service::TransferService::get_all_balances(
        pg_db.pool(),
        user_id as i64,
    )
    .await
    {
        Ok(b) => b,
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<()>::error(
                    error_codes::SERVICE_UNAVAILABLE,
                    format!("Postgres query failed: {}", e),
                )),
            ));
        }
    };

    // Filter out any "spot" records from Postgres (if they exist due to legacy/residue)
    // Design intent: Spot balances ONLY come from TDengine/Settlement.
    balances.retain(|b| b.account_type != "spot");

    // 2. Get Spot balances from TDengine (Source of truth for trading)
    // Use PostgreSQL assets_tb as ONLY source of truth for asset configuration
    if let Some(ref t_client) = state.db_client {
        match crate::persistence::queries::query_all_balances_with_pg(
            t_client.taos(),
            pg_db.pool(),
            user_id as u64,
        )
        .await
        {
            Ok(spot_balances) => {
                for sb in spot_balances {
                    // Map BalanceApiData to BalanceInfo
                    balances.push(crate::funding::service::BalanceInfo {
                        asset_id: 0, // Not strictly required for the response type
                        asset: sb.asset,
                        account_type: "spot".to_string(),
                        available: sb.avail,
                        frozen: sb.frozen,
                    });
                }
            }
            Err(e) => {
                tracing::error!("TDengine Spot balance query failed: {}", e);
                // We don't fail the whole request if TDengine is down, just return Funding
            }
        }
    }

    let data = AccountResponseData { balances };
    Ok((StatusCode::OK, Json(ApiResponse::success(data))))
}

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
#[derive(serde::Serialize, ToSchema)]
pub struct HealthResponse {
    /// Server timestamp in milliseconds
    #[schema(example = 1703494800000_u64)]
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
#[utoipa::path(
    get,
    path = "/api/v1/health",
    responses(
        (status = 200, description = "Service healthy", body = HealthResponse, content_type = "application/json"),
        (status = 503, description = "Service unavailable")
    ),
    tag = "System"
)]

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
#[derive(serde::Serialize, ToSchema)]
pub struct AssetApiData {
    /// Asset ID
    #[schema(example = 1)]
    pub asset_id: i32,
    /// Asset symbol (e.g., BTC)
    #[schema(example = "BTC")]
    pub asset: String,
    /// Asset full name
    #[schema(example = "Bitcoin")]
    pub name: String,
    /// Decimal precision
    #[schema(example = 8)]
    pub decimals: i16,
    /// Can deposit
    pub can_deposit: bool,
    /// Can withdraw
    pub can_withdraw: bool,
    /// Can trade
    pub can_trade: bool,
}

/// Symbol API response data
#[derive(serde::Serialize, ToSchema)]
pub struct SymbolApiData {
    /// Symbol ID
    #[schema(example = 1)]
    pub symbol_id: i32,
    /// Symbol name (e.g., BTC_USDT)
    #[schema(example = "BTC_USDT")]
    pub symbol: String,
    /// Base asset symbol
    #[schema(example = "BTC")]
    pub base_asset: String,
    /// Quote asset symbol
    #[schema(example = "USDT")]
    pub quote_asset: String,
    /// Price decimal precision
    pub price_decimals: i16,
    /// Quantity decimal precision
    pub qty_decimals: i16,
    /// Is trading enabled
    pub is_tradable: bool,
    /// Is visible in UI
    pub is_visible: bool,
    /// Base maker fee in basis points
    #[schema(example = 10)]
    pub base_maker_fee: i32,
    /// Base taker fee in basis points
    #[schema(example = 20)]
    pub base_taker_fee: i32,
}

/// Get all assets
///
/// GET /api/v1/assets
#[utoipa::path(
    get,
    path = "/api/v1/public/assets",
    responses(
        (status = 200, description = "List of all assets", body = ApiResponse<Vec<AssetApiData>>)
    ),
    tag = "Market Data"
)]
pub async fn get_assets(
    State(state): State<Arc<AppState>>,
) -> Result<(StatusCode, Json<ApiResponse<Vec<AssetApiData>>>), (StatusCode, Json<ApiResponse<()>>)>
{
    // Use TTL-cached loader (30 second cache, refreshes on expiry)
    if let Some(ref pg_db) = state.pg_db {
        match super::cache::load_assets_cached(pg_db.pool().clone().into()).await {
            Ok(assets) => {
                let data: Vec<AssetApiData> = assets
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
                return Ok((StatusCode::OK, Json(ApiResponse::success(data))));
            }
            Err(e) => {
                tracing::warn!("[get_assets] Cached loader failed, falling back: {}", e);
            }
        }
    }

    // Fallback to startup cache if DB unavailable
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
#[utoipa::path(
    get,
    path = "/api/v1/public/symbols",
    responses(
        (status = 200, description = "List of all trading pairs", body = ApiResponse<Vec<SymbolApiData>>)
    ),
    tag = "Market Data"
)]
pub async fn get_symbols(
    State(state): State<Arc<AppState>>,
) -> Result<(StatusCode, Json<ApiResponse<Vec<SymbolApiData>>>), (StatusCode, Json<ApiResponse<()>>)>
{
    // Use TTL-cached loaders (30 second cache, refreshes on expiry)
    if let Some(ref pg_db) = state.pg_db {
        let pool = pg_db.pool().clone().into();
        let assets_result = super::cache::load_assets_cached(Arc::clone(&pool)).await;
        let symbols_result = super::cache::load_symbols_cached(pool).await;

        if let (Ok(assets), Ok(symbols)) = (assets_result, symbols_result) {
            let asset_map: std::collections::HashMap<i32, &crate::account::Asset> =
                assets.iter().map(|a| (a.asset_id, a)).collect();

            let data: Vec<SymbolApiData> = symbols
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
                        base_maker_fee: s.base_maker_fee,
                        base_taker_fee: s.base_taker_fee,
                    }
                })
                .collect();
            return Ok((StatusCode::OK, Json(ApiResponse::success(data))));
        } else {
            tracing::warn!("[get_symbols] Cached loader failed, falling back to startup cache");
        }
    }

    // Fallback to startup cache if DB unavailable
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
                base_maker_fee: s.base_maker_fee,
                base_taker_fee: s.base_taker_fee,
            }
        })
        .collect();

    Ok((StatusCode::OK, Json(ApiResponse::success(symbols))))
}

// ============================================================================
// Exchange Info API (Combined Assets + Symbols)
// ============================================================================

/// Exchange info response data
#[derive(serde::Serialize, ToSchema)]
pub struct ExchangeInfoData {
    /// All available assets
    pub assets: Vec<AssetApiData>,
    /// All trading pairs
    pub symbols: Vec<SymbolApiData>,
    /// Server timestamp in milliseconds
    #[schema(example = 1703494800000_u64)]
    pub server_time: u64,
}

/// Get exchange info (combined assets and symbols)
///
/// GET /api/v1/exchange_info
/// Returns all assets and symbols in a single response.
#[utoipa::path(
    get,
    path = "/api/v1/public/exchange_info",
    responses(
        (status = 200, description = "Exchange metadata", body = ApiResponse<ExchangeInfoData>)
    ),
    tag = "Market Data"
)]

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
                base_maker_fee: s.base_maker_fee,
                base_taker_fee: s.base_taker_fee,
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

/// Account info for JWT auth
pub async fn get_account_jwt(
    State(state): State<Arc<AppState>>,
    Extension(claims): Extension<crate::user_auth::service::Claims>,
) -> Result<(StatusCode, Json<ApiResponse<AccountResponseData>>), (StatusCode, Json<ApiResponse<()>>)>
{
    let user_id = claims.sub.parse::<i64>().unwrap_or_default();
    tracing::info!("DEBUG: get_account_jwt called for user_id: {}", user_id);

    // 1. Get Funding balances from Postgres
    let pg_db = state.pg_db.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiResponse::<()>::error(
                error_codes::SERVICE_UNAVAILABLE,
                "Account database not available",
            )),
        )
    })?;

    let mut balances =
        match crate::funding::service::TransferService::get_all_balances(pg_db.pool(), user_id)
            .await
        {
            Ok(b) => b,
            Err(e) => {
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ApiResponse::<()>::error(
                        error_codes::SERVICE_UNAVAILABLE,
                        format!("Postgres query failed: {}", e),
                    )),
                ));
            }
        };

    // Filter out any "spot" records from Postgres
    balances.retain(|b| b.account_type != "spot");

    // 2. Get Spot balances from TDengine
    if let Some(ref t_client) = state.db_client {
        match crate::persistence::queries::query_all_balances(
            t_client.taos(),
            user_id as u64,
            &state.symbol_mgr,
        )
        .await
        {
            Ok(spot_balances) => {
                for sb in spot_balances {
                    balances.push(crate::funding::service::BalanceInfo {
                        asset_id: 0,
                        asset: sb.asset,
                        account_type: "spot".to_string(),
                        available: sb.avail,
                        frozen: sb.frozen,
                    });
                }
            }
            Err(e) => {
                tracing::error!("TDengine Spot balance query failed: {}", e);
            }
        }
    }

    let data = AccountResponseData { balances };
    Ok((StatusCode::OK, Json(ApiResponse::success(data))))
}

// ============================================================================
// JWT Compatible Handlers (Phase 0x11-b)
// ============================================================================

/// Create transfer (JWT)
pub async fn create_transfer_jwt(
    State(state): State<Arc<AppState>>,
    Extension(claims): Extension<crate::user_auth::service::Claims>,
    Json(req): Json<crate::funding::transfer::TransferRequest>,
) -> Result<
    (
        StatusCode,
        Json<ApiResponse<crate::funding::transfer::TransferResponse>>,
    ),
    (StatusCode, Json<ApiResponse<()>>),
> {
    let user_id = claims.sub.parse::<i64>().unwrap_or_default();
    tracing::info!("[TRACE] Transfer Request (JWT): User {}", user_id);

    if let Some(ref coordinator) = state.transfer_coordinator {
        return create_transfer_fsm_handler(state.clone(), coordinator, user_id as u64, req).await;
    }

    let db = state.pg_db.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiResponse::<()>::error(
                error_codes::SERVICE_UNAVAILABLE,
                "Database not available",
            )),
        )
    })?;

    match crate::funding::service::TransferService::execute(db, user_id, req).await {
        Ok(resp) => Ok((StatusCode::OK, Json(ApiResponse::success(resp)))),
        Err(e) => Err((
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<()>::error(
                error_codes::INVALID_PARAMETER,
                e.to_string(),
            )),
        )),
    }
}

/// Create order (JWT)
pub async fn create_order_jwt(
    State(state): State<Arc<AppState>>,
    Extension(claims): Extension<crate::user_auth::service::Claims>,
    Json(req): Json<ClientOrder>,
) -> Result<(StatusCode, Json<ApiResponse<OrderResponseData>>), (StatusCode, Json<ApiResponse<()>>)>
{
    let user_id = claims.sub.parse::<u64>().unwrap_or_default();
    tracing::info!("[TRACE] Create Order (JWT): User {}", user_id);

    let validated =
        super::types::validate_client_order(req.clone(), &state.symbol_mgr).map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<()>::error(error_codes::INVALID_PARAMETER, e)),
            )
        })?;

    let order_id = state.next_order_id();
    let timestamp = now_ns();

    let internal_order = validated
        .into_internal_order(order_id, user_id, timestamp)
        .map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<()>::error(error_codes::INVALID_PARAMETER, e)),
            )
        })?;

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
                "Order queue is full",
            )),
        ));
    }

    Ok((
        StatusCode::OK,
        Json(ApiResponse::success(OrderResponseData {
            order_id,
            cid: req.cid,
            order_status: "ACCEPTED".to_string(),
            accepted_at: now_ms(),
        })),
    ))
}

/// Cancel order (JWT)
pub async fn cancel_order_jwt(
    State(state): State<Arc<AppState>>,
    Extension(claims): Extension<crate::user_auth::service::Claims>,
    Json(req): Json<CancelOrderRequest>,
) -> Result<(StatusCode, Json<ApiResponse<OrderResponseData>>), (StatusCode, Json<ApiResponse<()>>)>
{
    let user_id = claims.sub.parse::<u64>().unwrap_or_default();
    tracing::info!(
        "[TRACE] Cancel Order (JWT) {}: User {}",
        req.order_id,
        user_id
    );

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
                "Order queue is full",
            )),
        ));
    }

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

/// Get orders (JWT)
pub async fn get_orders_jwt(
    State(state): State<Arc<AppState>>,
    Extension(claims): Extension<crate::user_auth::service::Claims>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Result<
    (
        StatusCode,
        Json<ApiResponse<Vec<crate::persistence::queries::OrderApiData>>>,
    ),
    (StatusCode, Json<ApiResponse<()>>),
> {
    let user_id = claims.sub.parse::<u64>().unwrap_or_default();
    let db_client = state.db_client.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiResponse::<()>::error(
                error_codes::SERVICE_UNAVAILABLE,
                "Persistence not enabled",
            )),
        )
    })?;

    let limit: usize = params
        .get("limit")
        .and_then(|s| s.parse().ok())
        .unwrap_or(10);

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

/// Get single balance (JWT)
pub async fn get_balance_jwt(
    State(state): State<Arc<AppState>>,
    Extension(claims): Extension<crate::user_auth::service::Claims>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Result<
    (
        StatusCode,
        Json<ApiResponse<crate::persistence::queries::BalanceApiData>>,
    ),
    (StatusCode, Json<ApiResponse<()>>),
> {
    let user_id = claims.sub.parse::<u64>().unwrap_or_default();
    let db_client = state.db_client.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ApiResponse::<()>::error(
                error_codes::SERVICE_UNAVAILABLE,
                "Persistence not enabled",
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
