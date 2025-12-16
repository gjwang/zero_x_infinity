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
//! - Order WAL (persistence first!)
//! - Balance Lock/Unlock/Settle
//! - Single-threaded atomic operations
//! ```

use std::fs::File;
use std::io::Write;
use std::time::Instant;

use rustc_hash::FxHashMap;
use zero_x_infinity::config::{TradingConfig, UserId};
use zero_x_infinity::csv_io::{
    InputOrder, chrono_lite_now, dump_balances, dump_orderbook_snapshot, load_balances_and_deposit,
    load_orders, load_trading_config,
};
use zero_x_infinity::engine::MatchingEngine;
use zero_x_infinity::ledger::{LedgerEntry, LedgerWriter};
use zero_x_infinity::messages::TradeEvent;
use zero_x_infinity::models::{Order, Side};
use zero_x_infinity::orderbook::OrderBook;
use zero_x_infinity::perf::PerfMetrics;
use zero_x_infinity::ubscore::UBSCore;
use zero_x_infinity::user_account::UserAccount;
use zero_x_infinity::wal::WalConfig;

// ============================================================
// OUTPUT DIRECTORY
// ============================================================

fn get_output_dir() -> &'static str {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--baseline") {
        "baseline"
    } else {
        "output"
    }
}

// ============================================================
// ORDER EXECUTION
// ============================================================

fn execute_orders(
    orders: &[InputOrder],
    accounts: &mut FxHashMap<UserId, UserAccount>,
    book: &mut OrderBook,
    ledger: &mut LedgerWriter,
    config: &TradingConfig,
) -> (u64, u64, u64, PerfMetrics) {
    let qty_unit = config.qty_unit();
    let base_id = config.base_asset_id();
    let quote_id = config.quote_asset_id();

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

        let order = Order::new(
            input.order_id,
            input.user_id,
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
            let trade_cost = trade.price * trade.qty / qty_unit;

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
                        trade_id: trade.id,
                        user_id: trade.buyer_user_id,
                        asset_id: quote_id,
                        op: "debit",
                        delta: trade_cost,
                        balance_after: b.avail() + b.frozen(),
                    });
                }
                if let Some(b) = buyer_acc.get_balance(base_id) {
                    ledger.write_entry(&LedgerEntry {
                        trade_id: trade.id,
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
                        trade_id: trade.id,
                        user_id: trade.seller_user_id,
                        asset_id: base_id,
                        op: "debit",
                        delta: trade.qty,
                        balance_after: b.avail() + b.frozen(),
                    });
                }
                if let Some(b) = seller_acc.get_balance(quote_id) {
                    ledger.write_entry(&LedgerEntry {
                        trade_id: trade.id,
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
    config: &TradingConfig,
) -> (u64, u64, u64, PerfMetrics) {
    let base_id = config.base_asset_id();
    let quote_id = config.quote_asset_id();

    let mut accepted = 0u64;
    let mut rejected = 0u64;
    let mut total_trades = 0u64;
    let mut perf = PerfMetrics::new(10);

    for (i, input) in orders.iter().enumerate() {
        let order_start = Instant::now();

        if (i + 1) % 100_000 == 0 {
            println!("Processed {} / {} orders...", i + 1, orders.len());
        }

        // ========================================
        // STEP 1: UBSCore processes order (WAL + Lock)
        // ========================================
        let balance_check_start = Instant::now();

        let order = Order::new(
            input.order_id,
            input.user_id,
            input.price,
            input.qty,
            input.side,
        );

        let valid_order = match ubscore.process_order(order.clone()) {
            Ok(vo) => vo,
            Err(_event) => {
                // Order rejected (insufficient balance, etc.)
                rejected += 1;
                continue;
            }
        };

        perf.add_balance_check_time(balance_check_start.elapsed().as_nanos() as u64);

        // ========================================
        // STEP 2: ME matches (pure matching)
        // ========================================
        let match_start = Instant::now();
        let result = MatchingEngine::process_order(book, valid_order.order);
        perf.add_matching_time(match_start.elapsed().as_nanos() as u64);

        // ========================================
        // STEP 3: UBSCore settles each trade
        // ========================================
        for trade in &result.trades {
            let settle_start = Instant::now();

            // Create TradeEvent for UBSCore
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
            );

            // UBSCore handles all balance updates
            if let Err(e) = ubscore.settle_trade(&trade_event) {
                eprintln!("Trade settlement error: {}", e);
            }

            perf.add_settlement_time(settle_start.elapsed().as_nanos() as u64);

            // ========================================
            // STEP 4: Write ledger entries
            // ========================================
            let ledger_start = Instant::now();
            let trade_cost = trade.price * trade.qty / config.qty_unit();

            // Buyer ledger entries
            if let Some((avail, frozen)) = ubscore.query_balance(trade.buyer_user_id, quote_id) {
                ledger.write_entry(&LedgerEntry {
                    trade_id: trade.id,
                    user_id: trade.buyer_user_id,
                    asset_id: quote_id,
                    op: "debit",
                    delta: trade_cost,
                    balance_after: avail + frozen,
                });
            }
            if let Some((avail, frozen)) = ubscore.query_balance(trade.buyer_user_id, base_id) {
                ledger.write_entry(&LedgerEntry {
                    trade_id: trade.id,
                    user_id: trade.buyer_user_id,
                    asset_id: base_id,
                    op: "credit",
                    delta: trade.qty,
                    balance_after: avail + frozen,
                });
            }

            // Seller ledger entries
            if let Some((avail, frozen)) = ubscore.query_balance(trade.seller_user_id, base_id) {
                ledger.write_entry(&LedgerEntry {
                    trade_id: trade.id,
                    user_id: trade.seller_user_id,
                    asset_id: base_id,
                    op: "debit",
                    delta: trade.qty,
                    balance_after: avail + frozen,
                });
            }
            if let Some((avail, frozen)) = ubscore.query_balance(trade.seller_user_id, quote_id) {
                ledger.write_entry(&LedgerEntry {
                    trade_id: trade.id,
                    user_id: trade.seller_user_id,
                    asset_id: quote_id,
                    op: "credit",
                    delta: trade_cost,
                    balance_after: avail + frozen,
                });
            }

            perf.add_ledger_time(ledger_start.elapsed().as_nanos() as u64);
        }

        // No need to track order completion - Balance.frozen is the source of truth

        total_trades += result.trades.len() as u64;
        accepted += 1;
        perf.add_order_latency(order_start.elapsed().as_nanos() as u64);
    }

    // Flush WAL at the end
    if let Err(e) = ubscore.flush_wal() {
        eprintln!("WAL flush error: {}", e);
    }

    (accepted, rejected, total_trades, perf)
}

// ============================================================
// MAIN
// ============================================================

fn use_ubscore_mode() -> bool {
    std::env::args().any(|a| a == "--ubscore")
}

fn main() {
    let output_dir = get_output_dir();
    let ubscore_mode = use_ubscore_mode();

    if ubscore_mode {
        println!("=== 0xInfinity: Chapter 8b - UBSCore Pipeline ===");
    } else {
        println!("=== 0xInfinity: Chapter 7 - Testing Framework ===");
    }
    println!("Output directory: {}/\n", output_dir);

    let start_time = Instant::now();

    // Step 1: Load configuration
    println!("[1] Loading configuration...");
    let config = load_trading_config();

    // Generate output paths
    let balances_t1 = format!("{}/t1_balances_deposited.csv", output_dir);
    let balances_t2 = format!("{}/t2_balances_final.csv", output_dir);
    let orderbook_t2 = format!("{}/t2_orderbook.csv", output_dir);
    let ledger_path = format!("{}/t2_ledger.csv", output_dir);
    let summary_path = format!("{}/t2_summary.txt", output_dir);
    let wal_path = format!("{}/orders.wal", output_dir);

    std::fs::create_dir_all(output_dir).unwrap();

    // Step 2: Load balances
    println!("\n[2] Loading balances and depositing...");
    let mut accounts = load_balances_and_deposit("fixtures/balances_init.csv", &config);

    // Step 3: Snapshot after deposit
    println!("\n[3] Dumping balance snapshot after deposit...");
    dump_balances(&accounts, &config, &balances_t1);

    // Step 4: Load orders
    println!("\n[4] Loading orders...");
    let orders = load_orders("fixtures/orders.csv", &config);

    let load_time = start_time.elapsed();
    println!("\n    Data loading completed in {:.2?}", load_time);

    // Step 5: Initialize engine
    println!("\n[5] Initializing matching engine...");
    let mut book = OrderBook::new();
    let mut ledger = LedgerWriter::new(&ledger_path);

    // Step 6: Execute orders
    println!("\n[6] Executing orders...");
    let exec_start = Instant::now();

    let (accepted, rejected, total_trades, perf, final_accounts) = if ubscore_mode {
        println!("    Using UBSCore pipeline (WAL + Balance Lock)...");

        // Create UBSCore and initialize with deposits
        let wal_config = WalConfig {
            path: wal_path.clone(),
            flush_interval_entries: 100, // Group commit every 100 orders
            sync_on_flush: false,        // Faster for benchmarks
        };

        let mut ubscore =
            UBSCore::new(config.clone(), wal_config).expect("Failed to create UBSCore");

        // Transfer initial balances to UBSCore via deposit()
        for (user_id, account) in &accounts {
            for asset_id in [config.base_asset_id(), config.quote_asset_id()] {
                if let Some(balance) = account.get_balance(asset_id) {
                    if balance.avail() > 0 {
                        ubscore
                            .deposit(*user_id, asset_id, balance.avail())
                            .unwrap();
                    }
                }
            }
        }

        let (acc, rej, trades, perf) =
            execute_orders_with_ubscore(&orders, &mut ubscore, &mut book, &mut ledger, &config);

        // Get final accounts from UBSCore
        let final_accs = ubscore.accounts().clone();

        // Print WAL stats
        let (wal_entries, wal_bytes) = ubscore.wal_stats();
        println!("    WAL: {} entries, {} bytes", wal_entries, wal_bytes);

        (acc, rej, trades, perf, final_accs)
    } else {
        let (acc, rej, trades, perf) =
            execute_orders(&orders, &mut accounts, &mut book, &mut ledger, &config);
        (acc, rej, trades, perf, accounts)
    };

    let exec_time = exec_start.elapsed();

    // Step 7: Dump final state
    println!("\n[7] Dumping final state...");
    dump_balances(&final_accounts, &config, &balances_t2);
    dump_orderbook_snapshot(&book, &orderbook_t2);

    // Step 8: Summary
    let total_time = start_time.elapsed();
    let orders_per_sec = orders.len() as f64 / exec_time.as_secs_f64();
    let trades_per_sec = total_trades as f64 / exec_time.as_secs_f64();
    let (balance_pct, match_pct, settle_pct, ledger_pct) = perf.breakdown_pct();

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
Balance Check:    {:>8.2}ms ({:>5.1}%)
Matching Engine:  {:>8.2}ms ({:>5.1}%)
Settlement:       {:>8.2}ms ({:>5.1}%)
Ledger I/O:       {:>8.2}ms ({:>5.1}%)

=== Latency Percentiles (sampled) ===
  Min:   {:>8} ns
  Avg:   {:>8} ns
  P50:   {:>8} ns
  P99:   {:>8} ns
  P99.9: {:>8} ns
  Max:   {:>8} ns
Samples: {}
"#,
        config.active_symbol.symbol,
        orders.len(),
        accepted,
        rejected,
        total_trades,
        exec_time,
        orders_per_sec,
        trades_per_sec,
        book.best_bid(),
        book.best_ask(),
        book.depth().0,
        book.depth().1,
        total_time,
        perf.total_balance_check_ns as f64 / 1_000_000.0,
        balance_pct,
        perf.total_matching_ns as f64 / 1_000_000.0,
        match_pct,
        perf.total_settlement_ns as f64 / 1_000_000.0,
        settle_pct,
        perf.total_ledger_ns as f64 / 1_000_000.0,
        ledger_pct,
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
    writeln!(
        perf_file,
        "balance_check_ns={}",
        perf.total_balance_check_ns
    )
    .unwrap();
    writeln!(perf_file, "matching_ns={}", perf.total_matching_ns).unwrap();
    writeln!(perf_file, "settlement_ns={}", perf.total_settlement_ns).unwrap();
    writeln!(perf_file, "ledger_ns={}", perf.total_ledger_ns).unwrap();
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
