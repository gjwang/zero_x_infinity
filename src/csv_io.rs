//! CSV I/O - Load and save data from/to CSV files
//!
//! This module handles all CSV file operations for configuration,
//! orders, balances, and output snapshots.

use crate::core_types::UserId;
use crate::models::{OrderType, Side};
use crate::orderbook::OrderBook;
use crate::symbol_manager::SymbolManager;
use crate::user_account::UserAccount;
use rustc_hash::FxHashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};

// ============================================================
// Constants for file paths
// ============================================================

pub const ASSETS_CONFIG_CSV: &str = "fixtures/assets_config.csv";
pub const SYMBOLS_CONFIG_CSV: &str = "fixtures/symbols_config.csv";
pub const BALANCES_INIT_CSV: &str = "fixtures/balances_init.csv";
pub const ORDERS_CSV: &str = "fixtures/orders.csv";

// ============================================================
// Configuration Loading
// ============================================================

/// Load SymbolManager from CSV files
///
/// Returns (SymbolManager, active_symbol_id)
pub fn load_symbol_manager() -> (SymbolManager, u32) {
    let mut manager = SymbolManager::new();

    // Load assets first
    let file = File::open(ASSETS_CONFIG_CSV).expect("Failed to open assets_config.csv");
    let reader = BufReader::new(file);
    let mut asset_count = 0;

    for line in reader.lines().skip(1) {
        let line = line.unwrap();
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() >= 4 {
            let asset_id: u32 = parts[0].parse().unwrap();
            let name = parts[1];
            let decimals: u32 = parts[2].parse().unwrap();
            let display_decimals: u32 = parts[3].parse().unwrap();
            manager.add_asset(asset_id, decimals, display_decimals, name);
            asset_count += 1;
        }
    }
    println!("Loaded {} assets from {}", asset_count, ASSETS_CONFIG_CSV);

    // Load symbols
    let file = File::open(SYMBOLS_CONFIG_CSV).expect("Failed to open symbols_config.csv");
    let reader = BufReader::new(file);
    let mut symbol_count = 0;
    let mut first_symbol_id: Option<u32> = None;

    for line in reader.lines().skip(1) {
        let line = line.unwrap();
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() >= 6 {
            let symbol_id: u32 = parts[0].parse().unwrap();
            let symbol = parts[1];
            let base_asset_id: u32 = parts[2].parse().unwrap();
            let quote_asset_id: u32 = parts[3].parse().unwrap();
            let price_decimal: u32 = parts[4].parse().unwrap();
            let price_display_decimal: u32 = parts[5].parse().unwrap();

            manager
                .insert_symbol(
                    symbol,
                    symbol_id,
                    base_asset_id,
                    quote_asset_id,
                    price_decimal,
                    price_display_decimal,
                )
                .expect("Failed to insert symbol - check asset exists");

            if first_symbol_id.is_none() {
                first_symbol_id = Some(symbol_id);
            }
            symbol_count += 1;
        }
    }
    println!(
        "Loaded {} symbols from {}",
        symbol_count, SYMBOLS_CONFIG_CSV
    );

    // Use first symbol as active
    let active_symbol_id = first_symbol_id.expect("No symbols configured");
    let symbol_info = manager.get_symbol_info_by_id(active_symbol_id).unwrap();

    println!(
        "Active symbol: {} (base={}, quote={})",
        symbol_info.symbol,
        manager
            .get_asset_name(symbol_info.base_asset_id)
            .unwrap_or("?".to_string()),
        manager
            .get_asset_name(symbol_info.quote_asset_id)
            .unwrap_or("?".to_string())
    );
    println!("  Internal decimals: base={}", symbol_info.base_decimals);

    // Calculate and display max tradeable value (overflow safety check)
    // For price * qty to not overflow u64, we calculate max qty at a reference price
    let quote_decimals = manager
        .get_asset_decimal(symbol_info.quote_asset_id)
        .unwrap_or(6);
    let qty_scale = 10u64.pow(symbol_info.base_decimals);

    // Reference price: 100,000 (e.g., BTC @ $100k)
    let ref_price_human = 100_000u64;
    let ref_price_scaled = ref_price_human * 10u64.pow(quote_decimals);

    // Max qty (in base units) before overflow: u64::MAX / ref_price_scaled
    let max_qty_units = u64::MAX / ref_price_scaled;
    let max_qty_display = max_qty_units as f64 / qty_scale as f64;

    let base_name = manager
        .get_asset_name(symbol_info.base_asset_id)
        .unwrap_or("BASE".to_string());

    // Always print max tradeable value
    if max_qty_display < 100.0 {
        // Low value - show warning
        println!(
            "  ⚠️  Max tradeable @ ${}: {:.2} {} per order (u64 safe) ⚠️ LOW!",
            ref_price_human, max_qty_display, base_name
        );
    } else {
        // Normal value
        println!(
            "  Max tradeable @ ${}: {:.2} {} per order (u64 safe)",
            ref_price_human, max_qty_display, base_name
        );
    }

    (manager, active_symbol_id)
}

// ============================================================
// Order and Balance Loading
// ============================================================

/// Input order from CSV (before conversion to internal Order)
#[derive(Debug, Clone)]
pub struct InputOrder {
    pub order_id: u64,
    pub user_id: u64,
    pub side: Side,
    pub price: u64,
    pub qty: u64,
}

/// Load balances from CSV and create user accounts with deposits
pub fn load_balances_and_deposit(path: &str) -> FxHashMap<UserId, UserAccount> {
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
            let _frozen: u64 = parts[3].parse().unwrap();
            let _version: u64 = parts[4].parse().unwrap();

            let account = accounts
                .entry(user_id)
                .or_insert_with(|| UserAccount::new(user_id));
            account.deposit(asset_id, avail).unwrap();
        }
    }

    println!(
        "Loaded balances for {} accounts (row-based format)",
        accounts.len()
    );
    accounts
}

/// Load orders from CSV and convert to internal format
pub fn load_orders(path: &str, manager: &SymbolManager, active_symbol_id: u32) -> Vec<InputOrder> {
    let file = File::open(path).expect("Failed to open orders.csv");
    let reader = BufReader::new(file);

    let symbol_info = manager
        .get_symbol_info_by_id(active_symbol_id)
        .expect("Active symbol not found");
    let base_multiplier = symbol_info.qty_unit(); // 10^base_decimals
    let quote_multiplier = 10u64.pow(
        manager
            .get_asset_decimal(symbol_info.quote_asset_id)
            .unwrap_or(6),
    );

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

            let price_float: f64 = parts[3].parse().unwrap();
            let qty_float: f64 = parts[4].parse().unwrap();

            let price = (price_float * quote_multiplier as f64).round() as u64;
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
// Output Functions
// ============================================================

/// Dump balances to CSV file
pub fn dump_balances(
    accounts: &FxHashMap<UserId, UserAccount>,
    manager: &SymbolManager,
    active_symbol_id: u32,
    path: &str,
) {
    let mut file = File::create(path).unwrap();

    // Note: 'version' column now contains lock_version for backward compatibility
    // settle_version is tracked separately in the Balance struct
    writeln!(file, "user_id,asset_id,avail,frozen,version").unwrap();

    let mut user_ids: Vec<_> = accounts.keys().collect();
    user_ids.sort();

    let symbol_info = manager
        .get_symbol_info_by_id(active_symbol_id)
        .expect("Active symbol not found");
    let base_id = symbol_info.base_asset_id;
    let quote_id = symbol_info.quote_asset_id;

    for user_id in user_ids {
        let account = accounts.get(user_id).unwrap();

        if let Some(b) = account.get_balance(base_id) {
            writeln!(
                file,
                "{},{},{},{},{}",
                user_id,
                base_id,
                b.avail(),
                b.frozen(),
                b.lock_version() // Now uses lock_version explicitly
            )
            .unwrap();
        }

        if let Some(b) = account.get_balance(quote_id) {
            writeln!(
                file,
                "{},{},{},{},{}",
                user_id,
                quote_id,
                b.avail(),
                b.frozen(),
                b.lock_version() // Now uses lock_version explicitly
            )
            .unwrap();
        }
    }
    println!("Dumped balances to {}", path);
}

/// Dump complete ME orderbook snapshot
pub fn dump_orderbook_snapshot(book: &OrderBook, path: &str) {
    let mut file = File::create(path).unwrap();

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
            order.order_id,
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

/// Write final orderbook state (summary)
pub fn write_final_orderbook(book: &OrderBook, path: &str) {
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

/// Simple timestamp without external dependency
pub fn chrono_lite_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let duration = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    let secs = duration.as_secs();
    let days = secs / 86400;
    let years = 1970 + days / 365;
    let remaining_days = days % 365;
    let months = remaining_days / 30 + 1;
    let day = remaining_days % 30 + 1;
    format!("{}-{:02}-{:02}", years, months, day)
}
