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
use std::time::{Duration, Instant};

// High-frequency lifecycle logs are sent to hierarchical targets under "0XINFI"
// Example: "0XINFI::ME", "0XINFI::UBSC". This allows per-service toggling.
macro_rules! p_info {
    ($target:expr, $($arg:tt)+) => {
        tracing::info!(target: $target, $($arg)+);
    }
}

macro_rules! p_span {
    ($target:expr, $name:expr, $($arg:tt)*) => {
        tracing::info_span!(target: $target, $name, $($arg)*)
    }
}

use crate::csv_io::{ACTION_CANCEL, ACTION_PLACE, InputOrder};
use crate::engine::MatchingEngine;
use crate::ledger::{LedgerEntry, LedgerWriter, OP_CREDIT, OP_DEBIT};
use crate::messages::TradeEvent;
use crate::models::{InternalOrder, OrderStatus, OrderType, Side};
use crate::orderbook::OrderBook;
use crate::pipeline::{
    BalanceUpdateRequest, MultiThreadQueues, OrderAction, PipelineStats, PriceImprovement,
    SequencedOrder, ShutdownSignal, ValidAction,
};
use crate::symbol_manager::SymbolManager;
use crate::ubscore::UBSCore;
use crate::user_account::UserAccount;
use rustc_hash::FxHashMap;

// ============================================================
// LOGGING & PERFORMANCE CONSTANTS
// ============================================================

const TARGET_ME: &str = "0XINFI::ME";
const TARGET_PERS: &str = "0XINFI::PERS";

const LOG_ORDER: &str = "ORDER";
const LOG_CANCEL: &str = "CANCEL";
const LOG_TRADE: &str = "TRADE";

const IDLE_SPIN_LIMIT: u32 = 1000;
const IDLE_SLEEP_US: Duration = Duration::from_micros(100);

const UBSC_SETTLE_BATCH: usize = 128; // Max settlements per round
const UBSC_ORDER_BATCH: usize = 16; // Max new orders per round

// ============================================================
// MULTI-THREAD PIPELINE RESULT
// ============================================================

/// Result of multi-threaded pipeline execution
pub struct MultiThreadPipelineResult {
    pub accepted: u64,
    pub rejected: u64,
    pub total_trades: u64,
    pub stats: Arc<PipelineStats>,
    /// Final accounts after all processing (for verification)
    pub final_accounts: FxHashMap<u64, UserAccount>,
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
    sample_rate: usize,
) -> MultiThreadPipelineResult {
    let symbol_info = symbol_mgr
        .get_symbol_info_by_id(active_symbol_id)
        .expect("Active symbol not found");
    let qty_unit = symbol_info.qty_unit();
    let base_id = symbol_info.base_asset_id;
    let quote_id = symbol_info.quote_asset_id;

    // Create shared queues and state
    let queues = Arc::new(MultiThreadQueues::new());
    let stats = Arc::new(PipelineStats::new(sample_rate));
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

            let ingested_at_ns = Instant::now().duration_since(_start_time).as_nanos() as u64;

            // Create OrderAction based on input action type
            let action = if input.action == ACTION_CANCEL {
                // Cancel order - no seq needed, just pass order_id
                ingestion_stats.incr_cancel();
                ingestion_stats.record_cancel();
                OrderAction::Cancel {
                    order_id: input.order_id,
                    user_id: input.user_id,
                    ingested_at_ns,
                }
            } else {
                // Place order - assign sequence number
                ingestion_stats.incr_place();
                ingestion_stats.record_place();
                let _ = ACTION_PLACE; // Suppress unused warning if it happens
                seq_counter += 1;
                let mut order = InternalOrder::new_with_time(
                    input.order_id,
                    input.user_id,
                    active_symbol_id,
                    input.price,
                    input.qty,
                    input.side,
                    ingested_at_ns,
                );
                order.ingested_at_ns = ingested_at_ns;
                OrderAction::Place(SequencedOrder::new(seq_counter, order, ingested_at_ns))
            };

            // Push with backpressure (same path for both place and cancel)
            loop {
                match ingestion_queues.order_queue.push(action.clone()) {
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
        let mut batch_actions: Vec<ValidAction> = Vec::with_capacity(UBSC_ORDER_BATCH);
        let mut batch_events = Vec::with_capacity(UBSC_ORDER_BATCH + UBSC_SETTLE_BATCH * 4);

        loop {
            let mut did_work = false;

            // PHASE 1: COLLECT (Batch accumulation of intentions)

            // 1.1 Process Settlements
            for _ in 0..UBSC_SETTLE_BATCH {
                if let Some(req) = ubscore_queues.balance_update_queue.pop() {
                    did_work = true;
                    if let Ok(events) = ubscore.apply_balance_update(req) {
                        batch_events.extend(events);
                    }
                } else {
                    break;
                }
            }

            // 1.2 Process Orders
            for _ in 0..UBSC_ORDER_BATCH {
                if let Some(action) = ubscore_queues.order_queue.pop() {
                    did_work = true;
                    match ubscore.apply_order_action(action) {
                        Ok((actions, events)) => {
                            batch_actions.extend(actions);
                            batch_events.extend(events);
                            ubscore_stats.incr_accepted();
                        }
                        Err(_) => ubscore_stats.incr_rejected(),
                    }
                } else {
                    break;
                }
            }

            // PHASE 2 & 3: COMMIT & RELEASE (The actual atomic/visibility phase)
            if !batch_actions.is_empty() || !batch_events.is_empty() {
                // Batch Timing: Focus on the IO bottleneck (WAL Flush)
                let commit_start = Instant::now();
                if let Err(e) = ubscore.commit() {
                    tracing::error!("CRITICAL: WAL commit failed: {}", e);
                }
                ubscore_stats.add_settlement_time(commit_start.elapsed().as_nanos() as u64);

                // Publish results to downstream
                for action in batch_actions.drain(..) {
                    while let Err(_) = ubscore_queues.action_queue.push(action.clone()) {
                        std::hint::spin_loop();
                        if ubscore_shutdown.is_shutdown_requested() {
                            break;
                        }
                    }
                }

                for event in batch_events.drain(..) {
                    let lat_ingested = event.ingested_at_ns;
                    while let Err(_) = ubscore_queues.balance_event_queue.push(event.clone()) {
                        std::hint::spin_loop();
                        ubscore_stats.incr_backpressure();
                        if ubscore_shutdown.is_shutdown_requested() {
                            break;
                        }
                    }
                    if lat_ingested > 0 {
                        let now_ns = Instant::now().duration_since(_start_time).as_nanos() as u64;
                        ubscore_stats.record_latency(now_ns.saturating_sub(lat_ingested));
                    }
                    ubscore_stats.incr_settled();
                }
            }

            if ubscore_shutdown.is_shutdown_requested()
                && ubscore_queues.order_queue.is_empty()
                && ubscore_queues.balance_update_queue.is_empty()
            {
                break;
            }

            if !did_work {
                spin_count += 1;
                if spin_count > IDLE_SPIN_LIMIT {
                    thread::sleep(IDLE_SLEEP_US);
                    spin_count = 0;
                } else {
                    std::hint::spin_loop();
                }
            } else {
                spin_count = 0;
            }
        }
        let _ = ubscore.commit();
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

            if let Some(action) = me_queues.action_queue.pop() {
                did_work = true;
                let task_start = Instant::now();

                match action {
                    ValidAction::Order(valid_order) => {
                        let span =
                            p_span!(TARGET_ME, LOG_ORDER, order_id = valid_order.order.order_id);
                        let _enter = span.enter();

                        // Match order
                        let result =
                            MatchingEngine::process_order(&mut book, valid_order.order.clone());

                        p_info!(
                            TARGET_ME,
                            trades = result.trades.len(),
                            "Matching completed"
                        );

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
                                valid_order.ingested_at_ns,
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
                            let balance_update = BalanceUpdateRequest::Trade {
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
                            me_stats.record_trades(1);
                        }
                    }
                    ValidAction::Cancel {
                        order_id,
                        user_id,
                        ingested_at_ns,
                    } => {
                        let span = p_span!(
                            TARGET_ME,
                            LOG_CANCEL,
                            order_id = order_id,
                            user_id = user_id
                        );
                        let _enter = span.enter();
                        // Cancel order: remove from book
                        if let Some(mut cancelled_order) = book.remove_order_by_id(order_id) {
                            cancelled_order.status = OrderStatus::CANCELED;
                            let remaining_qty = cancelled_order.remaining_qty();

                            if remaining_qty > 0 {
                                // Calculate unlock amount
                                let mut temp_order = cancelled_order.clone();
                                temp_order.qty = remaining_qty;
                                let unlock_amount =
                                    temp_order.calculate_cost(qty_unit).unwrap_or(0);
                                let lock_asset_id = match cancelled_order.side {
                                    Side::Buy => quote_id,
                                    Side::Sell => base_id,
                                };

                                // Send cancel result to UBSCore for unlock
                                let cancel_update = BalanceUpdateRequest::Cancel {
                                    order_id,
                                    user_id,
                                    asset_id: lock_asset_id,
                                    unlock_amount,
                                    ingested_at_ns,
                                };
                                loop {
                                    match me_queues.balance_update_queue.push(cancel_update.clone())
                                    {
                                        Ok(()) => break,
                                        Err(_) => {
                                            me_stats.incr_backpressure();
                                            std::hint::spin_loop();
                                        }
                                    }
                                }
                            }
                        }
                        // If order not found in book, it may already be fully filled - silently ignore
                    }
                }
                me_stats.add_matching_time(task_start.elapsed().as_nanos() as u64);
            }

            // Check for shutdown
            if me_shutdown.is_shutdown_requested() && me_queues.action_queue.is_empty() {
                break;
            }

            if !did_work {
                spin_count += 1;
                if spin_count > IDLE_SPIN_LIMIT {
                    thread::sleep(IDLE_SLEEP_US);
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
    // THREAD 4: Settlement (Persist Trade Events, Balance Events, Ledger)
    // ================================================================
    let settlement_queues = queues.clone();
    let settlement_shutdown = shutdown.clone();
    let settlement_stats = stats.clone();
    let t4_settlement: JoinHandle<LedgerWriter> = thread::spawn(move || {
        let mut spin_count = 0u32;
        let mut balance_events_count = 0u64;

        loop {
            let mut did_work = false;

            // ============================================
            // PRIORITY 1: Balance Events from UBSCore
            // Drain balance_event_queue first (same priority pattern as UBSCore)
            // ============================================
            while let Some(balance_event) = settlement_queues.balance_event_queue.pop() {
                did_work = true;
                let task_start = Instant::now();
                balance_events_count += 1;

                // Persist balance event to event log
                ledger.write_balance_event(&balance_event);
                settlement_stats.add_event_log_time(task_start.elapsed().as_nanos() as u64);
            }

            // ============================================
            // PRIORITY 2: Trade Events from ME
            // ============================================
            if let Some(trade_event) = settlement_queues.trade_queue.pop() {
                did_work = true;
                let task_start = Instant::now();

                let trade = &trade_event.trade;
                let span = p_span!(
                    TARGET_PERS,
                    LOG_TRADE,
                    trade_id = trade.trade_id,
                    buyer = trade.buyer_user_id,
                    seller = trade.seller_user_id
                );
                let _enter = span.enter();

                let trade_cost = ((trade.price as u128) * (trade.qty as u128)
                    / (trade_event.qty_unit as u128)) as u64;

                // Persist to Ledger (legacy format)
                ledger.write_entry(&LedgerEntry {
                    trade_id: trade.trade_id,
                    user_id: trade.buyer_user_id,
                    asset_id: trade_event.quote_asset_id,
                    op: OP_DEBIT,
                    delta: trade_cost,
                    balance_after: 0, // Not tracked in this format
                });
                ledger.write_entry(&LedgerEntry {
                    trade_id: trade.trade_id,
                    user_id: trade.buyer_user_id,
                    asset_id: trade_event.base_asset_id,
                    op: OP_CREDIT,
                    delta: trade.qty,
                    balance_after: 0,
                });

                // Seller: debit base, credit quote
                ledger.write_entry(&LedgerEntry {
                    trade_id: trade.trade_id,
                    user_id: trade.seller_user_id,
                    asset_id: trade_event.base_asset_id,
                    op: OP_DEBIT,
                    delta: trade.qty,
                    balance_after: 0,
                });
                ledger.write_entry(&LedgerEntry {
                    trade_id: trade.trade_id,
                    user_id: trade.seller_user_id,
                    asset_id: trade_event.quote_asset_id,
                    op: OP_CREDIT,
                    delta: trade_cost,
                    balance_after: 0,
                });
                p_info!(TARGET_PERS, "Settlement persisted to ledger");
                settlement_stats.add_event_log_time(task_start.elapsed().as_nanos() as u64);
            }

            // Check for shutdown (after both queues are drained)
            if settlement_shutdown.is_shutdown_requested()
                && settlement_queues.trade_queue.is_empty()
                && settlement_queues.balance_event_queue.is_empty()
            {
                break;
            }

            if !did_work {
                spin_count += 1;
                if spin_count > IDLE_SPIN_LIMIT {
                    thread::sleep(IDLE_SLEEP_US);
                    spin_count = 0;
                } else {
                    std::hint::spin_loop();
                }
            } else {
                spin_count = 0;
            }
        }

        // Log balance events count (for debugging)
        if balance_events_count > 0 {
            // Could add to stats in future
            let _ = balance_events_count;
        }

        // Suppress unused warning for settlement_stats
        let _ = &settlement_stats;

        ledger.flush();
        ledger
    });

    // ================================================================
    // Wait for completion
    // ================================================================

    // Wait for ingestion to complete
    t1_ingestion.join().expect("Ingestion thread panicked");

    // Wait for all processing queues to drain before signaling shutdown
    // This ensures all orders are fully processed through the pipeline
    loop {
        if queues.all_empty() {
            break;
        }
        std::hint::spin_loop();
    }

    // Now signal shutdown (all threads should notice queues empty and exit)
    shutdown.request_shutdown();

    // Wait for all threads
    let final_ubscore = t2_ubscore.join().expect("UBSCore thread panicked");
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
        final_accounts: final_ubscore.accounts().clone(),
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
