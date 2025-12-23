use crossbeam_queue::ArrayQueue;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::market::depth_service::DepthService;
use crate::persistence::TDengineClient;
use crate::pipeline::OrderAction;
use crate::symbol_manager::SymbolManager;
use crate::websocket::ConnectionManager;

// Phase 0x0A: Account management types
use crate::account::{Asset, Database, Symbol};

// Phase 0x0A-b: Authentication
use crate::api_auth::AuthState;

// Phase 0x0B-a: Internal Transfer FSM
use crate::transfer::TransferCoordinator;

/// Gateway application state (shared)
#[derive(Clone)]
pub struct AppState {
    /// Order queue (sends to Trading Core)
    pub order_queue: Arc<ArrayQueue<OrderAction>>,
    /// Symbol Manager (read-only)
    pub symbol_mgr: Arc<SymbolManager>,
    /// Active trading pair ID
    pub active_symbol_id: u32,
    /// Order ID generator
    order_id_gen: Arc<AtomicU64>,
    /// TDengine client (optional, for queries)
    pub db_client: Option<Arc<TDengineClient>>,
    /// WebSocket connection manager
    pub ws_manager: Arc<ConnectionManager>,
    /// DepthService (for market depth queries)
    pub depth_service: Arc<DepthService>,
    /// PostgreSQL database (Phase 0x0A)
    pub pg_db: Option<Arc<Database>>,
    /// Cached asset list (Phase 0x0A)
    pub pg_assets: Arc<Vec<Asset>>,
    /// Cached symbol list (Phase 0x0A)
    pub pg_symbols: Arc<Vec<Symbol>>,
    /// Authentication state (Phase 0x0A-b)
    pub auth_state: Arc<AuthState>,
    /// Transfer coordinator (Phase 0x0B-a) - optional until fully integrated
    pub transfer_coordinator: Option<Arc<TransferCoordinator>>,
}

impl AppState {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        order_queue: Arc<ArrayQueue<OrderAction>>,
        symbol_mgr: Arc<SymbolManager>,
        active_symbol_id: u32,
        db_client: Option<Arc<TDengineClient>>,
        ws_manager: Arc<ConnectionManager>,
        depth_service: Arc<DepthService>,
        pg_db: Option<Arc<Database>>,
        pg_assets: Arc<Vec<Asset>>,
        pg_symbols: Arc<Vec<Symbol>>,
        auth_state: Arc<AuthState>,
    ) -> Self {
        Self {
            order_queue,
            symbol_mgr,
            active_symbol_id,
            order_id_gen: Arc::new(AtomicU64::new(1)),
            db_client,
            ws_manager,
            depth_service,
            pg_db,
            pg_assets,
            pg_symbols,
            auth_state,
            transfer_coordinator: None, // Will be set when FSM is enabled
        }
    }

    /// Set the transfer coordinator (call after setup)
    pub fn with_transfer_coordinator(mut self, coordinator: Arc<TransferCoordinator>) -> Self {
        self.transfer_coordinator = Some(coordinator);
        self
    }

    /// Generate next order ID
    pub fn next_order_id(&self) -> u64 {
        self.order_id_gen.fetch_add(1, Ordering::SeqCst)
    }
}
