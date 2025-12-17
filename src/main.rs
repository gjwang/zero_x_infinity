//! 0xInfinity - High-Frequency Trading Engine
//!
//! Chapter 8b: UBSCore Integration
//!
//! This is the main entry point. Architecture:
//!
//! ```text
//! ┌──────────┐    ┌──────────┐    ┌──────────┐    ┌──────────┐
//! │  Config  │───▶│ UBSCore  │───▶│  Engine  │───▶│  Output  │
//! │  (CSV)   │    │(WAL+Lock)│    │ (Match)  │    │  (CSV)   │
//! └──────────┘    └──────────┘    └──────────┘    └──────────┘
//!
//! UBSCore responsibilities:
//! - InternalOrder WAL (persistence first!)
//! - Balance Lock/Unlock/Settle
//! - Single-threaded atomic operations
//! ```

use std::fs::File;
use std::io::Write;
use std::time::Instant;

use rustc_hash::FxHashMap;
use zero_x_infinity::core_types::UserId;
use zero_x_infinity::csv_io::{
    InputOrder, chrono_lite_now, dump_balances, dump_orderbook_snapshot, load_balances_and_deposit,
    load_orders, load_symbol_manager,
};
use zero_x_infinity::engine::MatchingEngine;
use zero_x_infinity::ledger::{LedgerEntry, LedgerWriter};
use zero_x_infinity::messages::{BalanceEvent, OrderEvent, TradeEvent};
use zero_x_infinity::models::{InternalOrder, OrderStatus, OrderType, Side};
use zero_x_infinity::orderbook::OrderBook;
use zero_x_infinity::perf::PerfMetrics;
use zero_x_infinity::pipeline_mt::run_pipeline_multi_thread;
use zero_x_infinity::pipeline_runner::run_pipeline_single_thread;
use zero_x_infinity::symbol_manager::SymbolManager;
use zero_x_infinity::ubscore::UBSCore;
use zero_x_infinity::user_account::UserAccount;
use zero_x_infinity::wal::WalConfig;

// ============================================================
// OUTPUT DIRECTORY
// ============================================================

fn get_output_dir() -> &'static str {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--baseline") {
        "baseline/default"
    } else {
        "output"
    }
}

fn get_input_dir() -> String {
    let args: Vec<String> = std::env::args().collect();
    for i in 0..args.len() {
        if args[i] == "--input" && i + 1 < args.len() {
            return args[i + 1].clone();
        }
    }
    "fixtures".to_string()
}

// ============================================================
// ORDER EXECUTION
// ============================================================

fn execute_orders(
    orders: &[InputOrder],
    accounts: &mut FxHashMap<UserId, UserAccount>,
    book: &mut OrderBook,
    ledger: &mut LedgerWriter,
    symbol_mgr: &SymbolManager,
    active_symbol_id: u32,
) -> (u64, u64, u64, PerfMetrics) {
    let symbol_info = symbol_mgr
        .get_symbol_info_by_id(active_symbol_id)
        .expect("Active symbol not found");
    let qty_unit = symbol_info.qty_unit();
    let base_id = symbol_info.base_asset_id;
    let quote_id = symbol_info.quote_asset_id;

    let mut accepted = 0u64;
    let mut rejected = 0u64;
    let mut total_trades = 0u64;
    let mut perf = PerfMetrics::new(10); // Sample every 10th order

    for (i, input) in orders.iter().enumerate() {
        let order_start = Instant::now();

        // Progress every 100k orders
        if (i + 1) % 100_000 == 0 {
            println!("Processed {} / {} orders...", i + 1, orders.len());
        }

        // ========================================
        // STEP 1: Balance Check + Lock
        // ========================================
        let balance_check_start = Instant::now();

        let account = match accounts.get_mut(&input.user_id) {
            Some(acc) => acc,
            None => {
                rejected += 1;
                continue;
            }
        };

        let lock_result = match input.side {
            Side::Buy => {
                let cost = input.price * input.qty / qty_unit;
                account.get_balance_mut(quote_id).and_then(|b| b.lock(cost))
            }
            Side::Sell => account
                .get_balance_mut(base_id)
                .and_then(|b| b.lock(input.qty)),
        };

        perf.add_balance_check_time(balance_check_start.elapsed().as_nanos() as u64);

        if lock_result.is_err() {
            rejected += 1;
            continue;
        }

        // ========================================
        // STEP 2: Matching
        // ========================================
        let match_start = Instant::now();

        let order = InternalOrder::new(
            input.order_id,
            input.user_id,
            active_symbol_id,
            input.price,
            input.qty,
            input.side,
        );
        let result = MatchingEngine::process_order(book, order);

        perf.add_matching_time(match_start.elapsed().as_nanos() as u64);

        // ========================================
        // STEP 3: Settlement + Ledger per Trade
        // ========================================
        for trade in &result.trades {
            let trade_cost =
                ((trade.price as u128) * (trade.qty as u128) / (qty_unit as u128)) as u64;

            // Buyer settlement
            let settle_start = Instant::now();
            if let Some(buyer_acc) = accounts.get_mut(&trade.buyer_user_id) {
                let _ = buyer_acc.settle_as_buyer(quote_id, base_id, trade_cost, trade.qty, 0);
            }
            perf.add_settlement_time(settle_start.elapsed().as_nanos() as u64);

            // Buyer ledger
            let ledger_start = Instant::now();
            if let Some(buyer_acc) = accounts.get(&trade.buyer_user_id) {
                if let Some(b) = buyer_acc.get_balance(quote_id) {
                    ledger.write_entry(&LedgerEntry {
                        trade_id: trade.trade_id,
                        user_id: trade.buyer_user_id,
                        asset_id: quote_id,
                        op: "debit",
                        delta: trade_cost,
                        balance_after: b.avail() + b.frozen(),
                    });
                }
                if let Some(b) = buyer_acc.get_balance(base_id) {
                    ledger.write_entry(&LedgerEntry {
                        trade_id: trade.trade_id,
                        user_id: trade.buyer_user_id,
                        asset_id: base_id,
                        op: "credit",
                        delta: trade.qty,
                        balance_after: b.avail() + b.frozen(),
                    });
                }
            }
            perf.add_ledger_time(ledger_start.elapsed().as_nanos() as u64);

            // Seller settlement
            let settle_start = Instant::now();
            if let Some(seller_acc) = accounts.get_mut(&trade.seller_user_id) {
                let _ = seller_acc.settle_as_seller(base_id, quote_id, trade.qty, trade_cost, 0);
            }
            perf.add_settlement_time(settle_start.elapsed().as_nanos() as u64);

            // Seller ledger
            let ledger_start = Instant::now();
            if let Some(seller_acc) = accounts.get(&trade.seller_user_id) {
                if let Some(b) = seller_acc.get_balance(base_id) {
                    ledger.write_entry(&LedgerEntry {
                        trade_id: trade.trade_id,
                        user_id: trade.seller_user_id,
                        asset_id: base_id,
                        op: "debit",
                        delta: trade.qty,
                        balance_after: b.avail() + b.frozen(),
                    });
                }
                if let Some(b) = seller_acc.get_balance(quote_id) {
                    ledger.write_entry(&LedgerEntry {
                        trade_id: trade.trade_id,
                        user_id: trade.seller_user_id,
                        asset_id: quote_id,
                        op: "credit",
                        delta: trade_cost,
                        balance_after: b.avail() + b.frozen(),
                    });
                }
            }
            perf.add_ledger_time(ledger_start.elapsed().as_nanos() as u64);
        }

        total_trades += result.trades.len() as u64;
        accepted += 1;
        perf.add_order_latency(order_start.elapsed().as_nanos() as u64);
    }

    (accepted, rejected, total_trades, perf)
}

// ============================================================
// ORDER EXECUTION WITH UBSCORE (New Pipeline)
// ============================================================

/// Execute orders using UBSCore service
///
/// This is the new pipeline that follows the 0x08a architecture:
/// 1. UBSCore writes to WAL first (persistence)
/// 2. UBSCore locks balance
/// 3. ME matches (pure matching, no balance logic)
/// 4. UBSCore settles trades
/// 5. Ledger writes audit log
#[allow(dead_code)] // Will be used when we switch to full pipeline
fn execute_orders_with_ubscore(
    orders: &[InputOrder],
    ubscore: &mut UBSCore,
    book: &mut OrderBook,
    ledger: &mut LedgerWriter,
    symbol_mgr: &SymbolManager,
    active_symbol_id: u32,
) -> (u64, u64, u64, PerfMetrics) {
    let symbol_info = symbol_mgr
        .get_symbol_info_by_id(active_symbol_id)
        .expect("Active symbol not found");
    let qty_unit = symbol_info.qty_unit();
    let base_id = symbol_info.base_asset_id;
    let quote_id = symbol_info.quote_asset_id;

    let mut accepted = 0u64;
    let mut rejected = 0u64;
    let mut total_trades = 0u64;
    let mut perf = PerfMetrics::new(10);

    // Use full pipeline
    for (i, input) in orders.iter().enumerate() {
        let order_start = Instant::now();

        if (i + 1) % 100_000 == 0 {
            println!("Processed {} / {} orders...", i + 1, orders.len());
        }

        match input.action.as_str() {
            "cancel" => {
                // ========================================
                // CANCEL FLOW - Architectural Profiling
                // ========================================
                perf.inc_cancel();

                // --- 1. PRE-TRADE: OrderBook lookup ---
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

                        // --- 3. SETTLEMENT: Unlock balance ---
                        let settle_start = Instant::now();
                        if let Err(e) =
                            ubscore.unlock(cancelled_order.user_id, lock_asset_id, unlock_amount)
                        {
                            eprintln!("Cancel unlock failed: {}", e);
                        }
                        perf.add_settlement_time(settle_start.elapsed().as_nanos() as u64);

                        // --- 4. EVENT LOG: BalanceEvent ---
                        let event_start = Instant::now();
                        if let Some(b) = ubscore.get_balance(cancelled_order.user_id, lock_asset_id)
                        {
                            let unlock_event = BalanceEvent::unlock(
                                cancelled_order.user_id,
                                lock_asset_id,
                                cancelled_order.order_id,
                                unlock_amount,
                                b.lock_version(),
                                b.avail(),
                                b.frozen(),
                            );
                            ledger.write_balance_event(&unlock_event);
                        }
                        perf.add_event_log_time(event_start.elapsed().as_nanos() as u64);
                    }

                    // --- 4. EVENT LOG: OrderEvent ---
                    let event_start = Instant::now();
                    let order_event = OrderEvent::Cancelled {
                        order_id: cancelled_order.order_id,
                        user_id: cancelled_order.user_id,
                        unfilled_qty: remaining_qty,
                    };
                    ledger.write_order_event(&order_event);
                    perf.add_event_log_time(event_start.elapsed().as_nanos() as u64);
                }

                perf.add_order_latency(order_start.elapsed().as_nanos() as u64);
            }
            "place" | _ => {
                // ========================================
                // PLACE FLOW - Architectural Profiling
                // ========================================
                perf.inc_place();

                // --- 1. PRE-TRADE: UBSCore (WAL + Lock) ---
                let pretrade_start = Instant::now();

                let order = InternalOrder::new(
                    input.order_id,
                    input.user_id,
                    active_symbol_id,
                    input.price,
                    input.qty,
                    input.side,
                );

                let lock_asset_id = match input.side {
                    Side::Buy => quote_id,
                    Side::Sell => base_id,
                };
                let lock_amount = match input.side {
                    Side::Buy => input.price * input.qty / qty_unit,
                    Side::Sell => input.qty,
                };

                let valid_order = match ubscore.process_order(order) {
                    Ok(vo) => vo,
                    Err(reason) => {
                        perf.add_pretrade_time(pretrade_start.elapsed().as_nanos() as u64);
                        rejected += 1;
                        // Event log for reject
                        let event_start = Instant::now();
                        let reject_event = OrderEvent::Rejected {
                            seq_id: 0,
                            order_id: input.order_id,
                            user_id: input.user_id,
                            reason,
                        };
                        ledger.write_order_event(&reject_event);
                        perf.add_event_log_time(event_start.elapsed().as_nanos() as u64);
                        continue;
                    }
                };
                perf.add_pretrade_time(pretrade_start.elapsed().as_nanos() as u64);

                // --- 4. EVENT LOG: Accepted + Lock events ---
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
                    );
                    ledger.write_balance_event(&lock_event);
                }
                perf.add_event_log_time(event_start.elapsed().as_nanos() as u64);

                // --- 2. MATCHING: Pure ME ---
                let match_start = Instant::now();
                let result = MatchingEngine::process_order(book, valid_order.order.clone());
                perf.add_matching_time(match_start.elapsed().as_nanos() as u64);

                // --- 3. SETTLEMENT + 4. EVENT LOG per trade ---
                for trade in &result.trades {
                    // 3. SETTLEMENT: UBSCore settle_trade
                    let settle_start = Instant::now();

                    let trade_event = TradeEvent::new(
                        trade.clone(),
                        if input.side == Side::Buy {
                            trade.buyer_order_id
                        } else {
                            trade.seller_order_id
                        },
                        if input.side == Side::Buy {
                            trade.seller_order_id
                        } else {
                            trade.buyer_order_id
                        },
                        input.side,
                        base_id,
                        quote_id,
                        qty_unit,
                    );

                    if let Err(e) = ubscore.settle_trade(&trade_event) {
                        eprintln!("Trade settlement error: {}", e);
                    }

                    // Price Improvement Refund (part of settlement)
                    if input.side == Side::Buy && valid_order.order.order_type == OrderType::Limit {
                        if valid_order.order.price > trade.price {
                            let diff = valid_order.order.price - trade.price;
                            let refund = diff * trade.qty / qty_unit;
                            if refund > 0 {
                                if let Ok(_) = ubscore
                                    .accounts_mut()
                                    .get_mut(&input.user_id)
                                    .unwrap()
                                    .settle_unlock(quote_id, refund)
                                {
                                    // Refund event logged below with other events
                                }
                            }
                        }
                    }
                    perf.add_settlement_time(settle_start.elapsed().as_nanos() as u64);

                    // 4. EVENT LOG: Trade events
                    let event_start = Instant::now();
                    let trade_cost =
                        ((trade.price as u128) * (trade.qty as u128) / (qty_unit as u128)) as u64;

                    // Buyer events
                    if let Some(b) = ubscore.get_balance(trade.buyer_user_id, quote_id) {
                        let settle_event = BalanceEvent::settle_spend(
                            trade.buyer_user_id,
                            quote_id,
                            trade.trade_id,
                            trade_cost,
                            b.settle_version(),
                            b.avail(),
                            b.frozen(),
                        );
                        ledger.write_balance_event(&settle_event);
                        ledger.write_entry(&LedgerEntry {
                            trade_id: trade.trade_id,
                            user_id: trade.buyer_user_id,
                            asset_id: quote_id,
                            op: "debit",
                            delta: trade_cost,
                            balance_after: b.avail() + b.frozen(),
                        });
                    }
                    if let Some(b) = ubscore.get_balance(trade.buyer_user_id, base_id) {
                        let settle_event = BalanceEvent::settle_receive(
                            trade.buyer_user_id,
                            base_id,
                            trade.trade_id,
                            trade.qty,
                            b.settle_version(),
                            b.avail(),
                            b.frozen(),
                        );
                        ledger.write_balance_event(&settle_event);
                        ledger.write_entry(&LedgerEntry {
                            trade_id: trade.trade_id,
                            user_id: trade.buyer_user_id,
                            asset_id: base_id,
                            op: "credit",
                            delta: trade.qty,
                            balance_after: b.avail() + b.frozen(),
                        });
                    }

                    // Seller events
                    if let Some(b) = ubscore.get_balance(trade.seller_user_id, base_id) {
                        let settle_event = BalanceEvent::settle_spend(
                            trade.seller_user_id,
                            base_id,
                            trade.trade_id,
                            trade.qty,
                            b.settle_version(),
                            b.avail(),
                            b.frozen(),
                        );
                        ledger.write_balance_event(&settle_event);
                        ledger.write_entry(&LedgerEntry {
                            trade_id: trade.trade_id,
                            user_id: trade.seller_user_id,
                            asset_id: base_id,
                            op: "debit",
                            delta: trade.qty,
                            balance_after: b.avail() + b.frozen(),
                        });
                    }
                    if let Some(b) = ubscore.get_balance(trade.seller_user_id, quote_id) {
                        let settle_event = BalanceEvent::settle_receive(
                            trade.seller_user_id,
                            quote_id,
                            trade.trade_id,
                            trade_cost,
                            b.settle_version(),
                            b.avail(),
                            b.frozen(),
                        );
                        ledger.write_balance_event(&settle_event);
                        ledger.write_entry(&LedgerEntry {
                            trade_id: trade.trade_id,
                            user_id: trade.seller_user_id,
                            asset_id: quote_id,
                            op: "credit",
                            delta: trade_cost,
                            balance_after: b.avail() + b.frozen(),
                        });
                    }

                    // Refund event if applicable
                    if input.side == Side::Buy && valid_order.order.order_type == OrderType::Limit {
                        if valid_order.order.price > trade.price {
                            let diff = valid_order.order.price - trade.price;
                            let refund = diff * trade.qty / qty_unit;
                            if refund > 0 {
                                if let Some(b) = ubscore.get_balance(input.user_id, quote_id) {
                                    let restore_event = BalanceEvent::settle_restore(
                                        input.user_id,
                                        quote_id,
                                        trade.trade_id,
                                        refund,
                                        b.settle_version(),
                                        b.avail(),
                                        b.frozen(),
                                    );
                                    ledger.write_balance_event(&restore_event);
                                }
                            }
                        }
                    }
                    perf.add_event_log_time(event_start.elapsed().as_nanos() as u64);
                }

                // Final order event (Filled/PartialFilled)
                let event_start = Instant::now();
                if result.order.filled_qty > 0 {
                    let avg_price = if result.trades.is_empty() {
                        0
                    } else {
                        let total_value: u128 = result
                            .trades
                            .iter()
                            .map(|t| (t.price as u128) * (t.qty as u128))
                            .sum();
                        let total_qty: u128 = result.trades.iter().map(|t| t.qty as u128).sum();
                        if total_qty > 0 {
                            (total_value / total_qty) as u64
                        } else {
                            0
                        }
                    };

                    let event = if result.order.remaining_qty() == 0 {
                        OrderEvent::Filled {
                            order_id: result.order.order_id,
                            user_id: result.order.user_id,
                            filled_qty: result.order.filled_qty,
                            avg_price,
                        }
                    } else {
                        OrderEvent::PartialFilled {
                            order_id: result.order.order_id,
                            user_id: result.order.user_id,
                            filled_qty: result.order.filled_qty,
                            remaining_qty: result.order.remaining_qty(),
                        }
                    };
                    ledger.write_order_event(&event);
                }
                perf.add_event_log_time(event_start.elapsed().as_nanos() as u64);

                perf.add_trades(result.trades.len() as u64);
                total_trades += result.trades.len() as u64;
                accepted += 1;
                perf.add_order_latency(order_start.elapsed().as_nanos() as u64);
            }
        }
    }

    // Flush WAL and ledger at the end
    if let Err(e) = ubscore.flush_wal() {
        eprintln!("WAL flush error: {}", e);
    }
    ledger.flush();

    (accepted, rejected, total_trades, perf)
}

// ============================================================
// MAIN
// ============================================================

fn use_ubscore_mode() -> bool {
    std::env::args().any(|a| a == "--ubscore")
}

fn use_pipeline_mode() -> bool {
    std::env::args().any(|a| a == "--pipeline")
}

fn use_pipeline_mt_mode() -> bool {
    std::env::args().any(|a| a == "--pipeline-mt")
}

fn main() {
    let output_dir = get_output_dir();
    let input_dir = get_input_dir();
    let ubscore_mode = use_ubscore_mode();
    let pipeline_mode = use_pipeline_mode();
    let pipeline_mt_mode = use_pipeline_mt_mode();

    if pipeline_mt_mode {
        println!("=== 0xInfinity: Chapter 8g - Multi-Thread Pipeline ===");
    } else if pipeline_mode {
        println!("=== 0xInfinity: Chapter 8f - Ring Buffer Pipeline ===");
    } else if ubscore_mode {
        println!("=== 0xInfinity: Chapter 8b - UBSCore Pipeline ===");
    } else {
        println!("=== 0xInfinity: Chapter 7 - Testing Framework ===");
    }
    println!("Output directory: {}/", output_dir);
    println!("Input directory: {}/\n", input_dir);

    let start_time = Instant::now();

    // Step 1: Load configuration
    println!("[1] Loading configuration...");
    let (symbol_mgr, active_symbol_id) = load_symbol_manager();

    // Generate output paths
    let balances_t1 = format!("{}/t1_balances_deposited.csv", output_dir);
    let balances_t2 = format!("{}/t2_balances_final.csv", output_dir);
    let orderbook_t2 = format!("{}/t2_orderbook.csv", output_dir);
    let ledger_path = format!("{}/t2_ledger.csv", output_dir);
    let events_path = format!("{}/t2_events.csv", output_dir); // New: BalanceEvent log
    let order_events_path = format!("{}/t2_order_events.csv", output_dir); // New: OrderEvent log
    let summary_path = format!("{}/t2_summary.txt", output_dir);
    let wal_path = format!("{}/orders.wal", output_dir);

    std::fs::create_dir_all(output_dir).unwrap();

    // Step 2: Load balances
    println!("\n[2] Loading balances and depositing...");
    let mut accounts = load_balances_and_deposit(&format!("{}/balances_init.csv", input_dir));

    // Step 3: Snapshot after deposit
    println!("\n[3] Dumping balance snapshot after deposit...");
    dump_balances(&accounts, &symbol_mgr, active_symbol_id, &balances_t1);

    // Step 4: Load orders
    println!("\n[4] Loading orders...");
    let orders = load_orders(
        &format!("{}/orders.csv", input_dir),
        &symbol_mgr,
        active_symbol_id,
    );

    let load_time = start_time.elapsed();
    println!("\n    Data loading completed in {:.2?}", load_time);

    // Step 5: Initialize engine
    println!("\n[5] Initializing matching engine...");
    let mut book = OrderBook::new();
    let mut ledger = LedgerWriter::new(&ledger_path);

    // Enable event logging for UBSCore mode (complete event sourcing)
    if ubscore_mode {
        ledger.enable_event_logging(&events_path);
        ledger.enable_order_logging(&order_events_path);
        println!(
            "    Event logging enabled: {}, {}",
            events_path, order_events_path
        );
    }

    // Step 6: Execute orders
    println!("\n[6] Executing orders...");
    let exec_start = Instant::now();

    let symbol_info = symbol_mgr
        .get_symbol_info_by_id(active_symbol_id)
        .expect("Active symbol not found");
    let base_id = symbol_info.base_asset_id;
    let quote_id = symbol_info.quote_asset_id;
    let (accepted, rejected, total_trades, perf, final_accounts, final_book) = if pipeline_mt_mode {
        println!("    Using Multi-Thread Pipeline (4 threads)...");

        // Enable event logging for multi-thread mode
        ledger.enable_event_logging(&events_path);
        println!("    Event logging enabled: {}", events_path);

        // Create UBSCore and initialize with deposits
        let wal_config = WalConfig {
            path: wal_path.clone(),
            flush_interval_entries: 100,
            sync_on_flush: false,
        };

        let mut ubscore =
            UBSCore::new(symbol_mgr.clone(), wal_config).expect("Failed to create UBSCore");

        // Transfer initial balances to UBSCore via deposit()
        let mut deposit_count = 0u64;
        for (user_id, account) in &accounts {
            for asset_id in [base_id, quote_id] {
                if let Some(balance) = account.get_balance(asset_id) {
                    if balance.avail() > 0 {
                        ubscore
                            .deposit(*user_id, asset_id, balance.avail())
                            .unwrap();

                        // Record Deposit event
                        if let Some(b) = ubscore.get_balance(*user_id, asset_id) {
                            deposit_count += 1;
                            let deposit_event = BalanceEvent::deposit(
                                *user_id,
                                asset_id,
                                deposit_count,
                                balance.avail(),
                                b.lock_version(),
                                b.avail(),
                                b.frozen(),
                            );
                            ledger.write_balance_event(&deposit_event);
                        }
                    }
                }
            }
        }
        println!("    Recorded {} deposit events", deposit_count);

        // Clone orders for multi-thread (takes ownership)
        let orders_owned: Vec<_> = orders.iter().cloned().collect();

        // Run multi-threaded pipeline (consumes ubscore, book, ledger)
        let result = run_pipeline_multi_thread(
            orders_owned,
            ubscore,
            book,
            ledger,
            &symbol_mgr,
            active_symbol_id,
        );

        // Print stats (use snapshot to get place/cancel counts)
        let stats_snap = result.stats.snapshot();
        println!(
            "    Pipeline: ingested={} (place={}, cancel={}), accepted={}, rejected={}, trades={}",
            stats_snap.orders_ingested,
            stats_snap.places_count,
            stats_snap.cancels_count,
            result.accepted,
            result.rejected,
            result.total_trades
        );

        (
            result.accepted,
            result.rejected,
            result.total_trades,
            PerfMetrics::default(), // Multi-thread mode doesn't track perf yet
            result.final_accounts,
            None, // OrderBook consumed by threads
        )
    } else if pipeline_mode {
        println!("    Using Ring Buffer Pipeline (Single Thread)...");

        // Create UBSCore and initialize with deposits
        let wal_config = WalConfig {
            path: wal_path.clone(),
            flush_interval_entries: 100,
            sync_on_flush: false,
        };

        let mut ubscore =
            UBSCore::new(symbol_mgr.clone(), wal_config).expect("Failed to create UBSCore");

        // Transfer initial balances to UBSCore via deposit()
        let mut deposit_count = 0u64;
        for (user_id, account) in &accounts {
            for asset_id in [base_id, quote_id] {
                if let Some(balance) = account.get_balance(asset_id) {
                    if balance.avail() > 0 {
                        ubscore
                            .deposit(*user_id, asset_id, balance.avail())
                            .unwrap();

                        // Record Deposit event
                        if let Some(b) = ubscore.get_balance(*user_id, asset_id) {
                            deposit_count += 1;
                            let deposit_event = BalanceEvent::deposit(
                                *user_id,
                                asset_id,
                                deposit_count,
                                balance.avail(),
                                b.lock_version(),
                                b.avail(),
                                b.frozen(),
                            );
                            ledger.write_balance_event(&deposit_event);
                        }
                    }
                }
            }
        }
        println!("    Recorded {} deposit events", deposit_count);

        // Run pipeline
        let result = run_pipeline_single_thread(
            &orders,
            &mut ubscore,
            &mut book,
            &mut ledger,
            &symbol_mgr,
            active_symbol_id,
        );

        // Get final accounts from UBSCore
        let final_accs = ubscore.accounts().clone();

        // Print stats
        let (wal_entries, wal_bytes) = ubscore.wal_stats();
        println!("    WAL: {} entries, {} bytes", wal_entries, wal_bytes);
        println!("    Pipeline: {}", result.pipeline_stats);

        (
            result.accepted,
            result.rejected,
            result.total_trades,
            result.perf,
            final_accs,
            Some(book),
        )
    } else if ubscore_mode {
        println!("    Using UBSCore pipeline (WAL + Balance Lock)...");

        // Create UBSCore and initialize with deposits
        let wal_config = WalConfig {
            path: wal_path.clone(),
            flush_interval_entries: 100, // Group commit every 100 orders
            sync_on_flush: false,        // Faster for benchmarks
        };

        let mut ubscore =
            UBSCore::new(symbol_mgr.clone(), wal_config).expect("Failed to create UBSCore");

        // Transfer initial balances to UBSCore via deposit()
        // Also record Deposit events for complete audit trail
        let mut deposit_count = 0u64;
        for (user_id, account) in &accounts {
            for asset_id in [base_id, quote_id] {
                if let Some(balance) = account.get_balance(asset_id) {
                    if balance.avail() > 0 {
                        ubscore
                            .deposit(*user_id, asset_id, balance.avail())
                            .unwrap();

                        // Record Deposit event
                        if let Some(b) = ubscore.get_balance(*user_id, asset_id) {
                            deposit_count += 1;
                            let deposit_event = BalanceEvent::deposit(
                                *user_id,
                                asset_id,
                                deposit_count, // ref_id = deposit sequence
                                balance.avail(),
                                b.lock_version(),
                                b.avail(),
                                b.frozen(),
                            );
                            ledger.write_balance_event(&deposit_event);
                        }
                    }
                }
            }
        }
        println!("    Recorded {} deposit events", deposit_count);

        let (acc, rej, trades, perf) = execute_orders_with_ubscore(
            &orders,
            &mut ubscore,
            &mut book,
            &mut ledger,
            &symbol_mgr,
            active_symbol_id,
        );

        // Get final accounts from UBSCore
        let final_accs = ubscore.accounts().clone();

        // Print WAL stats
        let (wal_entries, wal_bytes) = ubscore.wal_stats();
        println!("    WAL: {} entries, {} bytes", wal_entries, wal_bytes);

        (acc, rej, trades, perf, final_accs, Some(book))
    } else {
        let (acc, rej, trades, perf) = execute_orders(
            &orders,
            &mut accounts,
            &mut book,
            &mut ledger,
            &symbol_mgr,
            active_symbol_id,
        );
        (acc, rej, trades, perf, accounts, Some(book))
    };

    let exec_time = exec_start.elapsed();

    // Step 7: Dump final state
    println!("\n[7] Dumping final state...");
    dump_balances(&final_accounts, &symbol_mgr, active_symbol_id, &balances_t2);
    if let Some(ref book) = final_book {
        dump_orderbook_snapshot(book, &orderbook_t2);
    } else {
        println!("    (OrderBook not available in multi-thread mode)");
    }

    // Step 8: Summary
    let total_time = start_time.elapsed();
    let orders_per_sec = orders.len() as f64 / exec_time.as_secs_f64();
    let trades_per_sec = total_trades as f64 / exec_time.as_secs_f64();

    let summary = format!(
        r#"=== Execution Summary ===
Symbol: {}
Total Orders: {}
  Accepted: {}
  Rejected: {}
Total Trades: {}
Execution Time: {:.2?}
Throughput: {:.0} orders/sec | {:.0} trades/sec

Final Orderbook:
  Best Bid: {:?}
  Best Ask: {:?}
  Bid Depth: {} levels
  Ask Depth: {} levels

Total Time: {:.2?}

=== Performance Breakdown ===
{}
=== Latency Percentiles (sampled) ===
  Min:   {:>8} ns
  Avg:   {:>8} ns
  P50:   {:>8} ns
  P99:   {:>8} ns
  P99.9: {:>8} ns
  Max:   {:>8} ns
Samples: {}
"#,
        symbol_info.symbol,
        orders.len(),
        accepted,
        rejected,
        total_trades,
        exec_time,
        orders_per_sec,
        trades_per_sec,
        final_book.as_ref().map(|b| b.best_bid()).unwrap_or(None),
        final_book.as_ref().map(|b| b.best_ask()).unwrap_or(None),
        final_book.as_ref().map(|b| b.depth().0).unwrap_or(0),
        final_book.as_ref().map(|b| b.depth().1).unwrap_or(0),
        total_time,
        perf.breakdown(),
        perf.min_latency().unwrap_or(0),
        perf.avg_latency().unwrap_or(0),
        perf.percentile(50.0).unwrap_or(0),
        perf.percentile(99.0).unwrap_or(0),
        perf.percentile(99.9).unwrap_or(0),
        perf.max_latency().unwrap_or(0),
        perf.latency_samples.len(),
    );

    println!("\n{}", summary);

    // Write summary
    let mut summary_file = File::create(&summary_path).unwrap();
    summary_file.write_all(summary.as_bytes()).unwrap();
    println!("Summary written to {}", summary_path);

    // Write perf baseline
    let perf_path = format!("{}/t2_perf.txt", output_dir);
    let mut perf_file = File::create(&perf_path).unwrap();
    writeln!(perf_file, "# Performance Baseline - 0xInfinity").unwrap();
    writeln!(perf_file, "# Generated: {}", chrono_lite_now()).unwrap();
    writeln!(perf_file, "orders={}", orders.len()).unwrap();
    writeln!(perf_file, "trades={}", total_trades).unwrap();
    writeln!(
        perf_file,
        "exec_time_ms={:.2}",
        exec_time.as_secs_f64() * 1000.0
    )
    .unwrap();
    writeln!(perf_file, "throughput_ops={:.0}", orders_per_sec).unwrap();
    writeln!(perf_file, "throughput_tps={:.0}", trades_per_sec).unwrap();
    writeln!(perf_file, "pretrade_ns={}", perf.total_pretrade_ns).unwrap();
    writeln!(perf_file, "matching_ns={}", perf.total_matching_ns).unwrap();
    writeln!(perf_file, "settlement_ns={}", perf.total_settlement_ns).unwrap();
    writeln!(perf_file, "event_log_ns={}", perf.total_event_log_ns).unwrap();
    writeln!(
        perf_file,
        "latency_min_ns={}",
        perf.min_latency().unwrap_or(0)
    )
    .unwrap();
    writeln!(
        perf_file,
        "latency_avg_ns={}",
        perf.avg_latency().unwrap_or(0)
    )
    .unwrap();
    writeln!(
        perf_file,
        "latency_p50_ns={}",
        perf.percentile(50.0).unwrap_or(0)
    )
    .unwrap();
    writeln!(
        perf_file,
        "latency_p99_ns={}",
        perf.percentile(99.0).unwrap_or(0)
    )
    .unwrap();
    writeln!(
        perf_file,
        "latency_p999_ns={}",
        perf.percentile(99.9).unwrap_or(0)
    )
    .unwrap();
    writeln!(
        perf_file,
        "latency_max_ns={}",
        perf.max_latency().unwrap_or(0)
    )
    .unwrap();
    println!("Perf baseline written to {}", perf_path);

    println!("\n=== Done ===");
}
