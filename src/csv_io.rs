//! CSV I/O - Load and save data from/to CSV files
//!
//! This module handles all CSV file operations for configuration,
//! orders, balances, and output snapshots.

use crate::config::{AssetConfig, SymbolConfig, TradingConfig};
use crate::core_types::{AssetId, UserId};
use crate::models::{OrderType, Side};
use crate::orderbook::OrderBook;
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

/// Load asset configurations from CSV
pub fn load_assets_config(path: &str) -> FxHashMap<AssetId, AssetConfig> {
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

/// Load symbol configurations from CSV
pub fn load_symbols_config(path: &str) -> Vec<SymbolConfig> {
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

/// Load complete trading configuration
pub fn load_trading_config() -> TradingConfig {
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

    // Client-facing display decimals
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
pub fn load_balances_and_deposit(
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
pub fn load_orders(path: &str, config: &TradingConfig) -> Vec<InputOrder> {
    let file = File::open(path).expect("Failed to open orders.csv");
    let reader = BufReader::new(file);

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
    config: &TradingConfig,
    path: &str,
) {
    let mut file = File::create(path).unwrap();

    writeln!(file, "user_id,asset_id,avail,frozen,version").unwrap();

    let mut user_ids: Vec<_> = accounts.keys().collect();
    user_ids.sort();

    let base_id = config.active_symbol.base_asset_id;
    let quote_id = config.active_symbol.quote_asset_id;

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
                b.version()
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
                b.version()
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
