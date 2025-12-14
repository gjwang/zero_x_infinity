mod models;
mod engine;

use models::{Order, Side};
use engine::OrderBook;


fn main() {
    let mut book = OrderBook::new();
    println!("--- 0xInfinity: Stage 2 (Integer) ---");

    // 1. Makers (Sells)
    println!("\n[1] Makers coming in...");
    book.add_order(Order::new(1, 100, 10, Side::Sell));
    book.add_order(Order::new(2, 102, 5, Side::Sell)); 
    book.add_order(Order::new(3, 101, 5, Side::Sell));

    // 2. Taker (Buy)
    // Price 1015 (represents 101.5 in decimal), Qty 12
    // - Matches 10 @ 100. Remaining need: 2.
    // - Matches 2 @ 101. Remaining need: 0.
    // Order #3 (Sell @ 101) had 5, now has 3 left.
    println!("\n[2] Taker eats liquidity...");
    book.add_order(Order::new(4, 1015, 12, Side::Buy));

    // 3. Maker (Buy)
    println!("\n[3] More makers...");
    book.add_order(Order::new(5, 99, 10, Side::Buy));

    println!("\n--- End of Simulation ---");
}