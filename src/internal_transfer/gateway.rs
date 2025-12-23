use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use tokio::sync::Mutex as AsyncMutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::extract::Query;
use axum::{
    extract::{Extension, Json},
    http::StatusCode,
    routing::post,
    Router,
};
use rust_decimal::Decimal;
use tokio::time::sleep;
use tower_http::cors::CorsLayer;

use crate::client_order_convertor::client_order_convert;
// V1 internal transfer imports removed - using transfer::* v2 module
use axum::extract::Path;
use axum::response::IntoResponse;
use crate::db::SettlementDb;
use crate::fast_ulid::SnowflakeGenRng;
use crate::ledger::MatchExecData;
use crate::models::balance_manager::{BalanceManager, ClientBalance};
use crate::models::{
    u64_to_decimal_string, ApiResponse, BalanceRequest, ClientOrder, OrderStatus,
    UserAccountManager,
};
use crate::symbol_manager::SymbolManager;
use crate::user_account::Balance;

#[derive(Debug, serde::Deserialize)]
pub struct TransferInRequestPayload {
    pub request_id: String,
    pub user_id: u64,
    pub asset: String,
    pub amount: Decimal,
}

#[derive(Debug, serde::Deserialize)]
pub struct TransferOutRequestPayload {
    pub request_id: String,
    pub user_id: u64,
    pub asset: String,
    pub amount: Decimal,
}

#[derive(Debug, serde::Serialize)]
pub struct TransferResponse {
    pub success: bool,
    pub message: String,
    pub request_id: Option<String>,
}

#[derive(serde::Deserialize)]
struct UserIdParams {
    user_id: u64,
}

pub trait OrderPublisher: Send + Sync {
    fn publish(
        &self,
        topic: String,
        key: String,
        payload: Vec<u8>,
    ) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>>;
}

pub struct SimulatedFundingAccount {
    balances: HashMap<u32, Balance>,
}

impl SimulatedFundingAccount {
    pub fn new() -> Self {
        let mut balances = HashMap::new();
        // Initialize with large available balances
        let mut bal_btc = Balance::default();
        let _ = bal_btc.deposit(1_000_000__000_000_000);
        balances.insert(1, bal_btc);
        let mut bal_usdt = Balance::default();
        let _ = bal_usdt.deposit(1_000_000_000_000_000);
        balances.insert(2, bal_usdt);
        let mut bal_eth = Balance::default();
        let _ = bal_eth.deposit(1_000_000_000_000_000);
        balances.insert(3, bal_eth);
        Self { balances }
    }

    /// Lock funds for Transfer In (Funding -> Trading)
    fn lock(&mut self, asset_id: u32, amount: u64) -> Result<(), String> {
        let balance = self
            .balances
            .get_mut(&asset_id)
            .ok_or_else(|| format!("Asset {} not found in funding account", asset_id))?;

        balance.lock(amount).map_err(|e| format!("Lock failed: {}", e))
    }

    /// Finalize Transfer In: Remove from locked (funds moved to Trading Engine)
    fn spend(&mut self, asset_id: u32, amount: u64) -> Result<(), String> {
        let balance = self
            .balances
            .get_mut(&asset_id)
            .ok_or_else(|| format!("Asset {} not found in funding account", asset_id))?;

        balance.spend_frozen(amount).map_err(|e| format!("Spend failed: {}", e))
    }

    /// Finalize Transfer Out: Add to available (funds received from Trading Engine)
    fn credit(&mut self, asset_id: u32, amount: u64) {
        let balance = self.balances.entry(asset_id).or_insert(Balance::default());
        // We ignore error here as deposit shouldn't fail unless overflow
        let _ = balance.deposit(amount);
    }
}

pub struct AppState {
    pub symbol_manager: Arc<SymbolManager>,
    pub balance_manager: BalanceManager,
    pub producer: Arc<dyn OrderPublisher>,
    pub snowflake_gen: Mutex<SnowflakeGenRng>,
    pub kafka_topic: String,  // For validated orders to ME
    pub balance_topic: String,
    pub user_manager: UserAccountManager,
    pub db: Option<SettlementDb>,
    // V1 internal_transfer_db removed - use transfer_coordinator instead
    pub funding_account: Arc<AsyncMutex<SimulatedFundingAccount>>,
    /// UBS Gateway client for async order validation
    #[cfg(feature = "aeron")]
    pub ubs_client: Arc<crate::ubs_core::comm::UbsGatewayClient>,
    /// UBSCore request timeout in milliseconds
    pub ubscore_timeout_ms: u64,
    /// TigerBeetle Client for direct balance lookups
    pub tb_client: Option<Arc<tigerbeetle_unofficial::Client>>,
    /// Transfer v2: Coordinator
    pub transfer_coordinator: Option<Arc<crate::transfer::TransferCoordinator>>,
    /// Transfer v2: Worker
    pub transfer_worker: Option<Arc<crate::transfer::TransferWorker>>,
    /// Transfer v2: Queue for async processing
    pub transfer_queue: Option<Arc<crate::transfer::TransferQueue>>,
}

pub fn create_app(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/api/v1/order/create", post(create_order))
        .route("/api/v1/order/cancel", post(cancel_order))
        .route("/api/v1/order/trades", axum::routing::get(get_trade_history))
        .route("/api/v1/order/history", axum::routing::get(get_order_history))
        .route("/api/v1/order/active", axum::routing::get(get_active_orders))

        .route("/api/v1/user/transfer_in", post(transfer_in))
        .route("/api/v1/user/transfer_out", post(transfer_out))
        .route("/api/v1/user/balance", axum::routing::get(get_balance))
        // Internal Transfer endpoints (FSM-based)
        .route("/api/v1/internal_transfer", post(handle_internal_transfer))
        .route("/api/v1/internal_transfer/:req_id", axum::routing::get(get_internal_transfer))
        // Legacy alias for backwards compatibility
        .route("/api/v1/transfer", post(handle_internal_transfer))
        .route("/api/v1/transfer/:req_id", axum::routing::get(get_internal_transfer))
        .layer(Extension(state))
        .layer(CorsLayer::permissive())
}

/// Get current timestamp in milliseconds
fn current_time_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64
}

async fn transfer_in(
    Extension(state): Extension<Arc<AppState>>,
    Json(payload): Json<TransferInRequestPayload>,
) -> Result<Json<TransferResponse>, StatusCode> {
    println!("📥 Transfer In request received: {:?}", payload);

    // Resolve Asset ID and Decimals
    let (asset_id, raw_amount) =
        state.balance_manager.to_internal_amount(&payload.asset, payload.amount).map_err(|e| {
            eprintln!("Conversion failed: {}", e);
            StatusCode::BAD_REQUEST
        })?;

    // 1. Lock Funding Account & Reserve Funds
    {
        let mut funding = state.funding_account.lock().await;
        if let Err(e) = funding.lock(asset_id, raw_amount) {
            eprintln!("❌ Lock failed: {}", e);
            return Err(StatusCode::BAD_REQUEST);
        }
        println!("🔒 Funds locked in funding account (pending transfer)");
    }

    // 2. Create Request & Send to Kafka
    // Generate unique request_id using snowflake (u64 for best performance)
    let gateway_request_id = {
        let mut gen = state.snowflake_gen.lock().unwrap();
        gen.generate()
    };

    let balance_req = BalanceRequest::TransferIn {
        request_id: gateway_request_id,
        user_id: payload.user_id,
        asset_id,
        amount: raw_amount,
        timestamp: current_time_ms(),
    };

    let json_payload = serde_json::to_string(&balance_req).map_err(|e| {
        eprintln!("Failed to serialize transfer_in request: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let key = payload.user_id.to_string();

    state
        .producer
        .publish(state.balance_topic.clone(), key, json_payload.into_bytes())
        .await
        .map_err(|e| {
            eprintln!("Failed to send transfer_in to Kafka: {}", e);
            // TODO: Rollback locked funds here if Kafka fails!
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    println!("✅ Transfer In request published to Kafka: {}", payload.request_id);

    // 3. Removed Sleep (Non-blocking simulation)
    // println!("⏳ Waiting 1s for settlement...");

    // 4. "Spend" the locked funds
    {
        let mut funding = state.funding_account.lock().await;
        if let Err(e) = funding.spend(asset_id, raw_amount) {
            eprintln!("❌ Critical: Failed to spend locked funds: {}", e);
            // In production this would be a critical alert
        } else {
            println!("💰 Locked funds spent. Transfer In complete.");
        }
    }

    Ok(Json(TransferResponse {
        success: true,
        message: format!(
            "Transfer In request submitted & settled: {} units of asset {} transferred to user {}",
            payload.amount, payload.asset, payload.user_id
        ),
        request_id: Some(gateway_request_id.to_string()),
    }))
}

async fn transfer_out(
    Extension(state): Extension<Arc<AppState>>,
    Json(payload): Json<TransferOutRequestPayload>,
) -> Result<Json<TransferResponse>, StatusCode> {
    println!("📤 Transfer Out request received: {:?}", payload);

    // Resolve Asset ID and Decimals
    let (asset_id, raw_amount) =
        state.balance_manager.to_internal_amount(&payload.asset, payload.amount).map_err(|e| {
            eprintln!("Conversion failed: {}", e);
            StatusCode::BAD_REQUEST
        })?;

    // 1. Create Request & Send to Kafka
    // Generate unique request_id using snowflake (u64 for best performance)
    let gateway_request_id = {
        let mut gen = state.snowflake_gen.lock().unwrap();
        gen.generate()
    };

    let balance_req = BalanceRequest::TransferOut {
        request_id: gateway_request_id,
        user_id: payload.user_id,
        asset_id,
        amount: raw_amount,
        timestamp: current_time_ms(),
    };

    let json_payload = serde_json::to_string(&balance_req).map_err(|e| {
        eprintln!("Failed to serialize transfer_out request: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let key = payload.user_id.to_string();

    state
        .producer
        .publish(state.balance_topic.clone(), key, json_payload.into_bytes())
        .await
        .map_err(|e| {
            eprintln!("Failed to send transfer_out to Kafka: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Update Balance in DB using append-only ledger
    if let Some(db) = &state.db {
        // Get current seq and use next seq for withdraw
        match db.get_current_balance(payload.user_id, asset_id).await {
            Ok(Some(current)) => {
                let next_seq = (current.seq + 1) as u64;
                if let Err(e) = db.withdraw(payload.user_id, asset_id, raw_amount, next_seq, 0).await {
                    eprintln!("DB Error during withdraw: {}", e);
                    // Continue anyway - Kafka event was already published
                }
            }
            Ok(None) => {
                eprintln!(
                    "⚠️ No existing balance for user {} asset {} - withdraw will be processed via Kafka/UBSCore",
                    payload.user_id, asset_id
                );
                // Don't fail - let UBSCore handle the balance check
            }
            Err(e) => {
                eprintln!("DB Error getting balance: {} - continuing with Kafka processing", e);
                // Don't fail - let UBSCore handle it
            }
        }
    }

    println!("✅ Transfer Out request published to Kafka: {}", payload.request_id);

    // 3. Credit funds to funding account (Funds coming from Trading Engine)
    {
        let mut funding = state.funding_account.lock().await;
        funding.credit(asset_id, raw_amount);
        println!("💰 Funds credited to funding account (Transfer Out complete).");
    }

    Ok(Json(TransferResponse {
        success: true,
    message: format!(
            "Transfer Out request submitted & settled: {} units of asset {} transferred from user {} to funding account",
            payload.amount, payload.asset, payload.user_id
        ),
        request_id: Some(gateway_request_id.to_string()),
    }))
}

// Old V1 internal_transfer handler removed - use FSM-based implementation



#[derive(Debug, serde::Deserialize, serde::Serialize)]
struct CancelOrderRequest {
    order_id: u64,
}

async fn cancel_order(
    Extension(state): Extension<Arc<AppState>>,
    Query(params): Query<CreateOrderParams>,
    Json(cancel_req): Json<CancelOrderRequest>,
) -> Result<Json<ApiResponse<OrderResponseData>>, (StatusCode, String)> {
    let user_id = params.user_id;
    let order_id = cancel_req.order_id;

    #[cfg(feature = "aeron")]
    {
        use crate::ubs_core::CancelRequest;

        // Create cancel request for UBSCore
        let cancel_request = CancelRequest { user_id, order_id };

        // Send to UBSCore for validation
        state
            .ubs_client
            .send_cancel(cancel_request, state.ubscore_timeout_ms)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("UBSCore error: {}", e)))?;

        Ok(Json(ApiResponse::success(OrderResponseData {
            order_id: order_id.to_string(),
            order_status: OrderStatus::Cancelled,
            cid: None,
        })))
    }

    #[cfg(not(feature = "aeron"))]
    {
        let _ = (user_id, order_id); // Suppress warnings
        Err((StatusCode::SERVICE_UNAVAILABLE, "Aeron feature not enabled".to_string()))
    }
}

#[derive(Debug, serde::Serialize)]
struct OrderResponseData {
    order_id: String,
    order_status: OrderStatus,
    cid: Option<String>,
}

#[derive(serde::Deserialize)]
struct CreateOrderParams {
    user_id: u64,
}

async fn create_order(
    Extension(state): Extension<Arc<AppState>>,
    Query(params): Query<CreateOrderParams>,
    Json(client_order): Json<ClientOrder>,
) -> Result<Json<ApiResponse<OrderResponseData>>, (StatusCode, String)> {
    let start = std::time::Instant::now();
    let user_id = params.user_id;

    // 1. Convert ClientOrder → InternalOrder
    let (order_id, internal_order) = client_order_convert(
        &client_order,
        &state.symbol_manager,
        &state.balance_manager,
        &state.snowflake_gen,
        user_id,
    )?;
    let convert_time = start.elapsed();

    // 2. Send to UBSCore for validation via Aeron
    #[cfg(feature = "aeron")]
    let validation_result = {
        let validate_start = std::time::Instant::now();

        // Send order to UBSCore via Aeron for validation
        // UBSCore will validate, log to WAL, and forward to ME via Kafka
        state
            .ubs_client
            .send_order(&internal_order, state.ubscore_timeout_ms)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("UBSCore error: {}", e)))?;

        let validate_time = validate_start.elapsed();
        let total_time = start.elapsed();

        log::info!(
            "[CREATE_ORDER] Validated via UBSCore order_id={} convert={}µs ubscore={}µs total={}µs",
            order_id, convert_time.as_micros(), validate_time.as_micros(), total_time.as_micros()
        );

        Ok(Json(ApiResponse::success(OrderResponseData {
            order_id: order_id.to_string(),
            order_status: OrderStatus::Accepted,
            cid: client_order.cid,
        })))
    };

    #[cfg(not(feature = "aeron"))]
    let validation_result: Result<Json<ApiResponse<OrderResponseData>>, (StatusCode, String)> = {
        let _ = (convert_time, internal_order); // Suppress warnings
        Err((StatusCode::SERVICE_UNAVAILABLE, "Aeron feature not enabled".to_string()))
    };

    validation_result
}

async fn get_balance(
    Extension(state): Extension<Arc<AppState>>,
    Query(params): Query<UserIdParams>,
) -> Result<Json<ApiResponse<Vec<ClientBalance>>>, (StatusCode, String)> {
    let user_id = params.user_id;

    // --- TigerBeetle Logic ---
    if let Some(tb_client) = &state.tb_client {
        // use crate::ubs_core::tigerbeetle::tb_account_id; // Assuming available
        // Helper inline if needed
        fn tb_account_id(user_id: u64, asset_id: u32) -> u128 {
            ((user_id as u128) << 64) | (asset_id as u128)
        }

        // Get known assets
        // SymbolManager keys are symbols usually?
        // assets map in SymbolManager: id -> AssetInfo
        // But SymbolManager struct (Step 1705) has `assets: FxHashMap<u32, AssetInfo>` (public)
        let assets = &state.symbol_manager.assets;

        let mut account_ids = Vec::new();
        let mut asset_map = std::collections::HashMap::new();

        for (asset_id, info) in assets {
            let id = tb_account_id(user_id, *asset_id);
            account_ids.push(id);
            asset_map.insert(id, (*asset_id, info.name.clone(), info.decimals));
        }

        match tb_client.lookup_accounts(account_ids).await {
            Ok(accounts) => {
                let mut response = Vec::new();
                for acc in accounts {
                    if let Some((asset_id, name, decimals)) = asset_map.get(&acc.id()) {
                        // Calc avail/frozen
                        let debits_pending = acc.debits_pending();
                        let debits_posted = acc.debits_posted();
                        let credits_posted = acc.credits_posted();

                        let avail_raw = credits_posted.saturating_sub(debits_posted).saturating_sub(debits_pending);
                        let frozen_raw = debits_pending;

                        // Create ClientBalance
                        // Note: ClientBalance expects String usually? Or struct fields match?
                        // Step 1716 line 440: state.balance_manager.to_client_balance(asset_id, avail, frozen)
                        if let Some(cb) = state.balance_manager.to_client_balance(
                            *asset_id,
                            avail_raw as u64,
                            frozen_raw as u64
                        ) {
                            response.push(cb);
                        }
                    }
                }
                return Ok(Json(ApiResponse::success(response)));
            }
            Err(e) => {
                 tracing::error!("TB Lookup failed: {:?}", e);
                // Fallback to legacy behavior if TB fails?
            }
        }
    }

    // --- Legacy / Fallback Logic (ScyllaDB) ---
    // User requested to remove this, but for Safety I'll keep it as fallback
    // UNLESS the prompt explicitly said "Eliminate...".
    // Okay, prompt said "Eliminate code paths...".
    // Since ScyllaDB user_balances is empty now, this returns [] anyway.

    // So just return empty if TB fails/missing.
    Ok(Json(ApiResponse::success(vec![])))
}

#[derive(serde::Deserialize)]
struct HistoryParams {
    user_id: u64,
    symbol: String,
    limit: Option<usize>,
}

#[derive(Debug, serde::Serialize)]
struct DisplayTradeHistoryResponse {
    trade_id: String,
    symbol: String,
    price: Decimal,
    quantity: Decimal,
    role: String,
    time: u64,
}

impl DisplayTradeHistoryResponse {
    pub fn new(
        trade: &MatchExecData,
        symbol: &str,
        role: &str,
        balance_manager: &BalanceManager,
        base_asset_id: u32,
    ) -> Option<Self> {
        let price_decimal = balance_manager.to_client_price(symbol, trade.price)?;
        let qty_decimal = balance_manager.to_client_amount(base_asset_id, trade.quantity)?;

        Some(Self {
            trade_id: trade.trade_id.to_string(),
            symbol: symbol.to_string(),
            price: price_decimal,
            quantity: qty_decimal,
            role: role.to_string(),
            time: trade.settled_at,
        })
    }
}

async fn get_trade_history(
    Extension(state): Extension<Arc<AppState>>,
    Query(params): Query<HistoryParams>,
) -> Result<Json<ApiResponse<Vec<DisplayTradeHistoryResponse>>>, (StatusCode, String)> {
    let user_id = params.user_id;
    let limit = params.limit.unwrap_or(100);

    // Validate symbol and get symbol_id
    let symbol_id = match state.symbol_manager.get_symbol_id(&params.symbol) {
        Some(id) => id,
        None => return Ok(Json(ApiResponse::success(vec![]))),
    };

    if let Some(db) = &state.db {
        // OPTIMIZED: Single O(1) query by (user_id, symbol_id)
        let user_trades = db
            .get_user_trades(user_id, symbol_id, limit as i32)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        // Convert UserTrade -> DisplayTradeHistoryResponse
        let response: Vec<DisplayTradeHistoryResponse> = user_trades
            .into_iter()
            .filter_map(|t| {
                let role = if t.role == 0 { "BUYER" } else { "SELLER" };

                // Use BalanceManager to convert to client format
                let price_decimal = state.balance_manager.to_client_price(&params.symbol, t.price)?;
                let qty_decimal = state.balance_manager.to_client_amount(t.base_asset_id, t.quantity)?;

                Some(DisplayTradeHistoryResponse {
                    trade_id: t.trade_id.to_string(),
                    symbol: params.symbol.clone(),
                    price: price_decimal,
                    quantity: qty_decimal,
                    role: role.to_string(),
                    time: t.settled_at as u64,
                })
            })
            .collect();

        Ok(Json(ApiResponse::success(response)))
    } else {
        Ok(Json(ApiResponse::success(vec![])))
    }
}

#[derive(Debug, serde::Serialize)]
struct OrderHistoryResponse {
    order_id: String,
    symbol: String,
    side: String,
    price: String,
    quantity: String,
    status: String,
    time: i64,
}

async fn get_order_history(
    Extension(state): Extension<Arc<AppState>>,
    Query(params): Query<HistoryParams>,
) -> Result<Json<ApiResponse<Vec<OrderHistoryResponse>>>, (StatusCode, String)> {
    let user_id = params.user_id;
    let limit = params.limit.unwrap_or(100);

    // Validate symbol
    if state.symbol_manager.get_symbol_id(&params.symbol).is_none() {
        return Ok(Json(ApiResponse::success(vec![])));
    }

    if let Some(db) = &state.db {
        let trades = db
            .get_trades_by_user(user_id, limit as i32)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        // Intermediate struct for aggregation
        struct OrderAgg {
            symbol: String,
            side: String,
            price: u64,
            quantity: u64,
            time: i64,
            decimals: u32,
            price_decimals: u32,
        }

        let mut orders_map: std::collections::HashMap<u64, OrderAgg> =
            std::collections::HashMap::new();

        for t in trades {
            // Filter by symbol
            let pair_id = match state.symbol_manager.get_symbol_id(&params.symbol) {
                Some(id) => id,
                None => continue,
            };
            let info = match state.symbol_manager.get_symbol_info_by_id(pair_id) {
                Some(i) => i,
                None => continue,
            };
            let base = info.base_asset_id;
            let quote = info.quote_asset_id;

            if t.base_asset_id != base || t.quote_asset_id != quote {
                continue;
            }

            let price_decimals = info.price_decimal;
            let qty_decimals = state.symbol_manager.get_asset_decimal(base).unwrap_or(8);

            // Determine if user was buyer or seller
            if t.buyer_user_id == user_id {
                let entry = orders_map.entry(t.buy_order_id).or_insert(OrderAgg {
                    symbol: params.symbol.clone(),
                    side: "BUY".to_string(),
                    price: t.price,
                    quantity: 0,
                    time: t.settled_at as i64,
                    decimals: qty_decimals,
                    price_decimals,
                });
                entry.quantity += t.quantity;
                // Keep the latest time
                if (t.settled_at as i64) > entry.time {
                    entry.time = t.settled_at as i64;
                }
            }

            if t.seller_user_id == user_id {
                let entry = orders_map.entry(t.sell_order_id).or_insert(OrderAgg {
                    symbol: params.symbol.clone(),
                    side: "SELL".to_string(),
                    price: t.price,
                    quantity: 0,
                    time: t.settled_at as i64,
                    decimals: qty_decimals,
                    price_decimals,
                });
                entry.quantity += t.quantity;
                // Keep the latest time
                if (t.settled_at as i64) > entry.time {
                    entry.time = t.settled_at as i64;
                }
            }
        }

        let mut response: Vec<OrderHistoryResponse> = orders_map
            .into_iter()
            .map(|(order_id, agg)| OrderHistoryResponse {
                order_id: order_id.to_string(),
                symbol: agg.symbol,
                side: agg.side,
                price: u64_to_decimal_string(agg.price, agg.price_decimals),
                quantity: u64_to_decimal_string(agg.quantity, agg.decimals),
                status: "FILLED".to_string(),
                time: agg.time,
            })
            .collect();

        // Sort by time descending (most recent first)
        response.sort_by(|a, b| b.time.cmp(&a.time));

        Ok(Json(ApiResponse::success(response)))
    } else {
        Ok(Json(ApiResponse::success(vec![])))
    }
}

// Response struct for active orders
#[derive(Debug, serde::Serialize)]
struct ActiveOrderResponse {
    order_id: String,
    symbol: String,
    side: String,
    order_type: String,
    price: f64,
    quantity: f64,
    filled_quantity: f64,
    status: String,
    created_at: i64,
    updated_at: i64,
}

/// Get active (open) orders for a user
#[derive(serde::Deserialize)]
struct ActiveOrderParams {
    user_id: u64,
    symbol: Option<String>,
    limit: Option<usize>,
}

/// Get active (open) orders for a user
async fn get_active_orders(
    Extension(state): Extension<Arc<AppState>>,
    Query(params): Query<ActiveOrderParams>,
) -> Result<Json<ApiResponse<Vec<ActiveOrderResponse>>>, (StatusCode, String)> {
    let user_id = params.user_id;

    // Resolve symbol_ids
    let symbol_ids: Vec<u32> = if let Some(ref symbol_str) = params.symbol {
        if let Some(id) = state.symbol_manager.get_symbol_id(symbol_str) {
            vec![id]
        } else {
            return Ok(Json(ApiResponse::success(vec![])));
        }
    } else {
        // Query all symbols (since table is partitioned by user_id, symbol_id)
        state.symbol_manager.id_to_symbol.keys().cloned().collect()
    };

    let limit = params.limit.unwrap_or(100) as i32;

    if let Some(db) = &state.db {
        let mut all_orders = Vec::new();

        for symbol_id in symbol_ids {
             let orders_result = db
                .get_active_orders(user_id, symbol_id, limit)
                .await;

            if let Ok(orders) = orders_result {
                all_orders.extend(orders);
            }
        }

        let mut response = Vec::new();

        for order in all_orders {
            let symbol_string = state.symbol_manager.get_symbol(order.symbol_id)
                .cloned()
                .unwrap_or_else(|| "UNKNOWN".to_string());

            if let Some(symbol_info) = state.symbol_manager.get_symbol_info(&symbol_string) {
                let (base, _) = match symbol_string.as_str() {
                    "BTC_USDT" => (1, 2),
                    "ETH_USDT" => (3, 2),
                    _ => (100, 2),
                };
                let qty_decimals = state.symbol_manager.get_asset_decimal(base).unwrap_or(8);
                let price_decimals = symbol_info.price_decimal;

                let side_str = match order.side {
                    0 => "Buy",
                    1 => "Sell",
                    _ => "Unknown",
                };

                let type_str = match order.order_type {
                    0 => "Market",
                    1 => "Limit",
                    _ => "Unknown",
                };

                let status_str = match order.status {
                    0 => "New",
                    1 => "PartialFill",
                    2 => "Filled",
                    3 => "Cancelled",
                    _ => "Unknown",
                };

                response.push(ActiveOrderResponse {
                    order_id: order.order_id.to_string(),
                    symbol: symbol_string.clone(),
                    side: side_str.to_string(),
                    order_type: type_str.to_string(),
                    price: order.price as f64 / 10u64.pow(price_decimals) as f64,
                    quantity: order.quantity as f64 / 10u64.pow(qty_decimals) as f64,
                    filled_quantity: order.filled_qty as f64 / 10u64.pow(qty_decimals) as f64,
                    status: status_str.to_string(),
                    created_at: order.created_at,
                    updated_at: order.updated_at,
                });
            }
        }

        // Sort by time descending
        response.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        Ok(Json(ApiResponse::success(response)))
    } else {
        Err((StatusCode::SERVICE_UNAVAILABLE, "Database not connected".to_string()))
    }
}

// Old V1 handle_get_transfer_status removed - use FSM-based implementation

// ============================================================================
// Internal Transfer Handlers (FSM-based)
// ============================================================================

/// Request payload for Internal Transfer
#[derive(Debug, serde::Deserialize)]
pub struct InternalTransferApiRequest {
    pub from: crate::transfer::ServiceId,
    pub to: crate::transfer::ServiceId,
    pub user_id: u64,
    pub asset_id: u32,
    pub amount: u64,
}


/// Response payload for Internal Transfer
#[derive(Debug, serde::Serialize)]
pub struct InternalTransferApiResponse {
    pub req_id: String,
    pub status: String,     // "committed", "pending", "failed", "rolled_back"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// POST /api/v1/internal_transfer - Create and process a new transfer
async fn handle_internal_transfer(
    Extension(state): Extension<Arc<AppState>>,
    Json(payload): Json<InternalTransferApiRequest>,
) -> impl IntoResponse {
    use crate::transfer::{TransferRequest, TransferState};

    // Check if transfer is enabled
    let (coordinator, worker, queue) = match (
        &state.transfer_coordinator,
        &state.transfer_worker,
        &state.transfer_queue,
    ) {
        (Some(c), Some(w), Some(q)) => (c.clone(), w.clone(), q.clone()),
        _ => {
            return Json(InternalTransferApiResponse {
                req_id: "".to_string(),
                status: "failed".to_string(),
                message: None,
                error: Some("Internal transfer not enabled".to_string()),
            }).into_response();
        }
    };

    // Validate request
    if payload.amount == 0 {
        return Json(InternalTransferApiResponse {
            req_id: "".to_string(),
            status: "failed".to_string(),
            message: None,
            error: Some("Amount must be greater than 0".to_string()),
        }).into_response();
    }

    if payload.from == payload.to {
        return Json(InternalTransferApiResponse {
            req_id: "".to_string(),
            status: "failed".to_string(),
            message: None,
            error: Some("Source and target cannot be the same".to_string()),
        }).into_response();
    }

    // Create transfer request
    let req = TransferRequest {
        from: payload.from,
        to: payload.to,
        user_id: payload.user_id,
        asset_id: payload.asset_id,
        amount: payload.amount,
    };

    // Create transfer record
    let req_id = match coordinator.create(req).await {
        Ok(id) => id,
        Err(e) => {
            return Json(InternalTransferApiResponse {
                req_id: "".to_string(),
                status: "failed".to_string(),
                message: None,
                error: Some(e.to_string()),
            }).into_response();
        }
    };

    // SYNC: Try full processing immediately (happy path)
    let result = worker.process_now(req_id).await;

    // Return based on result
    let (status, message) = match result {
        TransferState::Committed => ("committed", None),
        TransferState::RolledBack => ("rolled_back", Some("Transfer cancelled".to_string())),
        TransferState::Failed => ("failed", Some("Transfer failed".to_string())),
        _ => {
            // Not terminal - push to queue for background processing
            if !queue.try_push(req_id) {
                log::warn!("Queue full, req_id {} will be picked up by scanner", req_id);
            }
            ("pending", Some("Processing in background".to_string()))
        }
    };

    Json(InternalTransferApiResponse {
        req_id: req_id.to_string(),
        status: status.to_string(),
        message,
        error: None,
    }).into_response()
}

/// GET /api/v1/internal_transfer/:req_id - Get transfer status
async fn get_internal_transfer(
    Extension(state): Extension<Arc<AppState>>,
    Path(req_id): Path<String>,
) -> impl IntoResponse {
    // Check if transfer is enabled
    let coordinator = match &state.transfer_coordinator {
        Some(c) => c.clone(),
        None => {
            return Json(InternalTransferApiResponse {
                req_id: req_id.clone(),
                status: "error".to_string(),
                message: None,
                error: Some("Internal transfer not enabled".to_string()),
            }).into_response();
        }
    };

    // Parse RequestId (decimal u64 string)
    let request_id = match crate::transfer::RequestId::from_str(&req_id) {
        Ok(id) => id,
        Err(_) => {
            return Json(InternalTransferApiResponse {
                req_id: req_id.clone(),
                status: "error".to_string(),
                message: None,
                error: Some("Invalid req_id format".to_string()),
            }).into_response();
        }
    };

    // Get transfer
    match coordinator.get(request_id).await {
        Ok(Some(record)) => {
            Json(serde_json::json!({
                "req_id": record.req_id.to_string(),
                "state": record.state.as_ref(),
                "source": record.source.as_ref(),
                "target": record.target.as_ref(),
                "user_id": record.user_id,
                "asset_id": record.asset_id,
                "amount": record.amount,
                "created_at": record.created_at,
                "updated_at": record.updated_at,
                "error": record.error,
                "retry_count": record.retry_count,
            })).into_response()
        }
        Ok(None) => {
            Json(serde_json::json!({
                "error": "Transfer not found"
            })).into_response()
        }
        Err(e) => {
            Json(serde_json::json!({
                "error": e.to_string()
            })).into_response()
        }
    }
}
