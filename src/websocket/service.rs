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

        loop {
            tick.tick().await;

            // Batch process push events
            let mut count = 0;
            while let Some(event) = self.push_event_queue.pop() {
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
        match event {
            PushEvent::OrderUpdate {
                user_id,
                order_id,
                symbol_id,
                status,
                filled_qty,
                avg_price,
            } => {
                // TODO: Format with symbol name and display decimals
                let message = WsMessage::OrderUpdate {
                    order_id,
                    symbol: format!("SYMBOL_{}", symbol_id), // Placeholder
                    status: format!("{:?}", status),
                    filled_qty: filled_qty.to_string(),
                    avg_price: avg_price.map(|p| p.to_string()),
                };
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
                // TODO: Format with symbol name and display decimals
                let message = WsMessage::Trade {
                    trade_id,
                    order_id,
                    symbol: format!("SYMBOL_{}", symbol_id), // Placeholder
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
                delta,
            } => {
                // TODO: Format with asset name and display decimals
                let message = WsMessage::BalanceUpdate {
                    asset: format!("ASSET_{}", asset_id), // Placeholder
                    avail: avail.to_string(),
                    frozen: frozen.to_string(),
                    delta: delta.to_string(),
                };
                self.manager.send_to_user(user_id, message);
            }
        }
    }
}
