pub mod handlers;
pub mod state;
pub mod types;

use axum::{
    Router,
    routing::{get, post},
};
use std::sync::Arc;
use tokio::net::TcpListener;

use crate::symbol_manager::SymbolManager;
use crossbeam_queue::ArrayQueue;

use crate::pipeline::OrderAction;
use state::AppState;

/// 启动 HTTP Gateway 服务器
pub async fn run_server(
    port: u16,
    order_queue: Arc<ArrayQueue<OrderAction>>,
    symbol_mgr: Arc<SymbolManager>,
    active_symbol_id: u32,
) {
    // 创建共享状态 (暂时不连接数据库)
    let state = Arc::new(AppState::new(
        order_queue,
        symbol_mgr,
        active_symbol_id,
        None,
    ));

    // 创建路由
    let app = Router::new()
        // Write endpoints
        .route("/api/v1/create_order", post(handlers::create_order))
        .route("/api/v1/cancel_order", post(handlers::cancel_order))
        // Query endpoints (placeholder)
        .route("/api/v1/order/:order_id", get(handlers::get_order))
        .route("/api/v1/orders", get(handlers::get_orders))
        .route("/api/v1/trades", get(handlers::get_trades))
        .route("/api/v1/balances", get(handlers::get_balances))
        .with_state(state);

    // 绑定地址
    let addr = format!("0.0.0.0:{}", port);
    let listener = TcpListener::bind(&addr).await.unwrap();

    println!("🚀 Gateway listening on http://{}", addr);

    // 启动服务器
    axum::serve(listener, app).await.unwrap();
}
