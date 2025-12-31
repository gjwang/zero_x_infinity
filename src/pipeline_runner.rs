//! Pipeline Runner - Executes the pipeline processing loop
//!
//! This module implements the actual processing logic that uses the Ring Buffer
//! pipeline infrastructure to process orders through UBSCore → ME → Settlement.
//!
//! # Design
//!
//! The runner processes orders in a round-robin fashion:
//! 1. Pop from order_queue → UBSCore.process_order() → push to valid_order_queue
//! 2. Pop from valid_order_queue → ME.process_order() → push trades to me_result_queue
//! 3. Pop from me_result_queue → UBSCore.settle_trade() + Ledger.write()
//!
//! This single-threaded version validates correctness before multi-threading.

use std::time::Instant;

use crate::csv_io::{ACTION_PLACE, InputOrder};
use crate::engine::MatchingEngine;
use crate::ledger::LedgerWriter;
use crate::messages::{OrderEvent, TradeEvent};
use crate::models::{InternalOrder, Side};
use crate::orderbook::OrderBook;
use crate::perf::PerfMetrics;
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

use crate::pipeline::{
    BalanceUpdateRequest, OrderAction, PipelineConfig, PipelineServices, PipelineStatsSnapshot,
    PriceImprovement, SequencedOrder, SingleThreadPipeline, ValidAction,
};

/// Type alias for single-threaded services (using references)
pub type SingleThreadServices<'a> =
    PipelineServices<&'a mut UBSCore, &'a mut OrderBook, &'a mut LedgerWriter>;

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
///    - valid_order_queue → ME → me_result_queue
///    - me_result_queue → Settlement + Balance Update
/// 3. Return stats
/// ```
pub fn run_pipeline_single_thread(
    orders: &[InputOrder],
    services: SingleThreadServices,
    config: PipelineConfig,
) -> PipelineResult {
    let ubscore = services.ubscore;
    let book = services.book;
    let ledger = services.ledger;

    let symbol_info = config
        .symbol_mgr
        .get_symbol_info_by_id(config.active_symbol_id)
        .expect("Active symbol not found");
    let qty_unit = *symbol_info.qty_unit();
    let base_id = symbol_info.base_asset_id;
    let quote_id = symbol_info.quote_asset_id;
    let active_symbol_id = config.active_symbol_id;
    let sample_rate = config.sample_rate;

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

        let order_start = Instant::now();

        let _order = InternalOrder::new(
            input.order_id,
            input.user_id,
            active_symbol_id,
            input.price,
            input.qty,
            input.side,
        );
        let symbol_id = active_symbol_id;
        let seq_id = i as u64;
        let ingested_at_ns = order_start.elapsed().as_nanos() as u64;
        let pretrade_start = Instant::now();

        // Create action (Lean Ingestion)
        let action = if input.action == ACTION_PLACE {
            OrderAction::Place(SequencedOrder::new(
                seq_id,
                InternalOrder::new(
                    input.order_id,
                    input.user_id,
                    symbol_id,
                    input.price,
                    input.qty,
                    input.side,
                ),
                ingested_at_ns,
            ))
        } else {
            OrderAction::Cancel {
                order_id: input.order_id,
                user_id: input.user_id,
                ingested_at_ns,
            }
        };

        // Step 1: Ingest
        match ubscore.apply_order_action(action.clone()) {
            Ok((actions, events)) => {
                // Durability: Batch commit
                let _ = ubscore.commit();

                // Visibility: Log events
                for event in events {
                    ledger.write_balance_event(&event);
                }

                // Track Accepted
                pipeline.stats().incr_accepted();
                accepted += 1;

                // Step 2: ME and Settlement
                for act in actions {
                    match act {
                        ValidAction::Order(valid_order) => {
                            let match_start = Instant::now();
                            let result =
                                MatchingEngine::process_order(book, valid_order.order.clone());
                            perf.add_matching_time(match_start.elapsed().as_nanos() as u64);

                            // Log Accepted visibility
                            ledger.write_order_event(&OrderEvent::Accepted {
                                seq_id: valid_order.seq_id,
                                order_id: input.order_id,
                                user_id: input.user_id,
                            });

                            // Record trades to stats
                            pipeline.stats().add_trades(result.trades.len() as u64);
                            total_trades += result.trades.len() as u64;

                            // Process Settlements
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
                                    // Taker order state
                                    result.order.qty,
                                    result.order.filled_qty,
                                    // Maker order state (placeholder)
                                    0,
                                    0,
                                    base_id,
                                    quote_id,
                                    qty_unit, // Already dereferenced at line 79
                                    ingested_at_ns,
                                    active_symbol_id, // symbol_id for fee lookup
                                );

                                let pi = if valid_order.order.side == Side::Buy
                                    && valid_order.order.price > trade.price
                                {
                                    let diff = valid_order.order.price - trade.price;
                                    let refund = (diff as u128 * trade.qty as u128
                                        / qty_unit as u128) // qty_unit is u64
                                        as u64;
                                    if refund > 0 {
                                        Some(PriceImprovement {
                                            user_id: valid_order.order.user_id,
                                            asset_id: quote_id,
                                            amount: refund,
                                        })
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                };

                                let settle_req = BalanceUpdateRequest::Trade {
                                    trade_event,
                                    price_improvement: pi,
                                };
                                if let Ok(s_events) = ubscore.apply_balance_update(settle_req) {
                                    let _ = ubscore.commit();
                                    for e in s_events {
                                        ledger.write_balance_event(&e);
                                    }
                                    pipeline.stats().incr_settled();
                                }
                            }

                            if result.order.filled_qty > 0 {
                                log_order_fill_event(&result.order, &result.trades, ledger);
                            }
                        }
                        ValidAction::Cancel {
                            order_id,
                            user_id,
                            ingested_at_ns,
                        } => {
                            if let Some(cancelled_order) = book.remove_order_by_id(order_id) {
                                let rem = cancelled_order.remaining_qty();
                                if rem > 0 {
                                    let aid = if cancelled_order.side == Side::Buy {
                                        quote_id
                                    } else {
                                        base_id
                                    };

                                    // Use remaining_qty for unlock amount
                                    let mut tmp = cancelled_order.clone();
                                    tmp.qty = rem;
                                    let unlock_amount = tmp.calculate_cost(qty_unit).unwrap_or(0); // FIXME: FORBIDDEN - cost calculation must not default to 0

                                    let cancel_req = BalanceUpdateRequest::Cancel {
                                        order_id,
                                        user_id,
                                        asset_id: aid,
                                        unlock_amount,
                                        ingested_at_ns,
                                    };
                                    if let Ok(s_events) = ubscore.apply_balance_update(cancel_req) {
                                        let _ = ubscore.commit();
                                        for e in s_events {
                                            ledger.write_balance_event(&e);
                                        }
                                    }
                                }
                                ledger.write_order_event(&OrderEvent::Cancelled {
                                    order_id: cancelled_order.order_id,
                                    user_id: cancelled_order.user_id,
                                    unfilled_qty: rem,
                                });
                            }
                        }
                        ValidAction::Reduce { .. } | ValidAction::Move { .. } => {
                            // TODO: Implement for single-threaded pipeline
                        }
                    }
                }
            }
            Err(reason) => {
                pipeline.stats().incr_rejected();
                rejected += 1;
                ledger.write_order_event(&OrderEvent::Rejected {
                    seq_id: 0,
                    order_id: input.order_id,
                    user_id: input.user_id,
                    reason,
                });
            }
        }
        perf.add_pretrade_time(pretrade_start.elapsed().as_nanos() as u64);
        if input.action == ACTION_PLACE {
            perf.inc_place();
            pipeline.stats().record_place();
            pipeline.stats().incr_place();
        } else {
            perf.inc_cancel();
            pipeline.stats().record_cancel();
            pipeline.stats().incr_cancel();
        }
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
            pipeline_stats: crate::pipeline::PipelineStats::new(1).snapshot(),
        };

        assert_eq!(result.accepted, 100);
        assert_eq!(result.rejected, 5);
        assert_eq!(result.total_trades, 50);
    }
}
