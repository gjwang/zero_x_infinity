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
use crate::messages::{BalanceEvent, TradeEvent};
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

            // Create OrderAction based on input action type
            let action = if input.action == "cancel" {
                // Cancel order - no seq needed, just pass order_id
                OrderAction::Cancel {
                    order_id: input.order_id,
                    user_id: input.user_id,
                }
            } else {
                // Place order - assign sequence number
                seq_counter += 1;
                let order = InternalOrder::new(
                    input.order_id,
                    input.user_id,
                    active_symbol_id,
                    input.price,
                    input.qty,
                    input.side,
                );
                OrderAction::Place(SequencedOrder::new(seq_counter, order, 0))
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

        // Helper to push balance event with backpressure
        fn push_balance_event(
            queues: &MultiThreadQueues,
            event: BalanceEvent,
            stats: &PipelineStats,
        ) {
            loop {
                match queues.balance_event_queue.push(event.clone()) {
                    Ok(()) => break,
                    Err(_) => {
                        stats.incr_backpressure();
                        std::hint::spin_loop();
                    }
                }
            }
        }

        loop {
            let mut did_work = false;

            // ============================================
            // Pre-Trade: Process incoming order actions
            // ============================================
            if let Some(order_action) = ubscore_queues.order_queue.pop() {
                did_work = true;

                match order_action {
                    OrderAction::Place(seq_order) => {
                        // Place order: lock balance, send to ME
                        let order = seq_order.order.clone();
                        let order_id = order.order_id;
                        let user_id = order.user_id;

                        match ubscore.process_order(order.clone()) {
                            Ok(valid_order) => {
                                // Generate Lock BalanceEvent
                                let lock_asset = if order.side == Side::Buy {
                                    quote_id
                                } else {
                                    base_id
                                };

                                if let Some(balance) = ubscore.get_balance(user_id, lock_asset) {
                                    let lock_amount = order.calculate_cost(qty_unit).unwrap_or(0);

                                    // Use messages::BalanceEvent::lock()
                                    let event = BalanceEvent::lock(
                                        user_id,
                                        lock_asset,
                                        order_id, // order_seq_id
                                        lock_amount,
                                        balance.lock_version(),
                                        balance.avail(),
                                        balance.frozen(),
                                    );

                                    push_balance_event(&ubscore_queues, event, &ubscore_stats);
                                }

                                // Push ValidAction::Order to ME
                                loop {
                                    match ubscore_queues
                                        .action_queue
                                        .push(ValidAction::Order(valid_order.clone()))
                                    {
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
                    OrderAction::Cancel { order_id, user_id } => {
                        // Cancel order: pass through to ME (no balance lock needed)
                        // ME will remove from book and return the cancelled order info
                        loop {
                            match ubscore_queues
                                .action_queue
                                .push(ValidAction::Cancel { order_id, user_id })
                            {
                                Ok(()) => break,
                                Err(_) => {
                                    ubscore_stats.incr_backpressure();
                                    std::hint::spin_loop();
                                }
                            }
                        }
                    }
                }
            }

            // ============================================
            // Post-Trade: Process balance updates from ME
            // ============================================
            if let Some(balance_update) = ubscore_queues.balance_update_queue.pop() {
                did_work = true;

                match balance_update {
                    BalanceUpdateRequest::Trade {
                        trade_event,
                        price_improvement,
                    } => {
                        let trade = &trade_event.trade;
                        let trade_id = trade.trade_id;
                        let quote_amount = trade_event.quote_amount();

                        // Execute balance update
                        if ubscore.settle_trade(&trade_event).is_ok() {
                            // Generate BalanceEvents for buyer
                            // Buyer: SpendFrozen(quote), Credit(base)
                            if let Some(balance) =
                                ubscore.get_balance(trade.buyer_user_id, trade_event.quote_asset_id)
                            {
                                push_balance_event(
                                    &ubscore_queues,
                                    BalanceEvent::settle_spend(
                                        trade.buyer_user_id,
                                        trade_event.quote_asset_id,
                                        trade_id,
                                        quote_amount,
                                        balance.settle_version(),
                                        balance.avail(),
                                        balance.frozen(),
                                    ),
                                    &ubscore_stats,
                                );
                            }
                            if let Some(balance) =
                                ubscore.get_balance(trade.buyer_user_id, trade_event.base_asset_id)
                            {
                                push_balance_event(
                                    &ubscore_queues,
                                    BalanceEvent::settle_receive(
                                        trade.buyer_user_id,
                                        trade_event.base_asset_id,
                                        trade_id,
                                        trade.qty,
                                        balance.settle_version(),
                                        balance.avail(),
                                        balance.frozen(),
                                    ),
                                    &ubscore_stats,
                                );
                            }

                            // Generate BalanceEvents for seller
                            // Seller: SpendFrozen(base), Credit(quote)
                            if let Some(balance) =
                                ubscore.get_balance(trade.seller_user_id, trade_event.base_asset_id)
                            {
                                push_balance_event(
                                    &ubscore_queues,
                                    BalanceEvent::settle_spend(
                                        trade.seller_user_id,
                                        trade_event.base_asset_id,
                                        trade_id,
                                        trade.qty,
                                        balance.settle_version(),
                                        balance.avail(),
                                        balance.frozen(),
                                    ),
                                    &ubscore_stats,
                                );
                            }
                            if let Some(balance) = ubscore
                                .get_balance(trade.seller_user_id, trade_event.quote_asset_id)
                            {
                                push_balance_event(
                                    &ubscore_queues,
                                    BalanceEvent::settle_receive(
                                        trade.seller_user_id,
                                        trade_event.quote_asset_id,
                                        trade_id,
                                        quote_amount,
                                        balance.settle_version(),
                                        balance.avail(),
                                        balance.frozen(),
                                    ),
                                    &ubscore_stats,
                                );
                            }

                            // Handle price improvement refund
                            if let Some(pi) = price_improvement {
                                if let Some(account) = ubscore.accounts_mut().get_mut(&pi.user_id) {
                                    if account.settle_unlock(pi.asset_id, pi.amount).is_ok() {
                                        if let Some(balance) =
                                            ubscore.get_balance(pi.user_id, pi.asset_id)
                                        {
                                            push_balance_event(
                                                &ubscore_queues,
                                                BalanceEvent::settle_restore(
                                                    pi.user_id,
                                                    pi.asset_id,
                                                    trade_id,
                                                    pi.amount,
                                                    balance.lock_version(),
                                                    balance.avail(),
                                                    balance.frozen(),
                                                ),
                                                &ubscore_stats,
                                            );
                                        }
                                    }
                                }
                            }
                            ubscore_stats.incr_settled();
                        }
                    }
                    BalanceUpdateRequest::Cancel {
                        order_id,
                        user_id,
                        asset_id,
                        unlock_amount,
                    } => {
                        // Unlock balance for cancelled order
                        if let Err(e) = ubscore.unlock(user_id, asset_id, unlock_amount) {
                            eprintln!("Cancel unlock failed for order {}: {}", order_id, e);
                        } else {
                            // Generate Unlock BalanceEvent
                            if let Some(balance) = ubscore.get_balance(user_id, asset_id) {
                                push_balance_event(
                                    &ubscore_queues,
                                    BalanceEvent::unlock(
                                        user_id,
                                        asset_id,
                                        order_id,
                                        unlock_amount,
                                        balance.lock_version(),
                                        balance.avail(),
                                        balance.frozen(),
                                    ),
                                    &ubscore_stats,
                                );
                            }
                        }
                    }
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

            if let Some(action) = me_queues.action_queue.pop() {
                did_work = true;

                match action {
                    ValidAction::Order(valid_order) => {
                        // Match order
                        let result =
                            MatchingEngine::process_order(&mut book, valid_order.order.clone());

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
                        }
                    }
                    ValidAction::Cancel { order_id, user_id } => {
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
            }

            // Check for shutdown
            if me_shutdown.is_shutdown_requested() && me_queues.action_queue.is_empty() {
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

            // Process Balance Events from UBSCore
            if let Some(balance_event) = settlement_queues.balance_event_queue.pop() {
                did_work = true;
                balance_events_count += 1;

                // Persist balance event to event log (if enabled)
                ledger.write_balance_event(&balance_event);
            }

            // Check for shutdown (after both queues are drained)
            if settlement_shutdown.is_shutdown_requested()
                && settlement_queues.trade_queue.is_empty()
                && settlement_queues.balance_event_queue.is_empty()
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
