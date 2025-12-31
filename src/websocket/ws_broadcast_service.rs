//! WebSocket Service - Consumes push events and sends to clients
//!
//! This service runs in the Gateway's tokio runtime and consumes
//! PushEvent messages from the push_event_queue, formats them as
//! WsMessage, and sends them to connected clients via ConnectionManager.

use crossbeam_queue::ArrayQueue;
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::interval; // Needed for u64 conversion

use super::connection::ConnectionManager;
use super::messages::{PushEvent, WsMessage};
use crate::models::Side;
use crate::money;
use crate::symbol_manager::SymbolManager;

/// Format internal u64 to display string with specified decimals
/// Delegates to crate::money for unified implementation
#[inline]
fn format_amount(value: u64, decimals: u32, display_decimals: u32) -> String {
    money::format_amount(value, decimals, display_decimals)
}

#[derive(Debug, Clone)]
struct TickerState {
    open: Decimal,
    high: Decimal,
    low: Decimal,
    close: Decimal,
    volume: Decimal,
    quote_volume: Decimal,
}

pub struct WsService {
    /// Connection manager for sending messages to clients
    manager: Arc<ConnectionManager>,
    /// Queue of push events from Settlement
    push_event_queue: Arc<ArrayQueue<PushEvent>>,
    /// Symbol manager for name/decimal lookup
    symbol_mgr: Arc<SymbolManager>,
    /// In-memory state for 24h ticker (Session-based)
    ticker_states: HashMap<String, TickerState>,
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
            ticker_states: HashMap::new(),
        }
    }

    /// Run the service (consumes push events and sends to clients)
    ///
    /// This runs in a tokio task and continuously polls the push_event_queue.
    pub async fn run(mut self) {
        let mut tick = interval(Duration::from_millis(1));
        tracing::info!("[WsService] Started - polling push_event_queue");

        loop {
            tick.tick().await;

            // Batch process push events
            let mut count = 0;
            while let Some(event) = self.push_event_queue.pop() {
                // eprintln!("[WsService] Popped event from queue: count={}", count + 1);
                self.handle_event(event).await;
                count += 1;
                if count >= 1000 {
                    break;
                }
            }
        }
    }

    /// Handle a single push event
    async fn handle_event(&mut self, event: PushEvent) {
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
                let symbol_info = self
                    .symbol_mgr
                    .get_symbol_info_by_id(symbol_id)
                    .expect("Critical: Symbol info missing for order broadcast");
                let symbol_name = symbol_info.symbol.clone();

                // Get base asset decimals for qty formatting
                let base_internal_scale = self
                    .symbol_mgr
                    .get_asset_internal_scale(symbol_info.base_asset_id)
                    .expect("Critical: Base asset internal scale missing");
                let base_display_decimals = self
                    .symbol_mgr
                    .get_asset_precision(symbol_info.base_asset_id)
                    .expect("Critical: Base asset precision missing");

                // Get price decimals
                let price_decimals = symbol_info.price_scale();
                let price_display_decimals = symbol_info.price_precision();

                let message = WsMessage::OrderUpdate {
                    order_id,
                    symbol: symbol_name,
                    status: format!("{:?}", status),
                    filled_qty: format_amount(
                        filled_qty,
                        base_internal_scale,
                        base_display_decimals,
                    ),
                    avg_price: avg_price
                        .map(|p| format_amount(p, price_decimals, price_display_decimals)),
                };
                self.manager.send_to_user(Some(user_id), message);
            }
            PushEvent::Trade {
                user_id,
                trade_id,
                order_id,
                symbol_id,
                side,
                price,
                qty,
                fee,
                fee_asset_id,
                is_maker,
            } => {
                // Resolve symbol info for name and decimals
                // Resolve symbol info for name and decimals
                let symbol_info = self
                    .symbol_mgr
                    .get_symbol_info_by_id(symbol_id)
                    .expect("Critical: Symbol info missing for trade broadcast");
                let symbol_name = symbol_info.symbol.clone();

                // Get base asset decimals for qty formatting
                let base_internal_scale = self
                    .symbol_mgr
                    .get_asset_internal_scale(symbol_info.base_asset_id)
                    .expect("Critical: Base asset internal scale missing");
                let base_display_decimals = self
                    .symbol_mgr
                    .get_asset_precision(symbol_info.base_asset_id)
                    .expect("Critical: Base asset precision missing");

                // Get price decimals
                let price_decimals = symbol_info.price_scale();
                let price_display_decimals = symbol_info.price_precision();

                // Get fee asset info for formatting
                let fee_asset_name = self
                    .symbol_mgr
                    .get_asset_name(fee_asset_id)
                    .expect("Critical: Fee asset name missing");
                let fee_decimals = self
                    .symbol_mgr
                    .get_asset_internal_scale(fee_asset_id)
                    .expect("Critical: Fee asset internal scale missing");
                let fee_display_decimals = self
                    .symbol_mgr
                    .get_asset_precision(fee_asset_id)
                    .expect("Critical: Fee asset precision missing");

                let message = WsMessage::Trade {
                    trade_id,
                    order_id,
                    symbol: symbol_name.clone(),
                    side: format!("{:?}", side),
                    price: format_amount(price, price_decimals, price_display_decimals),
                    qty: format_amount(qty, base_internal_scale, base_display_decimals),
                    fee: format_amount(fee, fee_decimals, fee_display_decimals),
                    fee_asset: fee_asset_name,
                    role: if is_maker { "MAKER" } else { "TAKER" }.to_string(),
                };
                self.manager.send_to_user(Some(user_id), message);

                // PUBLIC BROADCAST
                // We broadcast only one event per trade (on the Maker side, or if we decide otherwise)
                // Since this event fires for both Buyer and Seller, we need a rule to avoid duplication.
                // However, matching engine emits two Trade events: one for Maker, one for Taker.
                // We'll pick one to trigger the public broadcast. Let's pick Maker (true) or fallback to Taker logic if needed.
                // Actually, the simplest for public feed is to emit it when `role == "MAKER"` or `is_maker == true`.
                // BUT wait: taker order matches immediately. Maker order sits there.
                // The trade happens at the moment of match.
                // Let's use is_maker=false (Taker) as the trigger because the trade happens due to the Taker action?
                // Or simply `is_maker`=true?
                // Let's stick to: "Broadcast on Maker Event" to ensure we have one distinct event.
                // OR: broadcast on Taker event. Taker is the aggressor.
                // Let's us `!is_maker` (TAKER) to trigger the public event.
                // No, wait, if a Taker fills 3 orders, we get 1 Taker event? No, 3 Taker events (partial fills).
                // So both approaches work. Let's choose !is_maker (Taker) to align with "new incoming order caused trade".
                // But wait, what if two market orders match? (Not possible in this engine).
                // Let's safe-guard: is_maker = true seems more passive.
                // Actually, let's look at the data we have. We have price/qty.
                // Let's use `!is_maker` (TAKER) as the trigger.

                if !is_maker {
                    // Calculate quote_qty for public feed
                    // quote_qty = price * qty theoretically.
                    // But we have u64 scaled values.
                    // price: 85000.00 (8500000), qty: 0.1 (10000000)
                    // Implementation in query logic was detailed. Here we just format.
                    // Let's replicate the logic or just do simple calc?
                    // WsMessage wants strings.
                    // Let's reuse format_amount.

                    // We need quote_decimals.
                    let quote_display_decimals = self
                        .symbol_mgr
                        .get_asset_precision(symbol_info.quote_asset_id)
                        .expect("Critical: Quote asset precision missing for public broadcast");

                    // Calculate quote_qty value: price * qty / 10^base_internal_scale?
                    // price (scaled by price_dec) * qty (scaled by base_dec)
                    // Result should be scaled by quote_dec?
                    // This is tricky without big math lib.
                    // But wait, we just need to display it.
                    // Let's approximate: P * Q.
                    // Actually, we can just omit quote_qty if it's too hard, OR do the math.
                    // price (e.g. 2 dec) * qty (e.g. 8 dec) = 10 decimals result.
                    // we want quote decimals (e.g. 6).
                    // So we need to adjust.

                    // Let's just forward the known formatted strings if possible?
                    // No, we only have u64 here.

                    // Shortcut: Use decimal lib if available or simple float math for display?
                    // We imported `rust_decimal::Decimal`. Let's use it.
                    // money-type-safety: use SymbolInfo's intent-based display APIs
                    // CRITICAL: Fail fast if symbol_info is missing, do not use hardcoded defaults
                    if let Some(symbol_info) = symbol_info {
                        // Intent-based APIs: hide price_unit/qty_unit details
                        let p_dec = symbol_info.price_as_decimal(price);
                        let q_dec = symbol_info.qty_as_decimal(qty);
                        let quote_val = symbol_info.format_quote_value(price, qty);
                        let quote_qty_str = format!(
                            "{:.prec$}",
                            quote_val,
                            prec = quote_display_decimals as usize
                        );

                        let public_msg = WsMessage::PublicTrade {
                            symbol: symbol_name.clone(),
                            price: format_amount(price, price_decimals, price_display_decimals),
                            qty: format_amount(qty, base_internal_scale, base_display_decimals),
                            quote_qty: quote_qty_str,
                            time: chrono::Utc::now().timestamp_millis(),
                            is_buyer_maker: if side == Side::Buy {
                                !is_maker
                            } else {
                                is_maker
                            },
                        };

                        let public_topic = format!("market.trade.{}", symbol_name);
                        self.manager.broadcast(&public_topic, public_msg);

                        // --- TICKER UPDATE (Mini Ticker) ---
                        let ticker = self
                            .ticker_states
                            .entry(symbol_name.clone())
                            .or_insert_with(|| TickerState {
                                open: p_dec,
                                high: p_dec,
                                low: p_dec,
                                close: p_dec,
                                volume: Decimal::new(0, 0),
                                quote_volume: Decimal::new(0, 0),
                            });

                        ticker.close = p_dec;
                        if p_dec > ticker.high {
                            ticker.high = p_dec;
                        }
                        if p_dec < ticker.low {
                            ticker.low = p_dec;
                        }
                        ticker.volume += q_dec;
                        ticker.quote_volume += quote_val;

                        let price_change = ticker.close - ticker.open;
                        let price_change_percent = if !ticker.open.is_zero() {
                            (price_change / ticker.open) * Decimal::from(100)
                        } else {
                            Decimal::from(0)
                        };

                        let ticker_msg = WsMessage::Ticker {
                            symbol: symbol_name.clone(),
                            price_change: format!(
                                "{:.prec$}",
                                price_change,
                                prec = price_display_decimals as usize
                            ),
                            price_change_percent: format!("{:.2}", price_change_percent),
                            last_price: format!(
                                "{:.prec$}",
                                ticker.close,
                                prec = price_display_decimals as usize
                            ),
                            high_price: format!(
                                "{:.prec$}",
                                ticker.high,
                                prec = price_display_decimals as usize
                            ),
                            low_price: format!(
                                "{:.prec$}",
                                ticker.low,
                                prec = price_display_decimals as usize
                            ),
                            volume: format!(
                                "{:.prec$}",
                                ticker.volume,
                                prec = base_display_decimals as usize
                            ),
                            quote_volume: format!(
                                "{:.prec$}",
                                ticker.quote_volume,
                                prec = quote_display_decimals as usize
                            ),
                            time: chrono::Utc::now().timestamp_millis() as u64,
                        };

                        self.manager
                            .broadcast(&format!("market.ticker.{}", symbol_name), ticker_msg);
                    } // end if let Some(symbol_info)
                } // end if !is_maker

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
                    .expect("Critical: Asset name missing for balance update");

                let asset_decimals = self
                    .symbol_mgr
                    .get_asset_internal_scale(asset_id)
                    .expect("Critical: Asset internal scale missing for balance update");
                let asset_display_decimals = self
                    .symbol_mgr
                    .get_asset_precision(asset_id)
                    .expect("Critical: Asset precision missing for balance update");

                let message = WsMessage::BalanceUpdate {
                    asset: asset_name,
                    avail: format_amount(avail, asset_decimals, asset_display_decimals),
                    frozen: format_amount(frozen, asset_decimals, asset_display_decimals),
                };
                self.manager.send_to_user(Some(user_id), message);
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
