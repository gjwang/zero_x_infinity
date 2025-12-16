//! Chapter 7: Testing Framework for Matching Engine
//!
//! This module provides a batch testing infrastructure:
//! 1. Load configuration from assets_config.csv and symbols_config.csv
//! 2. Load orders and accounts from CSV
//! 3. Execute all orders through the matching engine
//! 4. Output results to CSV (trades, order states, balances)
//! 5. Save final state snapshot
//! 6. Collect performance metrics (Chapter 7b)

mod enforced_balance;
mod engine;
mod models;
mod symbol_manager;
mod user_account;

use engine::OrderBook;
use models::{Order, OrderResult, OrderType, Side, Trade};
use rustc_hash::FxHashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::time::Instant;
use user_account::{AssetId, UserAccount, UserId};

// ============================================================
// CONFIGURATION
// ============================================================

/// File paths (single source of truth)
const ASSETS_CONFIG_CSV: &str = "fixtures/assets_config.csv";
const SYMBOLS_CONFIG_CSV: &str = "fixtures/symbols_config.csv";
const ORDERS_CSV: &str = "fixtures/orders.csv";
const BALANCES_INIT_CSV: &str = "fixtures/balances_init.csv";

// Output directory: current run results (compare against baseline/)
// baseline/ = golden files (first correct run, git tracked)
// output/   = current run (gitignored, compare with baseline/)
const OUTPUT_DIR: &str = "output";
const OUTPUT_BALANCES_T1: &str = "output/t1_balances_deposited.csv";
const OUTPUT_BALANCES_T2: &str = "output/t2_balances_final.csv";
const OUTPUT_ORDERBOOK_T2: &str = "output/t2_orderbook.csv";
const OUTPUT_TRADES: &str = "output/t2_trades.csv";
const OUTPUT_SUMMARY: &str = "output/t2_summary.txt";

// ============================================================
// DATA STRUCTURES
// ============================================================

/// Asset configuration from assets_config.csv
///
/// # Decimal Precision Design
///
/// Two types of decimals serve different purposes:
///
/// | Field | Mutable | Purpose | Example |
/// |-------|---------|---------|---------|
/// | `decimals` | ⚠️ **IMMUTABLE** | Internal storage precision | BTC=8 (satoshi) |
/// | `display_decimals` | ✅ Dynamic | Client-facing precision | BTC=2 (0.01 BTC) |
///
/// ## Key Rules:
///
/// 1. **`decimals`** - Set once, never change
///    - Defines minimum unit (e.g., 1 satoshi = 10^-8 BTC)
///    - All internal calculations use this precision
///    - Changing this would break all existing balances/orders
///
/// 2. **`display_decimals`** - Can be adjusted anytime
///    - Client sees prices/quantities with this precision
///    - Can be changed based on market conditions
///    - Example: Show $84,907.12 instead of $84,907.123456
#[derive(Debug, Clone)]
struct AssetConfig {
    asset_id: AssetId,
    asset: String,
    /// Internal precision - ⚠️ IMMUTABLE after launch
    /// All balances/orders stored with this precision
    decimals: u32,
    /// Client-facing precision - ✅ can be adjusted dynamically
    /// Orders from clients use this format
    display_decimals: u32,
}

/// Symbol configuration from symbols_config.csv
#[derive(Debug, Clone)]
struct SymbolConfig {
    #[allow(dead_code)]
    symbol_id: u32,
    symbol: String,
    base_asset_id: AssetId,
    quote_asset_id: AssetId,
    #[allow(dead_code)]
    price_decimal: u32,
    #[allow(dead_code)]
    price_display_decimal: u32,
}

/// Complete trading configuration
#[derive(Debug)]
struct TradingConfig {
    assets: FxHashMap<AssetId, AssetConfig>,
    symbols: Vec<SymbolConfig>,
    // Active symbol for this test run
    active_symbol: SymbolConfig,
    // Internal storage decimals
    base_decimals: u32,
    quote_decimals: u32,
    // Client-facing display decimals (for parsing orders)
    // qty_display_decimals: from base_asset.display_decimals
    // price_display_decimals: from symbol.price_display_decimal
    qty_display_decimals: u32,
    price_display_decimals: u32,
}

/// Input order from CSV
#[derive(Debug, Clone)]
struct InputOrder {
    order_id: u64,
    user_id: u64,
    side: Side,
    price: u64,
    qty: u64,
}

// ============================================================
// PERFORMANCE METRICS
// ============================================================

/// Performance metrics for execution analysis
/// Collects timing breakdown and latency samples for percentile calculation
#[derive(Default)]
struct PerfMetrics {
    // Timing breakdown (nanoseconds)
    total_balance_check_ns: u64, // Account lookup + balance check + lock
    total_matching_ns: u64,      // OrderBook.add_order()
    total_settlement_ns: u64,    // Balance updates after trade
    total_ledger_ns: u64,        // Ledger file I/O

    // Per-order latency samples (nanoseconds)
    // We sample every Nth order to keep memory bounded
    latency_samples: Vec<u64>,
    sample_rate: usize,
    sample_counter: usize,
}

impl PerfMetrics {
    fn new(sample_rate: usize) -> Self {
        PerfMetrics {
            sample_rate,
            latency_samples: Vec::with_capacity(10_000),
            ..Default::default()
        }
    }

    fn add_order_latency(&mut self, latency_ns: u64) {
        self.sample_counter += 1;
        if self.sample_counter >= self.sample_rate {
            self.latency_samples.push(latency_ns);
            self.sample_counter = 0;
        }
    }

    fn add_balance_check_time(&mut self, ns: u64) {
        self.total_balance_check_ns += ns;
    }

    fn add_matching_time(&mut self, ns: u64) {
        self.total_matching_ns += ns;
    }

    fn add_settlement_time(&mut self, ns: u64) {
        self.total_settlement_ns += ns;
    }

    fn add_ledger_time(&mut self, ns: u64) {
        self.total_ledger_ns += ns;
    }

    /// Calculate percentile from sorted samples
    fn percentile(&self, p: f64) -> Option<u64> {
        if self.latency_samples.is_empty() {
            return None;
        }
        let mut sorted = self.latency_samples.clone();
        sorted.sort_unstable();
        let idx = ((p / 100.0) * (sorted.len() - 1) as f64).round() as usize;
        Some(sorted[idx.min(sorted.len() - 1)])
    }

    fn min_latency(&self) -> Option<u64> {
        self.latency_samples.iter().copied().min()
    }

    fn max_latency(&self) -> Option<u64> {
        self.latency_samples.iter().copied().max()
    }

    fn avg_latency(&self) -> Option<u64> {
        if self.latency_samples.is_empty() {
            return None;
        }
        Some(self.latency_samples.iter().sum::<u64>() / self.latency_samples.len() as u64)
    }
}

// ============================================================
// CSV LOADING
// ============================================================

fn load_assets_config(path: &str) -> FxHashMap<AssetId, AssetConfig> {
    let file = File::open(path).expect("Failed to open assets_config.csv");
    let reader = BufReader::new(file);

    let mut assets = FxHashMap::default();

    for line in reader.lines().skip(1) {
        let line = line.unwrap();
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() >= 4 {
            let asset = AssetConfig {
                asset_id: parts[0].parse().unwrap(),
                asset: parts[1].to_string(),
                decimals: parts[2].parse().unwrap(),
                display_decimals: parts[3].parse().unwrap(),
            };
            assets.insert(asset.asset_id, asset);
        }
    }

    println!("Loaded {} assets from {}", assets.len(), path);
    assets
}

fn load_symbols_config(path: &str) -> Vec<SymbolConfig> {
    let file = File::open(path).expect("Failed to open symbols_config.csv");
    let reader = BufReader::new(file);

    let mut symbols = Vec::new();

    for line in reader.lines().skip(1) {
        let line = line.unwrap();
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() >= 6 {
            symbols.push(SymbolConfig {
                symbol_id: parts[0].parse().unwrap(),
                symbol: parts[1].to_string(),
                base_asset_id: parts[2].parse().unwrap(),
                quote_asset_id: parts[3].parse().unwrap(),
                price_decimal: parts[4].parse().unwrap(),
                price_display_decimal: parts[5].parse().unwrap(),
            });
        }
    }

    println!("Loaded {} symbols from {}", symbols.len(), path);
    symbols
}

fn load_trading_config() -> TradingConfig {
    let assets = load_assets_config(ASSETS_CONFIG_CSV);
    let symbols = load_symbols_config(SYMBOLS_CONFIG_CSV);

    // Use first symbol (BTC_USDT) as active
    let active_symbol = symbols.first().cloned().expect("No symbols configured");

    // Internal storage decimals
    let base_decimals = assets
        .get(&active_symbol.base_asset_id)
        .map(|a| a.decimals)
        .unwrap_or(8);
    let quote_decimals = assets
        .get(&active_symbol.quote_asset_id)
        .map(|a| a.decimals)
        .unwrap_or(6);

    // Client-facing display decimals for parsing orders:
    // - qty precision: from base_asset.display_decimals
    // - price precision: from symbol.price_display_decimal (NOT quote_asset!)
    let base_display_decimals = assets
        .get(&active_symbol.base_asset_id)
        .map(|a| a.display_decimals)
        .unwrap_or(6);
    let price_display_decimals = active_symbol.price_display_decimal;

    println!(
        "Active symbol: {} (base={}, quote={})",
        active_symbol.symbol,
        assets
            .get(&active_symbol.base_asset_id)
            .map(|a| a.asset.as_str())
            .unwrap_or("?"),
        assets
            .get(&active_symbol.quote_asset_id)
            .map(|a| a.asset.as_str())
            .unwrap_or("?")
    );
    println!(
        "  Internal decimals: base={}, quote={}",
        base_decimals, quote_decimals
    );
    println!(
        "  Display decimals:  qty={}, price={}",
        base_display_decimals, price_display_decimals
    );

    TradingConfig {
        assets,
        symbols,
        active_symbol,
        base_decimals,
        quote_decimals,
        qty_display_decimals: base_display_decimals,
        price_display_decimals,
    }
}

fn load_balances_and_deposit(
    path: &str,
    _config: &TradingConfig,
) -> FxHashMap<UserId, UserAccount> {
    let file = File::open(path).expect("Failed to open balances.csv");
    let reader = BufReader::new(file);

    let mut accounts: FxHashMap<UserId, UserAccount> = FxHashMap::default();

    // Row-based CSV format: user_id,asset_id,avail,frozen,version
    for line in reader.lines().skip(1) {
        let line = line.unwrap();
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() >= 5 {
            let user_id: u64 = parts[0].parse().unwrap();
            let asset_id: u32 = parts[1].parse().unwrap();
            let avail: u64 = parts[2].parse().unwrap();
            let _frozen: u64 = parts[3].parse().unwrap(); // Initial frozen is 0
            let _version: u64 = parts[4].parse().unwrap(); // Initial version is 0

            // Get or create account
            let account = accounts
                .entry(user_id)
                .or_insert_with(|| UserAccount::new(user_id));
            // deposit() adds to avail balance
            account.deposit(asset_id, avail).unwrap();
        }
    }

    println!(
        "Loaded balances for {} accounts (row-based format)",
        accounts.len()
    );
    accounts
}

fn load_orders(path: &str, config: &TradingConfig) -> Vec<InputOrder> {
    let file = File::open(path).expect("Failed to open orders.csv");
    let reader = BufReader::new(file);

    // Client input uses display_decimals format (e.g., "84907.12" for 2 decimals)
    // Convert to internal units using full decimals (e.g., 84907.12 * 10^6 = 84907120000)
    let base_multiplier = 10u64.pow(config.base_decimals);
    let quote_multiplier = 10u64.pow(config.quote_decimals);

    let mut orders = Vec::new();

    for line in reader.lines().skip(1) {
        let line = line.unwrap();
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() >= 5 {
            let order_id: u64 = parts[0].parse().unwrap();
            let user_id: u64 = parts[1].parse().unwrap();
            let side = if parts[2] == "buy" {
                Side::Buy
            } else {
                Side::Sell
            };

            // Parse client format (display_decimals) and convert to internal units
            let price_float: f64 = parts[3].parse().unwrap(); // e.g., 84907.12 (2 decimals)
            let qty_float: f64 = parts[4].parse().unwrap(); // e.g., 0.39048310 (8 decimals)

            let price = (price_float * quote_multiplier as f64).round() as u64; // -> 84907120000
            let qty = (qty_float * base_multiplier as f64).round() as u64;

            orders.push(InputOrder {
                order_id,
                user_id,
                side,
                price,
                qty,
            });
        }
    }

    println!(
        "Loaded {} orders (converted from client format)",
        orders.len()
    );
    orders
}

// ============================================================
// CSV OUTPUT
/// Ledger entry for settlement audit
/// Each balance change is recorded as one entry
struct LedgerEntry {
    trade_id: u64,
    user_id: u64,
    asset_id: u32,
    op: &'static str, // "credit" or "debit"
    delta: u64,
    balance_after: u64,
}

struct LedgerWriter {
    file: File,
    entry_count: u64,
}

impl LedgerWriter {
    fn new_with_path(path: &str) -> Self {
        let mut file = File::create(path).unwrap();
        // Ledger format: trade_id,user_id,asset_id,op,delta,balance_after
        writeln!(file, "trade_id,user_id,asset_id,op,delta,balance_after").unwrap();

        LedgerWriter {
            file,
            entry_count: 0,
        }
    }

    fn write_entry(&mut self, entry: &LedgerEntry) {
        writeln!(
            self.file,
            "{},{},{},{},{},{}",
            entry.trade_id,
            entry.user_id,
            entry.asset_id,
            entry.op,
            entry.delta,
            entry.balance_after
        )
        .unwrap();
        self.entry_count += 1;
    }
}

/// Dump complete ME orderbook snapshot
/// This snapshot contains all information needed to restore ME state after restart
fn dump_orderbook_snapshot(book: &OrderBook, path: &str) {
    let mut file = File::create(path).unwrap();

    // Complete snapshot format: all fields needed to reconstruct Order struct
    // order_id: unique ID
    // user_id: who placed the order
    // side: buy/sell
    // order_type: limit/market
    // price: limit price (internal units)
    // qty: original quantity
    // filled_qty: how much has been filled
    // status: current order status
    writeln!(
        file,
        "order_id,user_id,side,order_type,price,qty,filled_qty,status"
    )
    .unwrap();

    for order in book.all_orders() {
        let side_str = match order.side {
            Side::Buy => "buy",
            Side::Sell => "sell",
        };
        let type_str = match order.order_type {
            OrderType::Limit => "limit",
            OrderType::Market => "market",
        };
        writeln!(
            file,
            "{},{},{},{},{},{},{},{:?}",
            order.id,
            order.user_id,
            side_str,
            type_str,
            order.price,
            order.qty,
            order.filled_qty,
            order.status
        )
        .unwrap();
    }
    println!(
        "Dumped ME snapshot: {} active orders to {}",
        book.all_orders().len(),
        path
    );
}

fn dump_balances(accounts: &FxHashMap<UserId, UserAccount>, config: &TradingConfig, path: &str) {
    std::fs::create_dir_all(OUTPUT_DIR).unwrap();
    let mut file = File::create(path).unwrap();

    // Row-based format: user_id,asset_id,avail,frozen,version (matches input format)
    writeln!(file, "user_id,asset_id,avail,frozen,version").unwrap();

    let mut user_ids: Vec<_> = accounts.keys().collect();
    user_ids.sort();

    let base_id = config.active_symbol.base_asset_id;
    let quote_id = config.active_symbol.quote_asset_id;

    for user_id in user_ids {
        let account = accounts.get(user_id).unwrap();

        // Base asset row
        if let Some(b) = account.get_balance(base_id) {
            writeln!(
                file,
                "{},{},{},{},{}",
                user_id,
                base_id,
                b.avail(),
                b.frozen(),
                b.version()
            )
            .unwrap();
        }

        // Quote asset row
        if let Some(b) = account.get_balance(quote_id) {
            writeln!(
                file,
                "{},{},{},{},{}",
                user_id,
                quote_id,
                b.avail(),
                b.frozen(),
                b.version()
            )
            .unwrap();
        }
    }
    println!("Dumped balances to {}", path);
}

fn write_final_orderbook(book: &OrderBook, path: &str) {
    let mut file = File::create(path).unwrap();
    writeln!(file, "best_bid,best_ask,bid_depth,ask_depth").unwrap();

    let (bid_depth, ask_depth) = book.depth();
    writeln!(
        file,
        "{},{},{},{}",
        book.best_bid().map(|p| p.to_string()).unwrap_or_default(),
        book.best_ask().map(|p| p.to_string()).unwrap_or_default(),
        bid_depth,
        ask_depth
    )
    .unwrap();
    println!("Wrote final orderbook to {}", path);
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
    let qty_unit = 10u64.pow(config.base_decimals);
    let base_id = config.active_symbol.base_asset_id;
    let quote_id = config.active_symbol.quote_asset_id;

    let mut accepted = 0u64;
    let mut rejected = 0u64;
    let mut total_trades = 0u64;

    // Performance metrics: sample every 10th order for latency percentiles
    let mut perf = PerfMetrics::new(10);

    for (i, input) in orders.iter().enumerate() {
        let order_start = Instant::now();

        // Progress every 100k orders
        if (i + 1) % 100_000 == 0 {
            println!("Processed {} / {} orders...", i + 1, orders.len());
        }

        // PRE-ORDER VALIDATION: Account lookup + balance check + lock
        let balance_check_start = Instant::now();

        let account = match accounts.get_mut(&input.user_id) {
            Some(acc) => acc,
            None => {
                rejected += 1;
                continue;
            }
        };

        // Calculate required funds and try to lock
        let lock_result = match input.side {
            Side::Buy => {
                // Buy: lock quote asset
                let cost = input.price * input.qty / qty_unit;
                account.get_balance_mut(quote_id).and_then(|b| b.lock(cost))
            }
            Side::Sell => {
                // Sell: lock base asset
                account
                    .get_balance_mut(base_id)
                    .and_then(|b| b.lock(input.qty))
            }
        };

        perf.add_balance_check_time(balance_check_start.elapsed().as_nanos() as u64);

        if lock_result.is_err() {
            rejected += 1;
            continue;
        }

        // Submit order to matching engine (MATCHING)
        let match_start = Instant::now();
        let order = Order::new(
            input.order_id,
            input.user_id,
            input.price,
            input.qty,
            input.side.clone(),
        );
        let result = book.add_order(order);
        perf.add_matching_time(match_start.elapsed().as_nanos() as u64);

        // Process trades: settlement + ledger write per trade (interleaved)
        for trade in &result.trades {
            let trade_cost = trade.price * trade.qty / qty_unit;

            // Buyer settlement: debit quote (frozen -> out), credit base (in)
            let settle_start = Instant::now();
            if let Some(buyer_acc) = accounts.get_mut(&trade.buyer_user_id) {
                let _ = buyer_acc.settle_as_buyer(quote_id, base_id, trade_cost, trade.qty, 0);
            }
            perf.add_settlement_time(settle_start.elapsed().as_nanos() as u64);

            // Buyer ledger entries (immediately after settlement)
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

            // Seller settlement: debit base (frozen -> out), credit quote (in)
            let settle_start = Instant::now();
            if let Some(seller_acc) = accounts.get_mut(&trade.seller_user_id) {
                let _ = seller_acc.settle_as_seller(base_id, quote_id, trade.qty, trade_cost, 0);
            }
            perf.add_settlement_time(settle_start.elapsed().as_nanos() as u64);

            // Seller ledger entries (immediately after settlement)
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

        // Sample per-order latency
        perf.add_order_latency(order_start.elapsed().as_nanos() as u64);
    }

    (accepted, rejected, total_trades, perf)
}

// ============================================================
// MAIN
// ============================================================

/// Parse command line args: --baseline flag switches output to baseline/
fn get_output_dir() -> &'static str {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--baseline") {
        "baseline"
    } else {
        "output"
    }
}

fn main() {
    let output_dir = get_output_dir();
    println!("=== 0xInfinity: Chapter 7 - Testing Framework ===");
    println!("Output directory: {}/\n", output_dir);

    let start_time = Instant::now();

    // Step 1: Load configuration (single source of truth)
    println!("[1] Loading configuration...");
    let config = load_trading_config();

    // Generate output paths based on output_dir
    let balances_t1 = format!("{}/t1_balances_deposited.csv", output_dir);
    let balances_t2 = format!("{}/t2_balances_final.csv", output_dir);
    let orderbook_t2 = format!("{}/t2_orderbook.csv", output_dir);
    let ledger_path = format!("{}/t2_ledger.csv", output_dir);
    let summary_path = format!("{}/t2_summary.txt", output_dir);

    // Ensure output directory exists
    std::fs::create_dir_all(output_dir).unwrap();

    // Step 2: Load balances and deposit (init user accounts)
    println!("\n[2] Loading balances and depositing...");
    let mut accounts = load_balances_and_deposit(BALANCES_INIT_CSV, &config);

    // Step 3: Dump balance snapshot AFTER deposit (before trading)
    println!("\n[3] Dumping balance snapshot after deposit...");
    dump_balances(&accounts, &config, &balances_t1);

    // Step 4: Load orders
    println!("\n[4] Loading orders...");
    let orders = load_orders(ORDERS_CSV, &config);

    let load_time = start_time.elapsed();
    println!("\n    Data loading completed in {:.2?}", load_time);

    // Step 5: Initialize matching engine
    println!("\n[5] Initializing matching engine...");
    let mut book = OrderBook::new();
    let mut ledger = LedgerWriter::new_with_path(&ledger_path);

    // Step 6: Execute orders
    println!("\n[6] Executing orders...");
    let exec_start = Instant::now();

    let (accepted, rejected, total_trades, perf) =
        execute_orders(&orders, &mut accounts, &mut book, &mut ledger, &config);

    let exec_time = exec_start.elapsed();

    // Step 7: Dump final state
    println!("\n[7] Dumping final state...");
    dump_balances(&accounts, &config, &balances_t2);
    dump_orderbook_snapshot(&book, &orderbook_t2);

    // Step 8: Summary with performance metrics
    let total_time = start_time.elapsed();
    let orders_per_sec = orders.len() as f64 / exec_time.as_secs_f64();
    let trades_per_sec = total_trades as f64 / exec_time.as_secs_f64();

    // Calculate timing breakdown percentages
    let total_ns = perf.total_balance_check_ns
        + perf.total_matching_ns
        + perf.total_settlement_ns
        + perf.total_ledger_ns;
    let balance_pct = if total_ns > 0 {
        perf.total_balance_check_ns as f64 / total_ns as f64 * 100.0
    } else {
        0.0
    };
    let match_pct = if total_ns > 0 {
        perf.total_matching_ns as f64 / total_ns as f64 * 100.0
    } else {
        0.0
    };
    let settle_pct = if total_ns > 0 {
        perf.total_settlement_ns as f64 / total_ns as f64 * 100.0
    } else {
        0.0
    };
    let ledger_pct = if total_ns > 0 {
        perf.total_ledger_ns as f64 / total_ns as f64 * 100.0
    } else {
        0.0
    };

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

    // Write summary to file
    let mut summary_file = File::create(&summary_path).unwrap();
    summary_file.write_all(summary.as_bytes()).unwrap();
    println!("Summary written to {}", summary_path);

    // Write perf baseline to separate file
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

/// Simple timestamp without external dependency
fn chrono_lite_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let duration = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    let secs = duration.as_secs();
    // Convert to approximate date (not accounting for leap years/etc, just for display)
    let days = secs / 86400;
    let years = 1970 + days / 365;
    let remaining_days = days % 365;
    let months = remaining_days / 30 + 1;
    let day = remaining_days % 30 + 1;
    format!("{}-{:02}-{:02}", years, months, day)
}
