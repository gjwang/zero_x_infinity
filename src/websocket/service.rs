//! WebSocket Service - Consumes push events and sends to clients
//!
//! This service runs in the Gateway's tokio runtime and consumes
//! PushEvent messages from the push_event_queue, formats them as
//! WsMessage, and sends them to connected clients via ConnectionManager.

use crossbeam_queue::ArrayQueue;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::interval;

use super::connection::ConnectionManager;
use super::messages::{PushEvent, WsMessage};
use crate::symbol_manager::SymbolManager;

/// WebSocket service for consuming push events and broadcasting to clients
pub struct WsService {
    /// Connection manager for sending messages to clients
    manager: Arc<ConnectionManager>,
    /// Queue of push events from Settlement
    push_event_queue: Arc<ArrayQueue<PushEvent>>,
    /// Symbol manager for name/decimal lookup
    symbol_mgr: Arc<SymbolManager>,
}

impl WsService {
    /// Create a new WsService
    pub fn new(
        manager: Arc<ConnectionManager>,
        push_event_queue: Arc<ArrayQueue<PushEvent>>,
        symbol_mgr: Arc<SymbolManager>,
    ) -> Self {
        Self {
            manager,
            push_event_queue,
            symbol_mgr,
        }
    }

    /// Run the service (consumes push events and sends to clients)
    ///
    /// This runs in a tokio task and continuously polls the push_event_queue.
    pub async fn run(self) {
        let mut tick = interval(Duration::from_millis(1));
        tracing::info!("[WsService] Started - polling push_event_queue");

        loop {
            tick.tick().await;

            // Batch process push events
            let mut count = 0;
            while let Some(event) = self.push_event_queue.pop() {
                eprintln!("[WsService] Popped event from queue: count={}", count + 1);
                self.handle_event(event);
                count += 1;
                if count >= 1000 {
                    break;
                }
            }
        }
    }

    /// Handle a single push event
    fn handle_event(&self, event: PushEvent) {
        tracing::debug!(
            "[WsService] Handling event: {:?}",
            match &event {
                PushEvent::OrderUpdate {
                    user_id, order_id, ..
                } => format!("OrderUpdate(user={}, order={})", user_id, order_id),
                PushEvent::Trade {
                    user_id, trade_id, ..
                } => format!("Trade(user={}, trade={})", user_id, trade_id),
                PushEvent::BalanceUpdate {
                    user_id, asset_id, ..
                } => format!("BalanceUpdate(user={}, asset={})", user_id, asset_id),
            }
        );

        match &event {
            PushEvent::OrderUpdate {
                order_id, user_id, ..
            } => tracing::info!(
                "[TRACE] Order {}: WsService Picked Up (User {})",
                order_id,
                user_id
            ),
            _ => {}
        }

        match event {
            PushEvent::OrderUpdate {
                user_id,
                order_id,
                symbol_id,
                status,
                filled_qty,
                avg_price,
            } => {
                // Resolve symbol name from SymbolManager
                let symbol_name = self
                    .symbol_mgr
                    .get_symbol_info_by_id(symbol_id)
                    .map(|s| s.symbol.clone())
                    .unwrap_or_else(|| format!("SYMBOL_{}", symbol_id));

                let message = WsMessage::OrderUpdate {
                    order_id,
                    symbol: symbol_name,
                    status: format!("{:?}", status),
                    filled_qty: filled_qty.to_string(),
                    avg_price: avg_price.map(|p| p.to_string()),
                };
                tracing::info!("[WsService] Sending to user {}", user_id);
                self.manager.send_to_user(user_id, message);
            }
            PushEvent::Trade {
                user_id,
                trade_id,
                order_id,
                symbol_id,
                side,
                price,
                qty,
                role,
            } => {
                // Resolve symbol name from SymbolManager
                let symbol_name = self
                    .symbol_mgr
                    .get_symbol_info_by_id(symbol_id)
                    .map(|s| s.symbol.clone())
                    .unwrap_or_else(|| format!("SYMBOL_{}", symbol_id));

                let message = WsMessage::Trade {
                    trade_id,
                    order_id,
                    symbol: symbol_name,
                    side: format!("{:?}", side),
                    price: price.to_string(),
                    qty: qty.to_string(),
                    role: if role == 0 { "MAKER" } else { "TAKER" }.to_string(),
                };
                self.manager.send_to_user(user_id, message);
            }
            PushEvent::BalanceUpdate {
                user_id,
                asset_id,
                avail,
                frozen,
            } => {
                // Resolve asset name from SymbolManager
                let asset_name = self
                    .symbol_mgr
                    .get_asset_name(asset_id)
                    .unwrap_or_else(|| format!("ASSET_{}", asset_id));

                let message = WsMessage::BalanceUpdate {
                    asset: asset_name,
                    avail: avail.to_string(),
                    frozen: frozen.to_string(),
                };
                self.manager.send_to_user(user_id, message);
            }
        }
    }
}
