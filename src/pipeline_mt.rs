//! Multi-Threaded Pipeline Runner
//!
//! This module implements a multi-threaded version of the pipeline,
//! using separate threads for different processing stages.
//!
//! # Thread Architecture
//!
//! ```text
//! ┌────────────┐     order_queue      ┌────────────┐     valid_order_queue
//! │  Thread 1  │ ──────────────────▶  │  Thread 2  │ ──────────────────────▶
//! │ Ingestion  │                      │  UBSCore   │
//! └────────────┘                      │ (Pre+Settle│ ◀───────────────────────
//!                                     └────────────┘     settle_request_queue
//!                                           │
//!                                           │ event_queue
//!                                           ▼
//! ┌────────────┐                      ┌────────────┐
//! │  Thread 3  │ ◀── trade_queue ──── │  Thread 4  │
//! │  Ledger    │     event_queue      │     ME     │
//! └────────────┘                      └────────────┘
//! ```

use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Instant;

use crate::csv_io::InputOrder;
use crate::engine::MatchingEngine;
use crate::ledger::LedgerWriter;
use crate::messages::{BalanceEvent, OrderEvent, TradeEvent};
use crate::models::{InternalOrder, OrderType, Side};
use crate::orderbook::OrderBook;
use crate::pipeline::{
    MultiThreadQueues, PipelineEvent, PipelineStats, PriceImprovement, SequencedOrder,
    SettleRequest, ShutdownSignal,
};
use crate::symbol_manager::SymbolManager;
use crate::ubscore::UBSCore;

// ============================================================
// MULTI-THREAD PIPELINE RESULT
// ============================================================

/// Result of multi-threaded pipeline execution
pub struct MultiThreadPipelineResult {
    pub accepted: u64,
    pub rejected: u64,
    pub total_trades: u64,
    pub stats: Arc<PipelineStats>,
}

// ============================================================
// CONFIGURATION
// ============================================================

/// Configuration for multi-threaded pipeline
pub struct MultiThreadConfig {
    /// Whether to spin-wait or yield when queue is empty
    pub spin_wait: bool,
    /// How many iterations to spin before yielding
    pub spin_iterations: u32,
}

impl Default for MultiThreadConfig {
    fn default() -> Self {
        Self {
            spin_wait: true,
            spin_iterations: 100,
        }
    }
}

// ============================================================
// MULTI-THREAD PIPELINE RUNNER
// ============================================================

/// Run orders through multi-threaded pipeline
///
/// # Arguments
/// * `orders` - Input orders to process
/// * `ubscore` - UBSCore instance (moved to UBSCore thread)
/// * `book` - OrderBook (moved to ME thread)
/// * `ledger` - LedgerWriter (moved to Ledger thread)
/// * `symbol_mgr` - Symbol manager
/// * `active_symbol_id` - Active trading pair
///
/// # Returns
/// Pipeline execution result with stats
pub fn run_pipeline_multi_thread(
    orders: Vec<InputOrder>,
    mut ubscore: UBSCore,
    mut book: OrderBook,
    mut ledger: LedgerWriter,
    symbol_mgr: &SymbolManager,
    active_symbol_id: u32,
) -> MultiThreadPipelineResult {
    let symbol_info = symbol_mgr
        .get_symbol_info_by_id(active_symbol_id)
        .expect("Active symbol not found");
    let qty_unit = symbol_info.qty_unit();
    let base_id = symbol_info.base_asset_id;
    let quote_id = symbol_info.quote_asset_id;

    // Create shared queues and state
    let queues = Arc::new(MultiThreadQueues::new());
    let stats = Arc::new(PipelineStats::new());
    let shutdown = Arc::new(ShutdownSignal::new());

    let start_time = Instant::now();

    // ================================================================
    // THREAD 1: Ingestion
    // ================================================================
    let ingestion_queues = queues.clone();
    let ingestion_stats = stats.clone();
    let ingestion_shutdown = shutdown.clone();
    let t1: JoinHandle<()> = thread::spawn(move || {
        let mut seq_counter = 0u64;

        for input in orders {
            if ingestion_shutdown.is_shutdown_requested() {
                break;
            }

            // Skip cancel orders for now (handle separately)
            if input.action == "cancel" {
                // TODO: Handle cancel in separate pathway
                ingestion_stats.incr_ingested();
                continue;
            }

            seq_counter += 1;
            let order = InternalOrder::new(
                input.order_id,
                input.user_id,
                active_symbol_id,
                input.price,
                input.qty,
                input.side,
            );

            let seq_order = SequencedOrder::new(seq_counter, order, 0);

            // Push with backpressure
            loop {
                match ingestion_queues.order_queue.push(seq_order.clone()) {
                    Ok(()) => break,
                    Err(_) => {
                        ingestion_stats.incr_backpressure();
                        std::hint::spin_loop();
                    }
                }
            }

            ingestion_stats.incr_ingested();
        }
    });

    // ================================================================
    // THREAD 2: UBSCore (Pre-Trade + Settlement)
    // ================================================================
    let ubscore_queues = queues.clone();
    let ubscore_stats = stats.clone();
    let ubscore_shutdown = shutdown.clone();
    let t2: JoinHandle<UBSCore> = thread::spawn(move || {
        let mut spin_count = 0u32;

        loop {
            let mut did_work = false;

            // Process incoming orders (Pre-Trade)
            if let Some(seq_order) = ubscore_queues.order_queue.pop() {
                did_work = true;

                let lock_asset_id = if seq_order.order.side == Side::Buy {
                    quote_id
                } else {
                    base_id
                };

                match ubscore.process_order(seq_order.order.clone()) {
                    Ok(valid_order) => {
                        // Send accepted event
                        let _ = ubscore_queues
                            .event_queue
                            .push(PipelineEvent::OrderAccepted {
                                seq_id: valid_order.seq_id,
                                order_id: seq_order.order.order_id,
                                user_id: seq_order.order.user_id,
                            });

                        // Send lock event
                        if let Some(b) = ubscore.get_balance(seq_order.order.user_id, lock_asset_id)
                        {
                            let lock_amount = if seq_order.order.side == Side::Buy {
                                seq_order.order.price * seq_order.order.qty / qty_unit
                            } else {
                                seq_order.order.qty
                            };
                            let _ = ubscore_queues
                                .event_queue
                                .push(PipelineEvent::BalanceLocked {
                                    user_id: seq_order.order.user_id,
                                    asset_id: lock_asset_id,
                                    seq_id: valid_order.seq_id,
                                    amount: lock_amount,
                                    version: b.lock_version(),
                                    avail_after: b.avail(),
                                    frozen_after: b.frozen(),
                                });
                        }

                        // Push to ME
                        loop {
                            match ubscore_queues.valid_order_queue.push(valid_order.clone()) {
                                Ok(()) => break,
                                Err(_) => {
                                    ubscore_stats.incr_backpressure();
                                    std::hint::spin_loop();
                                }
                            }
                        }

                        ubscore_stats.incr_accepted();
                    }
                    Err(reason) => {
                        // Send rejected event
                        let _ = ubscore_queues
                            .event_queue
                            .push(PipelineEvent::OrderRejected {
                                order_id: seq_order.order.order_id,
                                user_id: seq_order.order.user_id,
                                reason,
                            });
                        ubscore_stats.incr_rejected();
                    }
                }
            }

            // Process settlement requests
            if let Some(settle_req) = ubscore_queues.settle_request_queue.pop() {
                did_work = true;

                // Settle the trade
                if ubscore.settle_trade(&settle_req.trade_event).is_ok() {
                    // Send settlement events
                    let trade = &settle_req.trade_event.trade;
                    let trade_cost =
                        ((trade.price as u128) * (trade.qty as u128) / (qty_unit as u128)) as u64;

                    // Buyer spend quote
                    if let Some(b) = ubscore
                        .get_balance(trade.buyer_user_id, settle_req.trade_event.quote_asset_id)
                    {
                        let _ = ubscore_queues.event_queue.push(PipelineEvent::SettleSpend {
                            user_id: trade.buyer_user_id,
                            asset_id: settle_req.trade_event.quote_asset_id,
                            trade_id: trade.trade_id,
                            amount: trade_cost,
                            version: b.settle_version(),
                            avail_after: b.avail(),
                            frozen_after: b.frozen(),
                        });
                    }

                    // Buyer receive base
                    if let Some(b) = ubscore
                        .get_balance(trade.buyer_user_id, settle_req.trade_event.base_asset_id)
                    {
                        let _ = ubscore_queues
                            .event_queue
                            .push(PipelineEvent::SettleReceive {
                                user_id: trade.buyer_user_id,
                                asset_id: settle_req.trade_event.base_asset_id,
                                trade_id: trade.trade_id,
                                amount: trade.qty,
                                version: b.settle_version(),
                                avail_after: b.avail(),
                                frozen_after: b.frozen(),
                            });
                    }

                    // Seller spend base
                    if let Some(b) = ubscore
                        .get_balance(trade.seller_user_id, settle_req.trade_event.base_asset_id)
                    {
                        let _ = ubscore_queues.event_queue.push(PipelineEvent::SettleSpend {
                            user_id: trade.seller_user_id,
                            asset_id: settle_req.trade_event.base_asset_id,
                            trade_id: trade.trade_id,
                            amount: trade.qty,
                            version: b.settle_version(),
                            avail_after: b.avail(),
                            frozen_after: b.frozen(),
                        });
                    }

                    // Seller receive quote
                    if let Some(b) = ubscore
                        .get_balance(trade.seller_user_id, settle_req.trade_event.quote_asset_id)
                    {
                        let _ = ubscore_queues
                            .event_queue
                            .push(PipelineEvent::SettleReceive {
                                user_id: trade.seller_user_id,
                                asset_id: settle_req.trade_event.quote_asset_id,
                                trade_id: trade.trade_id,
                                amount: trade_cost,
                                version: b.settle_version(),
                                avail_after: b.avail(),
                                frozen_after: b.frozen(),
                            });
                    }

                    // Handle price improvement refund
                    if let Some(pi) = settle_req.price_improvement {
                        if let Ok(()) = ubscore
                            .accounts_mut()
                            .get_mut(&pi.user_id)
                            .unwrap()
                            .settle_unlock(pi.asset_id, pi.amount)
                        {
                            if let Some(b) = ubscore.get_balance(pi.user_id, pi.asset_id) {
                                let _ =
                                    ubscore_queues
                                        .event_queue
                                        .push(PipelineEvent::SettleRestore {
                                            user_id: pi.user_id,
                                            asset_id: pi.asset_id,
                                            trade_id: trade.trade_id,
                                            amount: pi.amount,
                                            version: b.settle_version(),
                                            avail_after: b.avail(),
                                            frozen_after: b.frozen(),
                                        });
                            }
                        }
                    }

                    ubscore_stats.incr_settled();
                }
            }

            // Check for shutdown
            if ubscore_shutdown.is_shutdown_requested()
                && ubscore_queues.order_queue.is_empty()
                && ubscore_queues.settle_request_queue.is_empty()
            {
                break;
            }

            // Spin/yield if no work
            if !did_work {
                spin_count += 1;
                if spin_count > 100 {
                    thread::yield_now();
                    spin_count = 0;
                } else {
                    std::hint::spin_loop();
                }
            } else {
                spin_count = 0;
            }
        }

        // Flush WAL before returning
        let _ = ubscore.flush_wal();
        ubscore
    });

    // ================================================================
    // THREAD 3: Matching Engine
    // ================================================================
    let me_queues = queues.clone();
    let me_stats = stats.clone();
    let me_shutdown = shutdown.clone();
    let t3: JoinHandle<OrderBook> = thread::spawn(move || {
        let mut spin_count = 0u32;

        loop {
            let mut did_work = false;

            if let Some(valid_order) = me_queues.valid_order_queue.pop() {
                did_work = true;

                // Match order
                let result = MatchingEngine::process_order(&mut book, valid_order.order.clone());

                // Send settle requests for each trade
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
                    );

                    // Calculate price improvement for buy orders
                    let price_improvement = if valid_order.order.side == Side::Buy
                        && valid_order.order.order_type == OrderType::Limit
                        && valid_order.order.price > trade.price
                    {
                        let diff = valid_order.order.price - trade.price;
                        let refund = diff * trade.qty / qty_unit;
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

                    let settle_req = SettleRequest {
                        trade_event,
                        price_improvement,
                    };

                    // Push to settle queue
                    loop {
                        match me_queues.settle_request_queue.push(settle_req.clone()) {
                            Ok(()) => break,
                            Err(_) => {
                                me_stats.incr_backpressure();
                                std::hint::spin_loop();
                            }
                        }
                    }

                    me_stats.add_trades(1);
                }

                // Send order fill event
                if result.order.filled_qty > 0 {
                    if result.order.remaining_qty() == 0 {
                        let _ = me_queues.event_queue.push(PipelineEvent::OrderFilled {
                            order_id: result.order.order_id,
                            user_id: result.order.user_id,
                            filled_qty: result.order.filled_qty,
                            avg_price: if result.trades.is_empty() {
                                0
                            } else {
                                result.trades.iter().map(|t| t.price).sum::<u64>()
                                    / result.trades.len() as u64
                            },
                        });
                    } else {
                        let _ = me_queues
                            .event_queue
                            .push(PipelineEvent::OrderPartialFilled {
                                order_id: result.order.order_id,
                                user_id: result.order.user_id,
                                filled_qty: result.order.filled_qty,
                                remaining_qty: result.order.remaining_qty(),
                            });
                    }
                }
            }

            // Check for shutdown
            if me_shutdown.is_shutdown_requested() && me_queues.valid_order_queue.is_empty() {
                break;
            }

            // Spin/yield if no work
            if !did_work {
                spin_count += 1;
                if spin_count > 100 {
                    thread::yield_now();
                    spin_count = 0;
                } else {
                    std::hint::spin_loop();
                }
            } else {
                spin_count = 0;
            }
        }

        book
    });

    // ================================================================
    // THREAD 4: Ledger Writer
    // ================================================================
    let ledger_queues = queues.clone();
    let ledger_shutdown = shutdown.clone();
    let t4: JoinHandle<LedgerWriter> = thread::spawn(move || {
        let mut spin_count = 0u32;

        loop {
            let mut did_work = false;

            if let Some(event) = ledger_queues.event_queue.pop() {
                did_work = true;

                match event {
                    PipelineEvent::Shutdown => break,
                    PipelineEvent::OrderAccepted {
                        seq_id,
                        order_id,
                        user_id,
                    } => {
                        ledger.write_order_event(&OrderEvent::Accepted {
                            seq_id,
                            order_id,
                            user_id,
                        });
                    }
                    PipelineEvent::OrderRejected {
                        order_id,
                        user_id,
                        reason,
                    } => {
                        ledger.write_order_event(&OrderEvent::Rejected {
                            seq_id: 0,
                            order_id,
                            user_id,
                            reason,
                        });
                    }
                    PipelineEvent::BalanceLocked {
                        user_id,
                        asset_id,
                        seq_id,
                        amount,
                        version,
                        avail_after,
                        frozen_after,
                    } => {
                        ledger.write_balance_event(&BalanceEvent::lock(
                            user_id,
                            asset_id,
                            seq_id,
                            amount,
                            version,
                            avail_after,
                            frozen_after,
                        ));
                    }
                    PipelineEvent::BalanceUnlocked {
                        user_id,
                        asset_id,
                        order_id,
                        amount,
                        version,
                        avail_after,
                        frozen_after,
                    } => {
                        ledger.write_balance_event(&BalanceEvent::unlock(
                            user_id,
                            asset_id,
                            order_id,
                            amount,
                            version,
                            avail_after,
                            frozen_after,
                        ));
                    }
                    PipelineEvent::OrderFilled {
                        order_id,
                        user_id,
                        filled_qty,
                        avg_price,
                    } => {
                        ledger.write_order_event(&OrderEvent::Filled {
                            order_id,
                            user_id,
                            filled_qty,
                            avg_price,
                        });
                    }
                    PipelineEvent::OrderPartialFilled {
                        order_id,
                        user_id,
                        filled_qty,
                        remaining_qty,
                    } => {
                        ledger.write_order_event(&OrderEvent::PartialFilled {
                            order_id,
                            user_id,
                            filled_qty,
                            remaining_qty,
                        });
                    }
                    PipelineEvent::OrderCancelled {
                        order_id,
                        user_id,
                        unfilled_qty,
                    } => {
                        ledger.write_order_event(&OrderEvent::Cancelled {
                            order_id,
                            user_id,
                            unfilled_qty,
                        });
                    }
                    PipelineEvent::SettleSpend {
                        user_id,
                        asset_id,
                        trade_id,
                        amount,
                        version,
                        avail_after,
                        frozen_after,
                    } => {
                        ledger.write_balance_event(&BalanceEvent::settle_spend(
                            user_id,
                            asset_id,
                            trade_id,
                            amount,
                            version,
                            avail_after,
                            frozen_after,
                        ));
                    }
                    PipelineEvent::SettleReceive {
                        user_id,
                        asset_id,
                        trade_id,
                        amount,
                        version,
                        avail_after,
                        frozen_after,
                    } => {
                        ledger.write_balance_event(&BalanceEvent::settle_receive(
                            user_id,
                            asset_id,
                            trade_id,
                            amount,
                            version,
                            avail_after,
                            frozen_after,
                        ));
                    }
                    PipelineEvent::SettleRestore {
                        user_id,
                        asset_id,
                        trade_id,
                        amount,
                        version,
                        avail_after,
                        frozen_after,
                    } => {
                        ledger.write_balance_event(&BalanceEvent::settle_restore(
                            user_id,
                            asset_id,
                            trade_id,
                            amount,
                            version,
                            avail_after,
                            frozen_after,
                        ));
                    }
                    PipelineEvent::LedgerEntry {
                        trade_id,
                        user_id,
                        asset_id,
                        op,
                        delta,
                        balance_after,
                    } => {
                        ledger.write_entry(&crate::ledger::LedgerEntry {
                            trade_id,
                            user_id,
                            asset_id,
                            op,
                            delta,
                            balance_after,
                        });
                    }
                    PipelineEvent::TradeExecuted { .. } => {
                        // Already handled via SettleSpend/SettleReceive
                    }
                }
            }

            // Check for shutdown (after event queue is drained)
            if ledger_shutdown.is_shutdown_requested() && ledger_queues.event_queue.is_empty() {
                break;
            }

            // Spin/yield if no work
            if !did_work {
                spin_count += 1;
                if spin_count > 100 {
                    thread::yield_now();
                    spin_count = 0;
                } else {
                    std::hint::spin_loop();
                }
            } else {
                spin_count = 0;
            }
        }

        ledger.flush();
        ledger
    });

    // ================================================================
    // Wait for completion
    // ================================================================

    // Wait for ingestion to complete
    t1.join().expect("Ingestion thread panicked");

    // Signal shutdown (will drain queues first)
    shutdown.request_shutdown();

    // Wait for all threads
    let _final_ubscore = t2.join().expect("UBSCore thread panicked");
    let _final_book = t3.join().expect("ME thread panicked");

    // Send shutdown to ledger
    let _ = queues.event_queue.push(PipelineEvent::Shutdown);
    let _final_ledger = t4.join().expect("Ledger thread panicked");

    let _elapsed = start_time.elapsed();

    MultiThreadPipelineResult {
        accepted: stats
            .orders_accepted
            .load(std::sync::atomic::Ordering::Relaxed),
        rejected: stats
            .orders_rejected
            .load(std::sync::atomic::Ordering::Relaxed),
        total_trades: stats
            .trades_generated
            .load(std::sync::atomic::Ordering::Relaxed),
        stats,
    }
}

// ============================================================
// TESTS
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_multi_thread_config_default() {
        let config = MultiThreadConfig::default();
        assert!(config.spin_wait);
        assert_eq!(config.spin_iterations, 100);
    }
}
