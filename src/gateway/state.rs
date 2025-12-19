use crossbeam_queue::ArrayQueue;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::persistence::TDengineClient;
use crate::pipeline::OrderAction;
use crate::symbol_manager::SymbolManager;
use crate::websocket::ConnectionManager;

/// Gateway 应用状态 (共享)
#[derive(Clone)]
pub struct AppState {
    /// 订单队列 (发送到 Trading Core)
    pub order_queue: Arc<ArrayQueue<OrderAction>>,
    /// Symbol Manager (只读)
    pub symbol_mgr: Arc<SymbolManager>,
    /// 活跃交易对 ID
    pub active_symbol_id: u32,
    /// 订单 ID 生成器
    order_id_gen: Arc<AtomicU64>,
    /// TDengine 客户端 (可选，用于查询)
    pub db_client: Option<Arc<TDengineClient>>,
    /// WebSocket 连接管理器
    pub ws_manager: Arc<ConnectionManager>,
}

impl AppState {
    pub fn new(
        order_queue: Arc<ArrayQueue<OrderAction>>,
        symbol_mgr: Arc<SymbolManager>,
        active_symbol_id: u32,
        db_client: Option<Arc<TDengineClient>>,
        ws_manager: Arc<ConnectionManager>,
    ) -> Self {
        Self {
            order_queue,
            symbol_mgr,
            active_symbol_id,
            order_id_gen: Arc::new(AtomicU64::new(1)),
            db_client,
            ws_manager,
        }
    }

    /// 生成下一个订单 ID
    pub fn next_order_id(&self) -> u64 {
        self.order_id_gen.fetch_add(1, Ordering::SeqCst)
    }
}
