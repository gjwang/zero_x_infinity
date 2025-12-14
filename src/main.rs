mod models;
mod engine;

use models::{Order, Side};
use engine::OrderBook;

fn main() {
    let mut book = OrderBook::new();
    println!("--- 0xInfinity: Stage 1 (Genesis) ---");

    // 1. Makers (Sells)
    println!("\n[1] Makers coming in...");
    book.add_order(Order::new(1, 100.0, 10.0, Side::Sell));
    book.add_order(Order::new(2, 102.0, 5.0, Side::Sell)); 
    book.add_order(Order::new(3, 101.0, 5.0, Side::Sell));

    // 2. Taker (Buy)
    // Price 101.5, Qty 12
    // Expected: Eats 100.0 (10), Eats 101.0 (2), Rests on 101.0 (0) - logic check:
    // Actually, Taker buys 12. 
    // - Matches 10 @ 100.0. Remaining need: 2.
    // - Matches 2 @ 101.0. Remaining need: 0.
    // Order #3 (Sell @ 101.0) had 5, now has 3 left.
    println!("\n[2] Taker eats liquidity...");
    book.add_order(Order::new(4, 101.5, 12.0, Side::Buy));

    // 3. Maker (Buy)
    println!("\n[3] More makers...");
    book.add_order(Order::new(5, 99.0, 10.0, Side::Buy));

    println!("\n--- End of Simulation ---");
}