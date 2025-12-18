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

use crate::pipeline::{
    BalanceUpdateRequest, MultiThreadQueues, OrderAction, PipelineConfig, PipelineServices,
    PipelineStats, PriceImprovement, SequencedOrder, ShutdownSignal, ValidAction,
};

/// Type alias for owned multi-threaded services
pub type MultiThreadServices = PipelineServices<UBSCore, OrderBook, LedgerWriter>;

/// Market context derived from symbol configuration
#[derive(Clone, Copy)]
pub struct MarketContext {
    pub qty_unit: u64,
    pub base_id: u32,
    pub quote_id: u32,
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
    services: MultiThreadServices,
    config: PipelineConfig,
) -> MultiThreadPipelineResult {
    let active_symbol_id = config.active_symbol_id;
    let sample_rate = config.sample_rate;

    let symbol_info = config
        .symbol_mgr
        .get_symbol_info_by_id(active_symbol_id)
        .expect("Active symbol not found");

    let market = MarketContext {
        qty_unit: symbol_info.qty_unit(),
        base_id: symbol_info.base_asset_id,
        quote_id: symbol_info.quote_asset_id,
    };

    // Create shared queues and state
    let queues = Arc::new(MultiThreadQueues::new());
    let stats = Arc::new(PipelineStats::new(sample_rate));
    let shutdown = Arc::new(ShutdownSignal::new());

    let _start_time = Instant::now();

    // The execution setup is now much clearer - spawn each processing stage
    let t1_ingestion = {
        let mut service = crate::pipeline_services::IngestionService::new(
            orders,
            queues.clone(),
            stats.clone(),
            active_symbol_id,
            _start_time,
        );
        let s = shutdown.clone();
        thread::spawn(move || service.run(&s))
    };

    let t2_ubscore = {
        let mut service = crate::pipeline_services::UBSCoreService::new(
            services.ubscore,
            queues.clone(),
            stats.clone(),
            _start_time,
        );
        let s = shutdown.clone();
        thread::spawn(move || {
            service.run(&s);
            service.into_inner()
        })
    };

    let t3_me = {
        let mut service = crate::pipeline_services::MatchingService::new(
            services.book,
            queues.clone(),
            stats.clone(),
            market,
        );
        let s = shutdown.clone();
        thread::spawn(move || {
            service.run(&s);
            service.into_inner()
        })
    };

    let t4_settlement = {
        let mut service = crate::pipeline_services::SettlementService::new(
            services.ledger,
            queues.clone(),
            stats.clone(),
        );
        let s = shutdown.clone();
        thread::spawn(move || {
            service.run(&s);
            service.into_inner()
        })
    };
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
// STAGE IMPLEMENTATIONS (Decomposed for Intent-Based Design)
// ============================================================

fn spawn_ingestion_stage(
    orders: Vec<InputOrder>,
    queues: Arc<MultiThreadQueues>,
    stats: Arc<PipelineStats>,
    shutdown: Arc<ShutdownSignal>,
    active_symbol_id: u32,
    start_time: Instant,
) -> JoinHandle<()> {
    thread::spawn(move || {
        let mut seq_counter = 0u64;

        for input in orders {
            if shutdown.is_shutdown_requested() {
                break;
            }

            let ingested_at_ns = Instant::now().duration_since(start_time).as_nanos() as u64;

            // Create OrderAction based on input action type
            let action = if input.action == ACTION_CANCEL {
                // Cancel order - no seq needed, just pass order_id
                stats.incr_cancel();
                stats.record_cancel();
                OrderAction::Cancel {
                    order_id: input.order_id,
                    user_id: input.user_id,
                    ingested_at_ns,
                }
            } else {
                // Place order - assign sequence number
                stats.incr_place();
                stats.record_place();
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
                match queues.order_queue.push(action.clone()) {
                    Ok(()) => break,
                    Err(_) => {
                        stats.incr_backpressure();
                        std::hint::spin_loop();
                    }
                }
            }

            stats.incr_ingested();
        }
    })
}

fn spawn_ubscore_stage(
    mut ubscore: UBSCore,
    queues: Arc<MultiThreadQueues>,
    stats: Arc<PipelineStats>,
    shutdown: Arc<ShutdownSignal>,
    start_time: Instant,
) -> JoinHandle<UBSCore> {
    thread::spawn(move || {
        let mut spin_count = 0u32;
        let mut batch_actions: Vec<ValidAction> = Vec::with_capacity(UBSC_ORDER_BATCH);
        let mut batch_events = Vec::with_capacity(UBSC_ORDER_BATCH + UBSC_SETTLE_BATCH * 4);

        loop {
            let mut did_work = false;

            // PHASE 1: COLLECT (Batch accumulation of intentions)

            // 1.1 Process Settlements
            for _ in 0..UBSC_SETTLE_BATCH {
                if let Some(req) = queues.balance_update_queue.pop() {
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
                if let Some(action) = queues.order_queue.pop() {
                    did_work = true;
                    match ubscore.apply_order_action(action) {
                        Ok((actions, events)) => {
                            batch_actions.extend(actions);
                            batch_events.extend(events);
                            stats.incr_accepted();
                        }
                        Err(_) => stats.incr_rejected(),
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
                stats.add_settlement_time(commit_start.elapsed().as_nanos() as u64);

                // Publish results to downstream
                for action in batch_actions.drain(..) {
                    while let Err(_) = queues.action_queue.push(action.clone()) {
                        std::hint::spin_loop();
                        if shutdown.is_shutdown_requested() {
                            break;
                        }
                    }
                }

                for event in batch_events.drain(..) {
                    let lat_ingested = event.ingested_at_ns;
                    while let Err(_) = queues.balance_event_queue.push(event.clone()) {
                        std::hint::spin_loop();
                        stats.incr_backpressure();
                        if shutdown.is_shutdown_requested() {
                            break;
                        }
                    }
                    if lat_ingested > 0 {
                        let now_ns = Instant::now().duration_since(start_time).as_nanos() as u64;
                        stats.record_latency(now_ns.saturating_sub(lat_ingested));
                    }
                    stats.incr_settled();
                }
            }

            if shutdown.is_shutdown_requested()
                && queues.order_queue.is_empty()
                && queues.balance_update_queue.is_empty()
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
    })
}

fn spawn_me_stage(
    mut book: OrderBook,
    queues: Arc<MultiThreadQueues>,
    stats: Arc<PipelineStats>,
    shutdown: Arc<ShutdownSignal>,
    market: MarketContext,
) -> JoinHandle<OrderBook> {
    thread::spawn(move || {
        let mut spin_count = 0u32;

        loop {
            let mut did_work = false;

            if let Some(action) = queues.action_queue.pop() {
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
                                market.base_id,
                                market.quote_id,
                                market.qty_unit,
                                valid_order.ingested_at_ns,
                            );

                            // Calculate price improvement for buy orders
                            let price_improvement = if valid_order.order.side == Side::Buy
                                && valid_order.order.order_type == OrderType::Limit
                                && valid_order.order.price > trade.price
                            {
                                let diff = valid_order.order.price - trade.price;
                                let refund = diff * trade.qty / market.qty_unit;
                                if refund > 0 {
                                    Some(PriceImprovement {
                                        user_id: valid_order.order.user_id,
                                        asset_id: market.quote_id,
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
                                match queues.trade_queue.push(trade_event.clone()) {
                                    Ok(()) => break,
                                    Err(_) => {
                                        stats.incr_backpressure();
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
                                match queues.balance_update_queue.push(balance_update.clone()) {
                                    Ok(()) => break,
                                    Err(_) => {
                                        stats.incr_backpressure();
                                        std::hint::spin_loop();
                                    }
                                }
                            }

                            stats.add_trades(1);
                            stats.record_trades(1);
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
                                    temp_order.calculate_cost(market.qty_unit).unwrap_or(0);
                                let lock_asset_id = match cancelled_order.side {
                                    Side::Buy => market.quote_id,
                                    Side::Sell => market.base_id,
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
                                    match queues.balance_update_queue.push(cancel_update.clone()) {
                                        Ok(()) => break,
                                        Err(_) => {
                                            stats.incr_backpressure();
                                            std::hint::spin_loop();
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                stats.add_matching_time(task_start.elapsed().as_nanos() as u64);
            }

            // Check for shutdown
            if shutdown.is_shutdown_requested() && queues.action_queue.is_empty() {
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
    })
}

fn spawn_settlement_stage(
    mut ledger: LedgerWriter,
    queues: Arc<MultiThreadQueues>,
    stats: Arc<PipelineStats>,
    shutdown: Arc<ShutdownSignal>,
) -> JoinHandle<LedgerWriter> {
    thread::spawn(move || {
        let mut spin_count = 0u32;
        let mut balance_events_count = 0u64;

        loop {
            let mut did_work = false;

            // ============================================
            // PRIORITY 1: Balance Events from UBSCore
            // Drain balance_event_queue first
            // ============================================
            while let Some(balance_event) = queues.balance_event_queue.pop() {
                did_work = true;
                let task_start = Instant::now();
                balance_events_count += 1;

                // Persist balance event to event log
                ledger.write_balance_event(&balance_event);
                stats.add_event_log_time(task_start.elapsed().as_nanos() as u64);
            }

            // ============================================
            // PRIORITY 2: Trade Events from ME
            // ============================================
            if let Some(trade_event) = queues.trade_queue.pop() {
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
                    balance_after: 0,
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
                stats.add_event_log_time(task_start.elapsed().as_nanos() as u64);
            }

            // Check for shutdown
            if shutdown.is_shutdown_requested()
                && queues.trade_queue.is_empty()
                && queues.balance_event_queue.is_empty()
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

        if balance_events_count > 0 {
            let _ = balance_events_count;
        }

        ledger.flush();
        ledger
    })
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
