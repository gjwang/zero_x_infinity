//! WebSocket Service - Consumes push events and sends to clients
//!
//! This service runs in the Gateway's tokio runtime and consumes
//! PushEvent messages from the push_event_queue, formats them as
//! WsMessage, and sends them to connected clients via ConnectionManager.

use crossbeam_queue::ArrayQueue;
use rust_decimal::Decimal;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::interval;

use super::connection::ConnectionManager;
use super::messages::{PushEvent, WsMessage};
use crate::symbol_manager::SymbolManager;

/// Format internal u64 to display string with specified decimals
fn format_amount(value: u64, decimals: u32, display_decimals: u32) -> String {
    let decimal_value = Decimal::from(value) / Decimal::from(10u64.pow(decimals));
    format!("{:.prec$}", decimal_value, prec = display_decimals as usize)
}

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
                self.handle_event(event).await;
                count += 1;
                if count >= 1000 {
                    break;
                }
            }
        }
    }

    /// Handle a single push event
    async fn handle_event(&self, event: PushEvent) {
        // tracing::debug!(
        //     "[WsService] Handling event: {:?}",
        //     match &event {
        //         PushEvent::OrderUpdate {
        //             user_id, order_id, ..
        //         } => format!("OrderUpdate(user={}, order={})", user_id, order_id),
        //         PushEvent::Trade {
        //             user_id, trade_id, ..
        //         } => format!("Trade(user={}, trade={})", user_id, trade_id),
        //         PushEvent::BalanceUpdate {
        //             user_id, asset_id, ..
        //         } => format!("BalanceUpdate(user={}, asset={})", user_id, asset_id),
        //     }
        // );

        // match &event {
        //     PushEvent::OrderUpdate {
        //         order_id, user_id, ..
        //     } => tracing::info!(
        //         "[WsService] Order {}: WsService Picked Up (User {})",
        //         order_id,
        //         user_id
        //     ),
        //     _ => {}
        // }

        match event {
            PushEvent::OrderUpdate {
                user_id,
                order_id,
                symbol_id,
                status,
                filled_qty,
                avg_price,
            } => {
                // Resolve symbol info for name and decimals
                let symbol_info = self.symbol_mgr.get_symbol_info_by_id(symbol_id);
                let symbol_name = symbol_info
                    .map(|s| s.symbol.clone())
                    .unwrap_or_else(|| format!("SYMBOL_{}", symbol_id));

                // Get base asset decimals for qty formatting
                let (base_decimals, base_display_decimals) = symbol_info
                    .and_then(|s| {
                        let dec = self.symbol_mgr.get_asset_decimal(s.base_asset_id)?;
                        let disp = self
                            .symbol_mgr
                            .get_asset_display_decimals(s.base_asset_id)?;
                        Some((dec, disp))
                    })
                    .unwrap_or((8, 6));

                // Get price decimals
                let (price_decimals, price_display_decimals) = symbol_info
                    .map(|s| (s.price_decimal, s.price_display_decimal))
                    .unwrap_or((2, 2));

                let message = WsMessage::OrderUpdate {
                    order_id,
                    symbol: symbol_name,
                    status: format!("{:?}", status),
                    filled_qty: format_amount(filled_qty, base_decimals, base_display_decimals),
                    avg_price: avg_price
                        .map(|p| format_amount(p, price_decimals, price_display_decimals)),
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
                // Resolve symbol info for name and decimals
                let symbol_info = self.symbol_mgr.get_symbol_info_by_id(symbol_id);
                let symbol_name = symbol_info
                    .map(|s| s.symbol.clone())
                    .unwrap_or_else(|| format!("SYMBOL_{}", symbol_id));

                // Get base asset decimals for qty formatting
                let (base_decimals, base_display_decimals) = symbol_info
                    .and_then(|s| {
                        let dec = self.symbol_mgr.get_asset_decimal(s.base_asset_id)?;
                        let disp = self
                            .symbol_mgr
                            .get_asset_display_decimals(s.base_asset_id)?;
                        Some((dec, disp))
                    })
                    .unwrap_or((8, 6));

                // Get price decimals
                let (price_decimals, price_display_decimals) = symbol_info
                    .map(|s| (s.price_decimal, s.price_display_decimal))
                    .unwrap_or((2, 2));

                let message = WsMessage::Trade {
                    trade_id,
                    order_id,
                    symbol: symbol_name,
                    side: format!("{:?}", side),
                    price: format_amount(price, price_decimals, price_display_decimals),
                    qty: format_amount(qty, base_decimals, base_display_decimals),
                    role: if role == 0 { "MAKER" } else { "TAKER" }.to_string(),
                };
                self.manager.send_to_user(user_id, message);
                // NOTE: Trade persistence moved to SettlementService (correct architecture)
            }
            PushEvent::BalanceUpdate {
                user_id,
                asset_id,
                avail,
                frozen,
            } => {
                // Resolve asset name and decimals
                let asset_name = self
                    .symbol_mgr
                    .get_asset_name(asset_id)
                    .unwrap_or_else(|| format!("ASSET_{}", asset_id));

                let (asset_decimals, asset_display_decimals) = self
                    .symbol_mgr
                    .get_asset_decimal(asset_id)
                    .and_then(|dec| {
                        let disp = self.symbol_mgr.get_asset_display_decimals(asset_id)?;
                        Some((dec, disp))
                    })
                    .unwrap_or((6, 4));

                let message = WsMessage::BalanceUpdate {
                    asset: asset_name,
                    avail: format_amount(avail, asset_decimals, asset_display_decimals),
                    frozen: format_amount(frozen, asset_decimals, asset_display_decimals),
                };
                self.manager.send_to_user(user_id, message);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_amount_basic() {
        // 12345678 with 8 decimals = 0.12345678, display 4 decimals (truncated)
        assert_eq!(format_amount(12345678, 8, 4), "0.1234");

        // 100000000 with 8 decimals = 1.0, display 2 decimals
        assert_eq!(format_amount(100000000, 8, 2), "1.00");

        // Zero value
        assert_eq!(format_amount(0, 8, 2), "0.00");
    }

    #[test]
    fn test_format_amount_large_values() {
        // 1 BTC = 100_000_000 satoshi
        assert_eq!(format_amount(100_000_000, 8, 6), "1.000000");

        // 10 BTC
        assert_eq!(format_amount(1_000_000_000, 8, 6), "10.000000");

        // 0.001 BTC
        assert_eq!(format_amount(100_000, 8, 6), "0.001000");
    }

    #[test]
    fn test_format_amount_price() {
        // Price with 2 decimals: 85000.50 stored as 8500050
        assert_eq!(format_amount(8500050, 2, 2), "85000.50");

        // Price 30.00
        assert_eq!(format_amount(3000, 2, 2), "30.00");
    }
}
