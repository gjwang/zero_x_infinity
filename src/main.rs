mod enforced_balance;
mod engine;
mod models;
mod symbol_manager;
mod user_account;

use engine::OrderBook;
use models::{Order, Side};
use rustc_hash::FxHashMap;
use symbol_manager::SymbolManager;
use user_account::UserAccount;

/// Convert decimal string to u64 internal representation
#[allow(dead_code)]
fn parse_decimal(s: &str, decimals: u32) -> u64 {
    let multiplier = 10u64.pow(decimals);

    if let Some(pos) = s.find('.') {
        let (int_part, dec_part) = s.split_at(pos);
        let dec_part = &dec_part[1..];

        let int_val: u64 = int_part.parse().unwrap_or(0);
        let dec_len = dec_part.len() as u32;

        if dec_len > decimals {
            let dec_val: u64 = dec_part[..decimals as usize].parse().unwrap_or(0);
            int_val * multiplier + dec_val
        } else {
            let dec_val: u64 = dec_part.parse().unwrap_or(0);
            int_val * multiplier + dec_val * 10u64.pow(decimals - dec_len)
        }
    } else {
        let int_val: u64 = s.parse().unwrap_or(0);
        int_val * multiplier
    }
}

/// Convert u64 internal representation to decimal string
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

// Asset IDs
const BTC: u32 = 1;
const USDT: u32 = 2;

fn main() {
    let manager = SymbolManager::load_from_db();

    let symbol = "BTC_USDT";
    let symbol_info = manager.get_symbol_info(symbol).expect("Symbol not found");
    let base_asset_id = symbol_info.base_asset_id;
    let quote_asset_id = symbol_info.quote_asset_id;
    let price_decimal = symbol_info.price_decimal;

    // Get qty precision from base asset
    let base_asset = manager
        .assets
        .get(&base_asset_id)
        .expect("Base asset not found");
    let qty_decimal = base_asset.decimals;
    let qty_unit = 10u64.pow(qty_decimal); // Precomputed divisor for cost calculation

    println!("=== 0xInfinity: Stage 6 (Enforced Balance) ===");
    println!(
        "Symbol: {} | Price: {} decimals, Qty: {} decimals",
        symbol, price_decimal, qty_decimal
    );
    println!("Cost formula: price * qty / {}", qty_unit);
    println!();

    // Initialize user accounts (now using UserAccount directly instead of AccountManager)
    let mut accounts: FxHashMap<u64, UserAccount> = FxHashMap::default();

    // User 1 (Alice): Seller - has BTC
    // User 2 (Bob): Buyer - has USDT
    let alice = 1u64;
    let bob = 2u64;

    // Create accounts
    accounts.insert(alice, UserAccount::new(alice));
    accounts.insert(bob, UserAccount::new(bob));

    // Price unit for USDT (quote asset) - use price_decimal
    let price_unit = 10u64.pow(price_decimal);

    // Deposit initial funds using the new enforced API
    println!("[0] Initial deposits...");

    // Alice: deposit (auto-creates asset slots)
    let alice_acc = accounts.get_mut(&alice).unwrap();
    alice_acc.deposit(BTC, 100 * qty_unit).unwrap();
    alice_acc.deposit(USDT, 10_000 * price_unit).unwrap();

    // Bob: deposit (auto-creates asset slots)
    let bob_acc = accounts.get_mut(&bob).unwrap();
    bob_acc.deposit(USDT, 200_000 * price_unit).unwrap();
    bob_acc.deposit(BTC, 5 * qty_unit).unwrap();

    println!(
        "    Alice: {} BTC, {} USDT",
        format_decimal(
            accounts
                .get(&alice)
                .unwrap()
                .get_balance(BTC)
                .map(|b| b.avail())
                .unwrap_or(0),
            qty_decimal
        ),
        format_decimal(
            accounts
                .get(&alice)
                .unwrap()
                .get_balance(USDT)
                .map(|b| b.avail())
                .unwrap_or(0),
            price_decimal
        )
    );
    println!(
        "    Bob:   {} BTC, {} USDT",
        format_decimal(
            accounts
                .get(&bob)
                .unwrap()
                .get_balance(BTC)
                .map(|b| b.avail())
                .unwrap_or(0),
            qty_decimal
        ),
        format_decimal(
            accounts
                .get(&bob)
                .unwrap()
                .get_balance(USDT)
                .map(|b| b.avail())
                .unwrap_or(0),
            price_decimal
        )
    );

    let mut book = OrderBook::new();

    // [1] Alice places sell orders
    println!("\n[1] Alice places sell orders...");

    // price = 100 USDT (in price_unit), qty = 10 BTC (in qty_unit)
    let price1 = 100 * price_unit;
    let qty1 = 10 * qty_unit;

    // Lock funds (sell order locks base asset) - using new enforced API
    if accounts
        .get_mut(&alice)
        .unwrap()
        .get_balance_mut(BTC)
        .unwrap()
        .lock(qty1)
        .is_ok()
    {
        let result = book.add_order(Order::new(1, alice, price1, qty1, Side::Sell));
        println!(
            "    Order 1: Sell {} BTC @ ${} -> {:?}",
            format_decimal(qty1, qty_decimal),
            format_decimal(price1, price_decimal),
            result.order.status
        );
    }

    let price2 = 101 * price_unit;
    let qty2 = 5 * qty_unit;
    if accounts
        .get_mut(&alice)
        .unwrap()
        .get_balance_mut(BTC)
        .unwrap()
        .lock(qty2)
        .is_ok()
    {
        let result = book.add_order(Order::new(2, alice, price2, qty2, Side::Sell));
        println!(
            "    Order 2: Sell {} BTC @ ${} -> {:?}",
            format_decimal(qty2, qty_decimal),
            format_decimal(price2, price_decimal),
            result.order.status
        );
    }

    let alice_btc = accounts.get(&alice).unwrap().get_balance(BTC).unwrap();
    println!(
        "    Alice balance: avail={} BTC, frozen={} BTC",
        format_decimal(alice_btc.avail(), qty_decimal),
        format_decimal(alice_btc.frozen(), qty_decimal)
    );

    // [2] Bob places a buy order that matches
    println!("\n[2] Bob places buy order (taker)...");

    let price3 = 101 * price_unit;
    let qty3 = 12 * qty_unit;
    // cost = price * qty / qty_unit (price is "USDT per BTC", qty is BTC)
    let cost = price3 * qty3 / qty_unit;

    println!(
        "    Order 3: Buy {} BTC @ ${} (cost: {} USDT)",
        format_decimal(qty3, qty_decimal),
        format_decimal(price3, price_decimal),
        format_decimal(cost, price_decimal)
    );

    // Lock funds for buy order
    if accounts
        .get_mut(&bob)
        .unwrap()
        .get_balance_mut(USDT)
        .unwrap()
        .lock(cost)
        .is_ok()
    {
        let result = book.add_order(Order::new(3, bob, price3, qty3, Side::Buy));

        println!("    Trades:");
        for trade in &result.trades {
            println!(
                "      - Trade #{}: {} BTC @ ${}",
                trade.id,
                format_decimal(trade.qty, qty_decimal),
                format_decimal(trade.price, price_decimal)
            );

            // Settle each trade using new enforced API
            let trade_cost = trade.price * trade.qty / qty_unit;

            // Buyer (Bob): spend frozen USDT, receive BTC
            accounts
                .get_mut(&trade.buyer_user_id)
                .unwrap()
                .settle_as_buyer(quote_asset_id, base_asset_id, trade_cost, trade.qty, 0)
                .expect("Buyer settlement failed");

            // Seller (Alice): spend frozen BTC, receive USDT
            accounts
                .get_mut(&trade.seller_user_id)
                .unwrap()
                .settle_as_seller(base_asset_id, quote_asset_id, trade.qty, trade_cost, 0)
                .expect("Seller settlement failed");
        }

        // Refund unused frozen funds
        let filled_cost: u64 = result
            .trades
            .iter()
            .map(|t| t.price * t.qty / qty_unit)
            .sum();
        let refund = cost - filled_cost;
        if refund > 0 {
            accounts
                .get_mut(&bob)
                .unwrap()
                .get_balance_mut(USDT)
                .unwrap()
                .unlock(refund)
                .expect("Refund failed");
        }

        println!("    Order status: {:?}", result.order.status);
    } else {
        println!("    REJECTED: Insufficient USDT balance");
    }

    // [3] Final balances
    println!("\n[3] Final balances:");
    let alice_acc = accounts.get(&alice).unwrap();
    let bob_acc = accounts.get(&bob).unwrap();

    println!(
        "    Alice: {} BTC (frozen: {}), {} USDT",
        format_decimal(
            alice_acc.get_balance(BTC).map(|b| b.avail()).unwrap_or(0),
            qty_decimal
        ),
        format_decimal(
            alice_acc.get_balance(BTC).map(|b| b.frozen()).unwrap_or(0),
            qty_decimal
        ),
        format_decimal(
            alice_acc.get_balance(USDT).map(|b| b.avail()).unwrap_or(0),
            price_decimal
        )
    );
    println!(
        "    Bob:   {} BTC, {} USDT (frozen: {})",
        format_decimal(
            bob_acc.get_balance(BTC).map(|b| b.avail()).unwrap_or(0),
            qty_decimal
        ),
        format_decimal(
            bob_acc.get_balance(USDT).map(|b| b.avail()).unwrap_or(0),
            price_decimal
        ),
        format_decimal(
            bob_acc.get_balance(USDT).map(|b| b.frozen()).unwrap_or(0),
            price_decimal
        )
    );

    println!(
        "\n    Book: Best Bid={:?}, Best Ask={:?}",
        book.best_bid().map(|p| format_decimal(p, price_decimal)),
        book.best_ask().map(|p| format_decimal(p, price_decimal))
    );

    println!("\n=== End of Simulation ===");
}
