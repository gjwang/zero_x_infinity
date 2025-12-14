mod engine;
mod models;
mod symbol_manager;

use engine::OrderBook;
use models::{Order, Side};
use symbol_manager::SymbolManager;

/// Convert decimal string to u64 internal representation
/// e.g., "100.50" with 2 decimals -> 10050
fn parse_decimal(s: &str, decimals: u32) -> u64 {
    let multiplier = 10u64.pow(decimals);

    if let Some(pos) = s.find('.') {
        let (int_part, dec_part) = s.split_at(pos);
        let dec_part = &dec_part[1..]; // remove the '.'

        let int_val: u64 = int_part.parse().unwrap_or(0);
        let dec_len = dec_part.len() as u32;

        if dec_len > decimals {
            // Truncate extra decimals
            let dec_val: u64 = dec_part[..decimals as usize].parse().unwrap_or(0);
            int_val * multiplier + dec_val
        } else {
            // Pad with zeros
            let dec_val: u64 = dec_part.parse().unwrap_or(0);
            int_val * multiplier + dec_val * 10u64.pow(decimals - dec_len)
        }
    } else {
        let int_val: u64 = s.parse().unwrap_or(0);
        int_val * multiplier
    }
}

/// Convert u64 internal representation to decimal string for display
/// e.g., 10050 with 2 decimals -> "100.50"
fn format_decimal(value: u64, decimals: u32) -> String {
    let multiplier = 10u64.pow(decimals);
    let int_part = value / multiplier;
    let dec_part = value % multiplier;

    if decimals == 0 {
        format!("{}", int_part)
    } else {
        format!(
            "{}.{:0>width$}",
            int_part,
            dec_part,
            width = decimals as usize
        )
    }
}

fn main() {
    // Load symbol configuration (simulating database load)
    let manager = SymbolManager::load_from_db();

    // Get BTC_USDT symbol info
    let symbol = "BTC_USDT";
    let symbol_info = manager.get_symbol_info(symbol).expect("Symbol not found");
    let price_decimals = symbol_info.price_decimal;

    // Get BTC asset info for quantity decimals
    let btc_info = manager
        .assets
        .get(&symbol_info.base_asset_id)
        .expect("Asset not found");
    let qty_display_decimals = btc_info.display_decimals;

    println!("--- 0xInfinity: Stage 3 (Decimal World) ---");
    println!("Symbol: {} (ID: {})", symbol, symbol_info.symbol_id);
    println!(
        "Price Decimals: {}, Qty Display Decimals: {}",
        price_decimals, qty_display_decimals
    );
    println!();

    let mut book = OrderBook::new();

    // 1. Makers (Sells) - Using decimal parsing
    println!("[1] Makers coming in...");
    // Sell 10 BTC @ $100.00
    let price1 = parse_decimal("100.00", price_decimals);
    let qty1 = parse_decimal("10.000", qty_display_decimals);
    println!(
        "    Order 1: Sell {} BTC @ ${}",
        format_decimal(qty1, qty_display_decimals),
        format_decimal(price1, price_decimals)
    );
    book.add_order(Order::new(1, price1, qty1, Side::Sell));

    // Sell 5 BTC @ $102.00
    let price2 = parse_decimal("102.00", price_decimals);
    let qty2 = parse_decimal("5.000", qty_display_decimals);
    println!(
        "    Order 2: Sell {} BTC @ ${}",
        format_decimal(qty2, qty_display_decimals),
        format_decimal(price2, price_decimals)
    );
    book.add_order(Order::new(2, price2, qty2, Side::Sell));

    // Sell 5 BTC @ $101.00
    let price3 = parse_decimal("101.00", price_decimals);
    let qty3 = parse_decimal("5.000", qty_display_decimals);
    println!(
        "    Order 3: Sell {} BTC @ ${}",
        format_decimal(qty3, qty_display_decimals),
        format_decimal(price3, price_decimals)
    );
    book.add_order(Order::new(3, price3, qty3, Side::Sell));

    // 2. Taker (Buy)
    println!("\n[2] Taker eats liquidity...");
    // Buy 12 BTC @ $101.50 (will match 10@100 + 2@101)
    let price4 = parse_decimal("101.50", price_decimals);
    let qty4 = parse_decimal("12.000", qty_display_decimals);
    println!(
        "    Order 4: Buy {} BTC @ ${}",
        format_decimal(qty4, qty_display_decimals),
        format_decimal(price4, price_decimals)
    );
    book.add_order(Order::new(4, price4, qty4, Side::Buy));

    // 3. Maker (Buy)
    println!("\n[3] More makers...");
    // Buy 10 BTC @ $99.00
    let price5 = parse_decimal("99.00", price_decimals);
    let qty5 = parse_decimal("10.000", qty_display_decimals);
    println!(
        "    Order 5: Buy {} BTC @ ${}",
        format_decimal(qty5, qty_display_decimals),
        format_decimal(price5, price_decimals)
    );
    book.add_order(Order::new(5, price5, qty5, Side::Buy));

    println!("\n--- End of Simulation ---");

    // Demonstrate u64 max value
    println!("\n--- u64 Range Demo ---");
    let max_u64: u64 = u64::MAX;
    println!("u64::MAX = {}", max_u64);
    println!(
        "With 8 decimals, max representable value = {}",
        format_decimal(max_u64, 8)
    );
}
