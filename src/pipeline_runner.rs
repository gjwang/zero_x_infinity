//! Pipeline Runner - Executes the pipeline processing loop
//!
//! This module implements the actual processing logic that uses the Ring Buffer
//! pipeline infrastructure to process orders through UBSCore → ME → Settlement.
//!
//! # Design
//!
//! The runner processes orders in a round-robin fashion:
//! 1. Pop from order_queue → UBSCore.process_order() → push to valid_order_queue
//! 2. Pop from valid_order_queue → ME.process_order() → push trades to trade_queue
//! 3. Pop from trade_queue → UBSCore.settle_trade() + Ledger.write()
//!
//! This single-threaded version validates correctness before multi-threading.

use std::time::Instant;

use crate::core_types::AssetId;
use crate::csv_io::InputOrder;
use crate::engine::MatchingEngine;
use crate::ledger::{LedgerEntry, LedgerWriter};
use crate::messages::{BalanceEvent, OrderEvent, TradeEvent};
use crate::models::{InternalOrder, OrderStatus, OrderType, Side};
use crate::orderbook::OrderBook;
use crate::perf::PerfMetrics;
use crate::pipeline::{PipelineStatsSnapshot, SingleThreadPipeline};
use crate::symbol_manager::SymbolManager;
use crate::ubscore::UBSCore;

// ============================================================
// PIPELINE RUNNER RESULT
// ============================================================

/// Result of pipeline execution
pub struct PipelineResult {
    pub accepted: u64,
    pub rejected: u64,
    pub total_trades: u64,
    pub perf: PerfMetrics,
    pub pipeline_stats: PipelineStatsSnapshot,
}

// ============================================================
// SINGLE-THREAD PIPELINE RUNNER
// ============================================================

/// Run orders through the pipeline in single-threaded mode
///
/// This validates the Ring Buffer communication pattern before
/// implementing multi-threaded version.
///
/// # Flow
/// ```text
/// 1. Ingest: orders → order_queue
/// 2. Process Loop:
///    - order_queue → UBSCore → valid_order_queue (or reject)
///    - valid_order_queue → ME → trade_queue
///    - trade_queue → Settlement + Balance Update
/// 3. Return stats
/// ```
pub fn run_pipeline_single_thread(
    orders: &[InputOrder],
    ubscore: &mut UBSCore,
    book: &mut OrderBook,
    ledger: &mut LedgerWriter,
    symbol_mgr: &SymbolManager,
    active_symbol_id: u32,
    sample_rate: usize,
) -> PipelineResult {
    let symbol_info = symbol_mgr
        .get_symbol_info_by_id(active_symbol_id)
        .expect("Active symbol not found");
    let qty_unit = symbol_info.qty_unit();
    let base_id = symbol_info.base_asset_id;
    let quote_id = symbol_info.quote_asset_id;

    let pipeline = SingleThreadPipeline::new(sample_rate);
    let mut perf = PerfMetrics::new(sample_rate);

    let mut accepted = 0u64;
    let mut rejected = 0u64;
    let mut total_trades = 0u64;

    // Process orders one by one through the pipeline
    // This single-threaded version validates correctness before multi-threading
    for (i, input) in orders.iter().enumerate() {
        // Progress report
        if (i + 1) % 100_000 == 0 {
            println!("Processed {} / {} orders...", i + 1, orders.len());
        }

        if input.action == "cancel" {
            // Cancel orders bypass the pipeline (no WAL or balance lock needed)
            handle_cancel_order(
                input, ubscore, book, ledger, base_id, quote_id, qty_unit, &mut perf,
            );
            pipeline.stats().incr_cancel();
            pipeline.stats().incr_ingested();
            continue;
        }

        let order_start = Instant::now();
        perf.inc_place();

        let order = InternalOrder::new(
            input.order_id,
            input.user_id,
            active_symbol_id,
            input.price,
            input.qty,
            input.side,
        );

        // Step 1: Ingest → order_queue → UBSCore → valid_order_queue
        let pretrade_start = Instant::now();

        let lock_asset_id = if order.side == Side::Buy {
            quote_id
        } else {
            base_id
        };
        let lock_amount = if order.side == Side::Buy {
            order.price * order.qty / qty_unit
        } else {
            order.qty
        };

        match ubscore.process_order(order.clone()) {
            Ok(valid_order) => {
                perf.add_pretrade_time(pretrade_start.elapsed().as_nanos() as u64);

                // Log order accepted + lock event
                let event_start = Instant::now();
                let accept_event = OrderEvent::Accepted {
                    seq_id: valid_order.seq_id,
                    order_id: input.order_id,
                    user_id: input.user_id,
                };
                ledger.write_order_event(&accept_event);

                if let Some(b) = ubscore.get_balance(input.user_id, lock_asset_id) {
                    let lock_event = BalanceEvent::lock(
                        input.user_id,
                        lock_asset_id,
                        valid_order.seq_id,
                        lock_amount,
                        b.lock_version(),
                        b.avail(),
                        b.frozen(),
                        0, // Use 0 for single-thread (tracked in local perf)
                    );
                    ledger.write_balance_event(&lock_event);
                }
                perf.add_event_log_time(event_start.elapsed().as_nanos() as u64);

                // Step 2: valid_order_queue → ME → trade_queue
                let match_start = Instant::now();
                let result = MatchingEngine::process_order(book, valid_order.order.clone());
                perf.add_matching_time(match_start.elapsed().as_nanos() as u64);

                // Process trades
                for trade in &result.trades {
                    let trade_event = TradeEvent::new(
                        trade.clone(),
                        if valid_order.order.side == Side::Buy {
                            trade.buyer_order_id
                        } else {
                            trade.seller_order_id
                        },
                        if valid_order.order.side == Side::Buy {
                            trade.seller_order_id
                        } else {
                            trade.buyer_order_id
                        },
                        valid_order.order.side,
                        base_id,
                        quote_id,
                        qty_unit,
                        0, // Use 0 for single-thread
                    );

                    // Step 3: trade_queue → Settlement + Balance Update
                    let settle_start = Instant::now();
                    if let Err(e) = ubscore.settle_trade(&trade_event) {
                        eprintln!("Trade settlement error: {}", e);
                    }
                    perf.add_settlement_time(settle_start.elapsed().as_nanos() as u64);

                    // Log settlement events
                    let event_start = Instant::now();
                    log_trade_settlement_events(&trade_event, ubscore, ledger, qty_unit);
                    perf.add_event_log_time(event_start.elapsed().as_nanos() as u64);

                    pipeline.stats().incr_settled();
                }

                // Handle price improvement refund for buy orders
                if valid_order.order.side == Side::Buy
                    && valid_order.order.order_type == OrderType::Limit
                {
                    for trade in &result.trades {
                        if valid_order.order.price > trade.price {
                            let diff = valid_order.order.price - trade.price;
                            let refund = diff * trade.qty / qty_unit;
                            if refund > 0 {
                                if let Ok(()) = ubscore
                                    .accounts_mut()
                                    .get_mut(&valid_order.order.user_id)
                                    .unwrap()
                                    .settle_unlock(quote_id, refund)
                                {
                                    if let Some(b) =
                                        ubscore.get_balance(valid_order.order.user_id, quote_id)
                                    {
                                        let restore_event = BalanceEvent::settle_restore(
                                            valid_order.order.user_id,
                                            quote_id,
                                            trade.trade_id,
                                            refund,
                                            b.settle_version(),
                                            b.avail(),
                                            b.frozen(),
                                            0, // Use 0 for single-thread
                                        );
                                        ledger.write_balance_event(&restore_event);
                                    }
                                }
                            }
                        }
                    }
                }

                // Log final order event
                let event_start = Instant::now();
                if result.order.filled_qty > 0 {
                    log_order_fill_event(&result.order, &result.trades, ledger);
                }
                perf.add_event_log_time(event_start.elapsed().as_nanos() as u64);

                pipeline.stats().add_trades(result.trades.len() as u64);
                total_trades += result.trades.len() as u64;
                perf.add_trades(result.trades.len() as u64);

                pipeline.stats().incr_accepted();
                accepted += 1;
            }
            Err(reason) => {
                perf.add_pretrade_time(pretrade_start.elapsed().as_nanos() as u64);

                // Log rejection
                let event_start = Instant::now();
                let reject_event = OrderEvent::Rejected {
                    seq_id: 0,
                    order_id: input.order_id,
                    user_id: input.user_id,
                    reason,
                };
                ledger.write_order_event(&reject_event);
                perf.add_event_log_time(event_start.elapsed().as_nanos() as u64);

                pipeline.stats().incr_rejected();
                rejected += 1;
            }
        }

        pipeline.stats().incr_place();
        pipeline.stats().incr_ingested();
        perf.add_order_latency(order_start.elapsed().as_nanos() as u64);
    }

    // Flush WAL and ledger
    if let Err(e) = ubscore.flush_wal() {
        eprintln!("WAL flush error: {}", e);
    }
    ledger.flush();

    PipelineResult {
        accepted,
        rejected,
        total_trades,
        perf,
        pipeline_stats: pipeline.get_stats(),
    }
}

// ============================================================
// HELPER FUNCTIONS
// ============================================================

/// Handle cancel order (bypasses pipeline)
fn handle_cancel_order(
    input: &InputOrder,
    ubscore: &mut UBSCore,
    book: &mut OrderBook,
    ledger: &mut LedgerWriter,
    base_id: AssetId,
    quote_id: AssetId,
    qty_unit: u64,
    perf: &mut PerfMetrics,
) {
    perf.inc_cancel();

    let pretrade_start = Instant::now();
    let cancelled_order_opt = book.remove_order_by_id(input.order_id);
    perf.add_cancel_lookup_time(pretrade_start.elapsed().as_nanos() as u64);
    perf.add_pretrade_time(pretrade_start.elapsed().as_nanos() as u64);

    if let Some(mut cancelled_order) = cancelled_order_opt {
        cancelled_order.status = OrderStatus::CANCELED;
        let remaining_qty = cancelled_order.remaining_qty();

        if remaining_qty > 0 {
            let mut temp_order = cancelled_order.clone();
            temp_order.qty = remaining_qty;
            let unlock_amount = temp_order.calculate_cost(qty_unit).unwrap_or(0);
            let lock_asset_id = match cancelled_order.side {
                Side::Buy => quote_id,
                Side::Sell => base_id,
            };

            // Unlock balance
            let settle_start = Instant::now();
            if let Err(e) = ubscore.unlock(cancelled_order.user_id, lock_asset_id, unlock_amount) {
                eprintln!("Cancel unlock failed: {}", e);
            }
            perf.add_settlement_time(settle_start.elapsed().as_nanos() as u64);

            // Log unlock event
            let event_start = Instant::now();
            if let Some(b) = ubscore.get_balance(cancelled_order.user_id, lock_asset_id) {
                let unlock_event = BalanceEvent::unlock(
                    cancelled_order.user_id,
                    lock_asset_id,
                    cancelled_order.order_id,
                    unlock_amount,
                    b.lock_version(),
                    b.avail(),
                    b.frozen(),
                    0, // Use 0 for single-thread
                );
                ledger.write_balance_event(&unlock_event);
            }
            perf.add_event_log_time(event_start.elapsed().as_nanos() as u64);
        }

        // Log order cancelled event
        let event_start = Instant::now();
        let order_event = OrderEvent::Cancelled {
            order_id: cancelled_order.order_id,
            user_id: cancelled_order.user_id,
            unfilled_qty: remaining_qty,
        };
        ledger.write_order_event(&order_event);
        perf.add_event_log_time(event_start.elapsed().as_nanos() as u64);
    }
}

/// Log order fill events
fn log_order_fill_event(
    order: &InternalOrder,
    trades: &[crate::models::Trade],
    ledger: &mut LedgerWriter,
) {
    let avg_price = if trades.is_empty() {
        0
    } else {
        let total_value: u128 = trades
            .iter()
            .map(|t| (t.price as u128) * (t.qty as u128))
            .sum();
        let total_qty: u128 = trades.iter().map(|t| t.qty as u128).sum();
        if total_qty > 0 {
            (total_value / total_qty) as u64
        } else {
            0
        }
    };

    let event = if order.remaining_qty() == 0 {
        OrderEvent::Filled {
            order_id: order.order_id,
            user_id: order.user_id,
            filled_qty: order.filled_qty,
            avg_price,
        }
    } else {
        OrderEvent::PartialFilled {
            order_id: order.order_id,
            user_id: order.user_id,
            filled_qty: order.filled_qty,
            remaining_qty: order.remaining_qty(),
        }
    };
    ledger.write_order_event(&event);
}

/// Log trade settlement balance events
fn log_trade_settlement_events(
    trade_event: &TradeEvent,
    ubscore: &UBSCore,
    ledger: &mut LedgerWriter,
    qty_unit: u64,
) {
    let trade = &trade_event.trade;
    let trade_cost = ((trade.price as u128) * (trade.qty as u128) / (qty_unit as u128)) as u64;

    // Buyer events
    if let Some(b) = ubscore.get_balance(trade.buyer_user_id, trade_event.quote_asset_id) {
        let settle_event = BalanceEvent::settle_spend(
            trade.buyer_user_id,
            trade_event.quote_asset_id,
            trade.trade_id,
            trade_cost,
            b.settle_version(),
            b.avail(),
            b.frozen(),
            0, // Use 0 for single-thread
        );
        ledger.write_balance_event(&settle_event);
        ledger.write_entry(&LedgerEntry {
            trade_id: trade.trade_id,
            user_id: trade.buyer_user_id,
            asset_id: trade_event.quote_asset_id,
            op: "debit",
            delta: trade_cost,
            balance_after: b.avail() + b.frozen(),
        });
    }
    if let Some(b) = ubscore.get_balance(trade.buyer_user_id, trade_event.base_asset_id) {
        let settle_event = BalanceEvent::settle_receive(
            trade.buyer_user_id,
            trade_event.base_asset_id,
            trade.trade_id,
            trade.qty,
            b.settle_version(),
            b.avail(),
            b.frozen(),
            0, // Use 0 for single-thread
        );
        ledger.write_balance_event(&settle_event);
        ledger.write_entry(&LedgerEntry {
            trade_id: trade.trade_id,
            user_id: trade.buyer_user_id,
            asset_id: trade_event.base_asset_id,
            op: "credit",
            delta: trade.qty,
            balance_after: b.avail() + b.frozen(),
        });
    }

    // Seller events
    if let Some(b) = ubscore.get_balance(trade.seller_user_id, trade_event.base_asset_id) {
        let settle_event = BalanceEvent::settle_spend(
            trade.seller_user_id,
            trade_event.base_asset_id,
            trade.trade_id,
            trade.qty,
            b.settle_version(),
            b.avail(),
            b.frozen(),
            0, // Use 0 for single-thread
        );
        ledger.write_balance_event(&settle_event);
        ledger.write_entry(&LedgerEntry {
            trade_id: trade.trade_id,
            user_id: trade.seller_user_id,
            asset_id: trade_event.base_asset_id,
            op: "debit",
            delta: trade.qty,
            balance_after: b.avail() + b.frozen(),
        });
    }
    if let Some(b) = ubscore.get_balance(trade.seller_user_id, trade_event.quote_asset_id) {
        let settle_event = BalanceEvent::settle_receive(
            trade.seller_user_id,
            trade_event.quote_asset_id,
            trade.trade_id,
            trade_cost,
            b.settle_version(),
            b.avail(),
            b.frozen(),
            0, // Use 0 for single-thread
        );
        ledger.write_balance_event(&settle_event);
        ledger.write_entry(&LedgerEntry {
            trade_id: trade.trade_id,
            user_id: trade.seller_user_id,
            asset_id: trade_event.quote_asset_id,
            op: "credit",
            delta: trade_cost,
            balance_after: b.avail() + b.frozen(),
        });
    }
}

// ============================================================
// TESTS
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pipeline_result_creation() {
        let result = PipelineResult {
            accepted: 100,
            rejected: 5,
            total_trades: 50,
            perf: PerfMetrics::new(10),
            pipeline_stats: crate::pipeline::PipelineStats::new().snapshot(),
        };

        assert_eq!(result.accepted, 100);
        assert_eq!(result.rejected, 5);
        assert_eq!(result.total_trades, 50);
    }
}
