use crate::models::{Order, Side};
use std::collections::VecDeque;

#[derive(Debug)]
struct PriceLevel {
    price: u64,
    orders: VecDeque<Order>, // FIFO
}

pub struct OrderBook {
    buys: Vec<PriceLevel>,
    sells: Vec<PriceLevel>,
}

impl OrderBook {
    pub fn new() -> Self {
        Self {
            buys: Vec::new(),
            sells: Vec::new(),
        }
    }

    pub fn add_order(&mut self, mut order: Order) {
        // 1. Matching
        if order.side == Side::Buy {
            self.match_buy(&mut order);
        } else {
            self.match_sell(&mut order);
        }

        // 2. Resting (if not fully filled)
        if order.qty > 0 {
            let book_side = if order.side == Side::Buy {
                &mut self.buys
            } else {
                &mut self.sells
            };

            // With u64, we can use exact comparison (no floating point epsilon needed!)
            let level = book_side.iter_mut().find(|l| l.price == order.price);

            if let Some(l) = level {
                l.orders.push_back(order);
            } else {
                // New price level, allocation
                let mut orders = VecDeque::new();
                orders.push_back(order.clone());
                book_side.push(PriceLevel {
                    price: order.price,
                    orders,
                });
            }
        }
    }

    fn match_buy(&mut self, buy_order: &mut Order) {
        // Sells sorted ascending (cheapest first)
        self.sells.sort_by_key(|l| l.price);

        for level in self.sells.iter_mut() {
            // If buy price < lowest sell price, no match possible
            if buy_order.price < level.price {
                break;
            }

            while let Some(sell_order) = level.orders.front_mut() {
                let trade_qty = u64::min(buy_order.qty, sell_order.qty);

                println!(
                    "MATCH: Buy {} eats Sell {} @ Price {} (Qty: {})",
                    buy_order.id, sell_order.id, level.price, trade_qty
                );

                buy_order.qty -= trade_qty;
                sell_order.qty -= trade_qty;

                if sell_order.qty == 0 {
                    level.orders.pop_front();
                }

                if buy_order.qty == 0 {
                    return;
                }
            }
        }

        // CRIME: Memory shifting in Vec
        self.sells.retain(|l| !l.orders.is_empty());
    }

    fn match_sell(&mut self, sell_order: &mut Order) {
        // Buys sorted descending (highest first)
        self.buys.sort_by(|a, b| b.price.cmp(&a.price));

        for level in self.buys.iter_mut() {
            // If sell price > highest buy price, no match possible
            if sell_order.price > level.price {
                break;
            }

            while let Some(buy_order) = level.orders.front_mut() {
                let trade_qty = u64::min(sell_order.qty, buy_order.qty);

                println!(
                    "MATCH: Sell {} eats Buy {} @ Price {} (Qty: {})",
                    sell_order.id, buy_order.id, level.price, trade_qty
                );

                sell_order.qty -= trade_qty;
                buy_order.qty -= trade_qty;

                if buy_order.qty == 0 {
                    level.orders.pop_front();
                }

                if sell_order.qty == 0 {
                    return;
                }
            }
        }

        self.buys.retain(|l| !l.orders.is_empty());
    }
}
