pub mod handlers;
pub mod state;
pub mod types;

use axum::{
    Router,
    routing::{get, post},
};
use std::sync::Arc;
use tokio::net::TcpListener;

use crate::market::depth_service::DepthService;
use crate::symbol_manager::SymbolManager;
use crate::websocket::{ConnectionManager, ws_handler};
use crossbeam_queue::ArrayQueue;

use crate::persistence::TDengineClient;
use crate::pipeline::OrderAction;
use state::AppState;

// Phase 0x0A: Account management types
use crate::account::{Asset, Database, Symbol};

// Phase 0x0A-b: Authentication
use crate::auth::{AuthState, TsStore};

/// 启动 HTTP Gateway 服务器
#[allow(clippy::too_many_arguments)]
pub async fn run_server(
    port: u16,
    order_queue: Arc<ArrayQueue<OrderAction>>,
    symbol_mgr: Arc<SymbolManager>,
    active_symbol_id: u32,
    db_client: Option<Arc<TDengineClient>>,
    push_event_queue: Arc<ArrayQueue<crate::websocket::PushEvent>>,
    depth_service: Arc<DepthService>,
    pg_db: Option<Arc<Database>>,
    pg_assets: Arc<Vec<Asset>>,
    pg_symbols: Arc<Vec<Symbol>>,
) {
    // 创建 WebSocket 连接管理器
    let ws_manager = Arc::new(ConnectionManager::new());

    // 启动 WebSocket 推送服务
    let ws_service =
        crate::websocket::WsService::new(ws_manager.clone(), push_event_queue, symbol_mgr.clone());
    tokio::spawn(async move {
        ws_service.run().await;
    });
    println!("📡 WebSocket push service started");

    // 创建 Auth 状态 (Phase 0x0A-b)
    let auth_state = Arc::new(AuthState {
        ts_store: Arc::new(TsStore::new()),
        time_window_ms: 30_000, // 30 seconds
    });

    // 创建共享状态
    let state = Arc::new(AppState::new(
        order_queue,
        symbol_mgr,
        active_symbol_id,
        db_client,
        ws_manager.clone(),
        depth_service,
        pg_db.clone(),
        pg_assets,
        pg_symbols,
        auth_state,
    ));

    // ==========================================================================
    // Public Routes (无需鉴权)
    // ==========================================================================
    let public_routes = Router::new()
        // 行情数据
        .route("/exchange_info", get(handlers::get_exchange_info))
        .route("/assets", get(handlers::get_assets))
        .route("/symbols", get(handlers::get_symbols))
        .route("/depth", get(handlers::get_depth))
        .route("/klines", get(handlers::get_klines));

    // ==========================================================================
    // Private Routes (需要签名鉴权)
    // ==========================================================================
    // Note: Auth middleware will be added in Phase 2 once fully tested
    // For now, these routes are accessible but will require auth soon
    let private_routes = Router::new()
        // 账户查询
        .route("/orders", get(handlers::get_orders))
        .route("/order/{order_id}", get(handlers::get_order))
        .route("/trades", get(handlers::get_trades))
        .route("/balances", get(handlers::get_balances))
        // 交易操作
        .route("/order", post(handlers::create_order))
        .route("/cancel", post(handlers::cancel_order));
    // TODO: Apply auth middleware layer
    // .layer(from_fn_with_state(state.clone(), auth_middleware));

    // ==========================================================================
    // Legacy Routes (保持向后兼容)
    // ==========================================================================
    // These routes are deprecated but kept for backward compatibility
    let legacy_routes = Router::new()
        .route("/create_order", post(handlers::create_order))
        .route("/cancel_order", post(handlers::cancel_order))
        .route("/order/{order_id}", get(handlers::get_order))
        .route("/orders", get(handlers::get_orders))
        .route("/trades", get(handlers::get_trades))
        .route("/balances", get(handlers::get_balances))
        .route("/klines", get(handlers::get_klines))
        .route("/depth", get(handlers::get_depth))
        .route("/assets", get(handlers::get_assets))
        .route("/symbols", get(handlers::get_symbols))
        .route("/exchange_info", get(handlers::get_exchange_info));

    // 创建完整路由
    let app = Router::new()
        // WebSocket endpoint
        .route("/ws", get(ws_handler))
        // Health check
        .route("/api/v1/health", get(handlers::health_check))
        // New structured routes
        .nest("/api/v1/public", public_routes)
        .nest("/api/v1/private", private_routes)
        // Legacy routes (deprecated, will be removed in future)
        .nest("/api/v1", legacy_routes)
        .with_state(state);

    // 绑定地址
    let addr = format!("0.0.0.0:{}", port);
    let listener = TcpListener::bind(&addr).await.unwrap();

    println!("🚀 Gateway listening on http://{}", addr);
    println!("📡 WebSocket endpoint: ws://{}/ws", addr);
    println!("📂 Public API:  /api/v1/public/*");
    println!("🔒 Private API: /api/v1/private/* (auth pending)");

    // 启动服务器
    axum::serve(listener, app).await.unwrap();
}
