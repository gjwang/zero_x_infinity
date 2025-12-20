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

/// 启动 HTTP Gateway 服务器
pub async fn run_server(
    port: u16,
    order_queue: Arc<ArrayQueue<OrderAction>>,
    symbol_mgr: Arc<SymbolManager>,
    active_symbol_id: u32,
    db_client: Option<Arc<TDengineClient>>,
    push_event_queue: Arc<ArrayQueue<crate::websocket::PushEvent>>,
    depth_service: Arc<DepthService>,
) {
    // 创建 WebSocket 连接管理器
    let ws_manager = Arc::new(ConnectionManager::new());

    // 启动 WebSocket 推送服务
    let ws_service = crate::websocket::WsService::new(
        ws_manager.clone(),
        push_event_queue,
        symbol_mgr.clone(),
        db_client.clone(),
        active_symbol_id,
    );
    tokio::spawn(async move {
        ws_service.run().await;
    });
    println!("📡 WebSocket push service started");

    // 创建共享状态
    let state = Arc::new(AppState::new(
        order_queue,
        symbol_mgr,
        active_symbol_id,
        db_client,
        ws_manager.clone(),
        depth_service,
    ));

    // 创建路由
    let app = Router::new()
        // WebSocket endpoint
        .route("/ws", get(ws_handler))
        // Health check (no internal details exposed)
        .route("/api/v1/health", get(handlers::health_check))
        // Write endpoints
        .route("/api/v1/create_order", post(handlers::create_order))
        .route("/api/v1/cancel_order", post(handlers::cancel_order))
        // Query endpoints
        .route("/api/v1/order/{order_id}", get(handlers::get_order))
        .route("/api/v1/orders", get(handlers::get_orders))
        .route("/api/v1/trades", get(handlers::get_trades))
        .route("/api/v1/balances", get(handlers::get_balances))
        .route("/api/v1/klines", get(handlers::get_klines))
        .route("/api/v1/depth", get(handlers::get_depth))
        .with_state(state);

    // 绑定地址
    let addr = format!("0.0.0.0:{}", port);
    let listener = TcpListener::bind(&addr).await.unwrap();

    println!("🚀 Gateway listening on http://{}", addr);
    println!("📡 WebSocket endpoint: ws://{}/ws", addr);

    // 启动服务器
    axum::serve(listener, app).await.unwrap();
}
