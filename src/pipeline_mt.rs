//! Multi-Threaded Pipeline Runner
//!
//! This module implements a multi-threaded version of the pipeline,
//! following the architecture defined in 0x08-a Trading Pipeline Design.
//!
//! # Thread Architecture (from 0x08-a)
//!
//! ```text
//! Thread 1: Ingestion      Thread 2: UBSCore           Thread 3: ME          Thread 4: Settlement
//! ┌────────────┐           ┌────────────────────┐      ┌────────────┐        ┌────────────────┐
//! │ Read       │ order_q   │ Pre-Trade:         │ valid│ Match      │ trade_q│ Persist:       │
//! │ Orders     │ ────────▶ │   Write WAL        │ ───▶ │ Generate   │ ──────▶│   Trades       │
//! │            │           │   Lock Balance     │      │ Trades     │        │   Orders       │
//! └────────────┘           └─────────┬──────────┘      └──────┬─────┘        │   Ledger       │
//!                                    │                        │              │   Balances     │
//!                                    │                        │              └───────▲────────┘
//!                                    │ balance_update_queue   │                      │
//!                                    │ ◀──────────────────────┘                      │
//!                                    │                                               │
//!                                    ▼                                               │
//!                          ┌────────────────────┐  balance_event_queue (TODO)        │
//!                          │ Post-Trade:        │ ───────────────────────────────────┘
//!                          │   spend_frozen     │
//!                          │   deposit          │
//!                          │   generate events  │
//!                          └────────────────────┘
//! ```
//!
//! Key design: ME sends Trade Events to BOTH Settlement AND UBSCore in parallel (fan-out).
//!
//! TODO: UBSCore should send ALL Balance Events to Settlement via balance_event_queue:
//!   - Pre-Trade: Lock events
//!   - Post-Trade: SpendFrozen + Credit events  
//!   - Cancel/Reject: Unlock events
//!   - External: Deposit/Withdraw events

use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Instant;

use crate::csv_io::InputOrder;
use crate::engine::MatchingEngine;
use crate::ledger::{LedgerEntry, LedgerWriter};
use crate::messages::TradeEvent;
use crate::models::{InternalOrder, OrderType, Side};
use crate::orderbook::OrderBook;
use crate::pipeline::{
    BalanceUpdateRequest, MultiThreadQueues, PipelineStats, PriceImprovement, SequencedOrder,
    ShutdownSignal,
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
// MULTI-THREAD PIPELINE RUNNER
// ============================================================

/// Run orders through multi-threaded pipeline
///
/// Architecture from 0x08-a:
/// - Thread 1: Ingestion
/// - Thread 2: UBSCore (Pre-Trade: WAL + Lock) + (Post-Trade: Balance Update)
/// - Thread 3: ME (Match → Trade Events)
/// - Thread 4: Settlement (Persist Trade/Order/Ledger)
///
/// Fan-out: ME sends Trade Events to both Settlement and UBSCore in parallel.
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

    let _start_time = Instant::now();

    // ================================================================
    // THREAD 1: Ingestion
    // ================================================================
    let ingestion_queues = queues.clone();
    let ingestion_stats = stats.clone();
    let ingestion_shutdown = shutdown.clone();
    let t1_ingestion: JoinHandle<()> = thread::spawn(move || {
        let mut seq_counter = 0u64;

        for input in orders {
            if ingestion_shutdown.is_shutdown_requested() {
                break;
            }

            // Skip cancel orders for now
            if input.action == "cancel" {
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
    // THREAD 2: UBSCore (Pre-Trade + Post-Trade Balance Update)
    // ================================================================
    let ubscore_queues = queues.clone();
    let ubscore_stats = stats.clone();
    let ubscore_shutdown = shutdown.clone();
    let t2_ubscore: JoinHandle<UBSCore> = thread::spawn(move || {
        let mut spin_count = 0u32;

        loop {
            let mut did_work = false;

            // ============================================
            // Pre-Trade: Process incoming orders
            // ============================================
            if let Some(seq_order) = ubscore_queues.order_queue.pop() {
                did_work = true;

                match ubscore.process_order(seq_order.order.clone()) {
                    Ok(valid_order) => {
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
                    Err(_reason) => {
                        ubscore_stats.incr_rejected();
                    }
                }
            }

            // ============================================
            // Post-Trade: Process balance updates from ME
            // ============================================
            if let Some(balance_update) = ubscore_queues.balance_update_queue.pop() {
                did_work = true;

                // Execute balance update
                if ubscore.settle_trade(&balance_update.trade_event).is_ok() {
                    // Handle price improvement refund
                    if let Some(pi) = balance_update.price_improvement {
                        if let Some(account) = ubscore.accounts_mut().get_mut(&pi.user_id) {
                            let _ = account.settle_unlock(pi.asset_id, pi.amount);
                        }
                    }
                    ubscore_stats.incr_settled();
                }
            }

            // Check for shutdown
            if ubscore_shutdown.is_shutdown_requested()
                && ubscore_queues.order_queue.is_empty()
                && ubscore_queues.balance_update_queue.is_empty()
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

        let _ = ubscore.flush_wal();
        ubscore
    });

    // ================================================================
    // THREAD 3: Matching Engine
    // ================================================================
    let me_queues = queues.clone();
    let me_stats = stats.clone();
    let me_shutdown = shutdown.clone();
    let t3_me: JoinHandle<OrderBook> = thread::spawn(move || {
        let mut spin_count = 0u32;

        loop {
            let mut did_work = false;

            if let Some(valid_order) = me_queues.valid_order_queue.pop() {
                did_work = true;

                // Match order
                let result = MatchingEngine::process_order(&mut book, valid_order.order.clone());

                // Fan-out: Send Trade Events to BOTH Settlement AND UBSCore
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

                    // [1] Send to Settlement (for persistence) - trade_queue
                    loop {
                        match me_queues.trade_queue.push(trade_event.clone()) {
                            Ok(()) => break,
                            Err(_) => {
                                me_stats.incr_backpressure();
                                std::hint::spin_loop();
                            }
                        }
                    }

                    // [2] Send to UBSCore (for balance update) - balance_update_queue
                    let balance_update = BalanceUpdateRequest {
                        trade_event,
                        price_improvement,
                    };
                    loop {
                        match me_queues.balance_update_queue.push(balance_update.clone()) {
                            Ok(()) => break,
                            Err(_) => {
                                me_stats.incr_backpressure();
                                std::hint::spin_loop();
                            }
                        }
                    }

                    me_stats.add_trades(1);
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
    // THREAD 4: Settlement (Persist Trade Events, Order Events, Ledger)
    // ================================================================
    let settlement_queues = queues.clone();
    let settlement_shutdown = shutdown.clone();
    let t4_settlement: JoinHandle<LedgerWriter> = thread::spawn(move || {
        let mut spin_count = 0u32;

        loop {
            let mut did_work = false;

            // Process Trade Events from ME
            if let Some(trade_event) = settlement_queues.trade_queue.pop() {
                did_work = true;

                let trade = &trade_event.trade;
                let trade_cost = ((trade.price as u128) * (trade.qty as u128)
                    / (trade_event.qty_unit as u128)) as u64;

                // Persist to Ledger (legacy format)
                // Buyer: debit quote, credit base
                ledger.write_entry(&LedgerEntry {
                    trade_id: trade.trade_id,
                    user_id: trade.buyer_user_id,
                    asset_id: trade_event.quote_asset_id,
                    op: "debit",
                    delta: trade_cost,
                    balance_after: 0, // Not tracked in this format
                });
                ledger.write_entry(&LedgerEntry {
                    trade_id: trade.trade_id,
                    user_id: trade.buyer_user_id,
                    asset_id: trade_event.base_asset_id,
                    op: "credit",
                    delta: trade.qty,
                    balance_after: 0,
                });

                // Seller: debit base, credit quote
                ledger.write_entry(&LedgerEntry {
                    trade_id: trade.trade_id,
                    user_id: trade.seller_user_id,
                    asset_id: trade_event.base_asset_id,
                    op: "debit",
                    delta: trade.qty,
                    balance_after: 0,
                });
                ledger.write_entry(&LedgerEntry {
                    trade_id: trade.trade_id,
                    user_id: trade.seller_user_id,
                    asset_id: trade_event.quote_asset_id,
                    op: "credit",
                    delta: trade_cost,
                    balance_after: 0,
                });
            }

            // Check for shutdown (after trade queue is drained)
            if settlement_shutdown.is_shutdown_requested()
                && settlement_queues.trade_queue.is_empty()
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

        ledger.flush();
        ledger
    });

    // ================================================================
    // Wait for completion
    // ================================================================

    // Wait for ingestion to complete
    t1_ingestion.join().expect("Ingestion thread panicked");

    // Signal shutdown (will drain queues first)
    shutdown.request_shutdown();

    // Wait for all threads
    let _final_ubscore = t2_ubscore.join().expect("UBSCore thread panicked");
    let _final_book = t3_me.join().expect("ME thread panicked");
    let _final_ledger = t4_settlement.join().expect("Settlement thread panicked");

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
    #[test]
    fn test_placeholder() {
        // Placeholder for future tests
        assert!(true);
    }
}
