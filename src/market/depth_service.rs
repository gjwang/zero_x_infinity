// Market depth service
//
// Consumes DepthSnapshot from ME and serves HTTP queries

use crate::messages::DepthSnapshot;
use crate::pipeline::MultiThreadQueues;
use crate::symbol_manager::SymbolManager;
use crate::websocket::{ConnectionManager, messages::WsMessage};
use rust_decimal::Decimal;
use rust_decimal::prelude::FromPrimitive;
use std::sync::Arc;
use std::sync::RwLock;

/// DepthService - consumes depth snapshots and serves queries
pub struct DepthService {
    /// Current depth snapshot
    current_snapshot: Arc<RwLock<DepthSnapshot>>,
    /// Queue to consume snapshots from
    queues: Arc<MultiThreadQueues>,
    /// WebSocket manager for broadcasting
    ws_manager: Option<Arc<ConnectionManager>>,
    /// Active symbol
    symbol: String,
    /// Formatting info
    price_decimals: u32,
    qty_decimals: u32,
    price_display_decimals: u32,
    qty_display_decimals: u32,
    quote_display_decimals: u32, // For completeness, though maybe unused here
}

impl DepthService {
    pub fn new(
        queues: Arc<MultiThreadQueues>,
        ws_manager: Option<Arc<ConnectionManager>>,
        symbol_mgr: Arc<SymbolManager>,
        active_symbol_id: u32,
    ) -> Self {
        let symbol_info = symbol_mgr
            .get_symbol_info_by_id(active_symbol_id)
            .expect("Active symbol not found");

        let price_display_decimals = symbol_info.price_display_decimal;
        let qty_display_decimals = symbol_mgr
            .get_asset_display_decimals(symbol_info.base_asset_id)
            .unwrap_or(6);
        let quote_display_decimals = symbol_mgr
            .get_asset_display_decimals(symbol_info.quote_asset_id)
            .unwrap_or(2);

        Self {
            current_snapshot: Arc::new(RwLock::new(DepthSnapshot::empty())),
            queues,
            ws_manager,
            symbol: symbol_info.symbol.clone(),
            price_decimals: symbol_info.price_decimal,
            qty_decimals: symbol_info.base_decimals,
            price_display_decimals,
            qty_display_decimals,
            quote_display_decimals,
        }
    }

    /// Run the service - consume snapshots from queue
    pub async fn run(&self) {
        let mut spin_count = 0u32;
        const IDLE_SPIN_LIMIT: u32 = 1000;

        loop {
            // Try to consume snapshot from queue
            if let Some(snapshot) = self.queues.depth_event_queue.pop() {
                // Update current snapshot
                if let Ok(mut current) = self.current_snapshot.write() {
                    *current = snapshot.clone();
                }

                // Broadcast via WebSocket
                if let Some(ws) = &self.ws_manager {
                    // Convert raw u64 to formatted strings
                    let factor_p = Decimal::from(10u64.pow(self.price_decimals));
                    let factor_q = Decimal::from(10u64.pow(self.qty_decimals));

                    let format_level = |level: &(u64, u64)| -> (String, String) {
                        let p = Decimal::from_u64(level.0).unwrap_or_default() / factor_p;
                        let q = Decimal::from_u64(level.1).unwrap_or_default() / factor_q;
                        (
                            format!("{:.prec$}", p, prec = self.price_display_decimals as usize),
                            format!("{:.prec$}", q, prec = self.qty_display_decimals as usize),
                        )
                    };

                    let bids: Vec<(String, String)> =
                        snapshot.bids.iter().take(20).map(format_level).collect();
                    let asks: Vec<(String, String)> =
                        snapshot.asks.iter().take(20).map(format_level).collect();

                    let msg = WsMessage::Depth {
                        event_type: "depthUpdate".to_string(),
                        event_time: chrono::Utc::now().timestamp_millis() as u64,
                        symbol: self.symbol.clone(),
                        update_id: snapshot.update_id,
                        bids,
                        asks,
                    };

                    ws.broadcast(&format!("market.depth.{}", self.symbol), msg);
                }

                spin_count = 0;
            } else {
                // No snapshot available, spin or yield
                spin_count += 1;
                if spin_count > IDLE_SPIN_LIMIT {
                    tokio::time::sleep(tokio::time::Duration::from_micros(100)).await;
                    spin_count = 0;
                } else {
                    std::hint::spin_loop();
                }
            }
        }
    }

    /// Get current snapshot for HTTP queries
    pub fn get_snapshot(&self, limit: usize) -> DepthSnapshot {
        let snapshot = self.current_snapshot.read().unwrap();

        // Limit the number of levels returned
        let bids: Vec<(u64, u64)> = snapshot.bids.iter().take(limit).copied().collect();
        let asks: Vec<(u64, u64)> = snapshot.asks.iter().take(limit).copied().collect();

        DepthSnapshot::new(bids, asks, snapshot.update_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_depth_service_get_snapshot() {
        let queues = Arc::new(MultiThreadQueues::new());

        // Setup dummy symbol manager
        let mut symbol_mgr = SymbolManager::new();
        symbol_mgr.add_asset(1, 8, 2, "BTC");
        symbol_mgr.add_asset(2, 6, 2, "USDT");
        symbol_mgr
            .insert_symbol_with_fees("BTC_USDT", 1, 1, 2, 2, 2, 0, 0)
            .unwrap();
        let symbol_mgr = Arc::new(symbol_mgr);

        let service = DepthService::new(queues.clone(), None, symbol_mgr, 1);

        // Initially empty
        let snapshot = service.get_snapshot(10);
        assert_eq!(snapshot.bids.len(), 0);
        assert_eq!(snapshot.asks.len(), 0);

        // Push a snapshot
        let test_snapshot = DepthSnapshot::new(
            vec![(30000, 100), (29900, 200), (29800, 300)],
            vec![(30100, 150), (30200, 250)],
            42,
        );
        queues.depth_event_queue.push(test_snapshot).unwrap();

        // Manually update (simulating what run() does)
        if let Some(snap) = queues.depth_event_queue.pop() {
            if let Ok(mut current) = service.current_snapshot.write() {
                *current = snap;
            }
        }

        // Now should have data
        let snapshot = service.get_snapshot(2);
        assert_eq!(snapshot.bids.len(), 2);
        assert_eq!(snapshot.asks.len(), 2);
        assert_eq!(snapshot.update_id, 42);
        assert_eq!(snapshot.bids[0], (30000, 100));
    }
}
