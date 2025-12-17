//! Matching Engine - InternalOrder matching and trade execution
//!
//! The engine handles:
//! 1. Matching incoming orders against the order book
//! 2. Generating trades
//! 3. Updating order status

use crate::models::{InternalOrder, OrderResult, OrderStatus, OrderType, Side, Trade};
use crate::orderbook::OrderBook;

/// Pending trade info (before final Trade creation)
struct PendingTrade {
    buyer_order_id: u64,
    seller_order_id: u64,
    buyer_user_id: u64,
    seller_user_id: u64,
    price: u64,
    qty: u64,
}

/// Matching Engine that processes orders and generates trades
pub struct MatchingEngine;

impl MatchingEngine {
    /// Process an order: match against book and return result
    ///
    /// # Flow:
    /// 1. Try to match against opposite side
    /// 2. Update order status based on fill result
    /// 3. If partially filled or unfilled, rest in book
    /// 4. Return order status and any trades generated
    pub fn process_order(book: &mut OrderBook, mut order: InternalOrder) -> OrderResult {
        let pending_trades = match order.side {
            Side::Buy => Self::match_buy(book, &mut order),
            Side::Sell => Self::match_sell(book, &mut order),
        };

        // Convert pending trades to actual trades with IDs
        let trades: Vec<Trade> = pending_trades
            .into_iter()
            .map(|pt| {
                let trade_id = book.next_trade_id();
                Trade::new(
                    trade_id,
                    pt.buyer_order_id,
                    pt.seller_order_id,
                    pt.buyer_user_id,
                    pt.seller_user_id,
                    pt.price,
                    pt.qty,
                )
            })
            .collect();

        // Update status and rest order (matches original logic exactly)
        if order.is_filled() {
            order.status = OrderStatus::FILLED;
        } else if order.filled_qty > 0 {
            // Partially filled, rest remaining
            order.status = OrderStatus::PARTIALLY_FILLED;
            if order.order_type == OrderType::Limit {
                book.rest_order(order.clone());
            }
        } else {
            // No fills, rest entire order
            if order.order_type == OrderType::Limit {
                book.rest_order(order.clone());
            }
        }

        OrderResult { order, trades }
    }

    /// Match a buy order against asks (sell orders)
    /// NOTE: Does NOT update status - that's done in process_order
    fn match_buy(book: &mut OrderBook, buy_order: &mut InternalOrder) -> Vec<PendingTrade> {
        let mut pending_trades = Vec::new();
        let mut prices_to_remove = Vec::new();

        // Get sorted prices first to avoid borrow issues
        let prices: Vec<u64> = book.asks().keys().copied().collect();

        for price in prices {
            // Buy order can match asks at or below its price
            if buy_order.order_type == OrderType::Limit && price > buy_order.price {
                break;
            }

            if buy_order.is_filled() {
                break;
            }

            // Get orders at this price level
            if let Some(orders) = book.asks_mut().get_mut(&price) {
                while let Some(sell_order) = orders.front_mut() {
                    if buy_order.is_filled() {
                        break;
                    }

                    let trade_qty = buy_order.remaining_qty().min(sell_order.remaining_qty());
                    let trade_price = sell_order.price; // Maker price

                    // Update filled quantities
                    buy_order.filled_qty += trade_qty;
                    sell_order.filled_qty += trade_qty;

                    // Record pending trade
                    pending_trades.push(PendingTrade {
                        buyer_order_id: buy_order.order_id,
                        seller_order_id: sell_order.order_id,
                        buyer_user_id: buy_order.user_id,
                        seller_user_id: sell_order.user_id,
                        price: trade_price,
                        qty: trade_qty,
                    });

                    // Remove filled sell order from book
                    if sell_order.is_filled() {
                        orders.pop_front();
                    }
                }

                if orders.is_empty() {
                    prices_to_remove.push(price);
                }
            }
        }

        // Clean up empty price levels
        for price in prices_to_remove {
            book.asks_mut().remove(&price);
        }

        pending_trades
    }

    /// Match a sell order against bids (buy orders)
    /// NOTE: Does NOT update status - that's done in process_order
    fn match_sell(book: &mut OrderBook, sell_order: &mut InternalOrder) -> Vec<PendingTrade> {
        let mut pending_trades = Vec::new();
        let mut keys_to_remove = Vec::new();

        // Get sorted keys first to avoid borrow issues
        let keys: Vec<u64> = book.bids().keys().copied().collect();

        for key in keys {
            let bid_price = u64::MAX - key;

            // Sell order can match bids at or above its price
            if sell_order.order_type == OrderType::Limit && bid_price < sell_order.price {
                break;
            }

            if sell_order.is_filled() {
                break;
            }

            if let Some(orders) = book.bids_mut().get_mut(&key) {
                while let Some(buy_order) = orders.front_mut() {
                    if sell_order.is_filled() {
                        break;
                    }

                    let trade_qty = sell_order.remaining_qty().min(buy_order.remaining_qty());
                    let trade_price = buy_order.price; // Maker price

                    // Update filled quantities
                    sell_order.filled_qty += trade_qty;
                    buy_order.filled_qty += trade_qty;

                    // Record pending trade
                    pending_trades.push(PendingTrade {
                        buyer_order_id: buy_order.order_id,
                        seller_order_id: sell_order.order_id,
                        buyer_user_id: buy_order.user_id,
                        seller_user_id: sell_order.user_id,
                        price: trade_price,
                        qty: trade_qty,
                    });

                    // Remove filled buy order from book
                    if buy_order.is_filled() {
                        orders.pop_front();
                    }
                }

                if orders.is_empty() {
                    keys_to_remove.push(key);
                }
            }
        }

        // Clean up empty price levels
        for key in keys_to_remove {
            book.bids_mut().remove(&key);
        }

        pending_trades
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_order(id: u64, user_id: u64, price: u64, qty: u64, side: Side) -> InternalOrder {
        InternalOrder::new(id, user_id, 0, price, qty, side) // symbol_id=0
    }

    #[test]
    fn test_add_resting_order() {
        let mut book = OrderBook::new();

        let order = make_order(1, 1, 100, 10, Side::Buy);
        let result = MatchingEngine::process_order(&mut book, order);

        assert!(result.trades.is_empty());
        assert_eq!(result.order.status, OrderStatus::NEW);
        assert_eq!(book.best_bid(), Some(100));
    }

    #[test]
    fn test_full_match() {
        let mut book = OrderBook::new();

        // Add sell order first
        let sell = make_order(1, 1, 100, 10, Side::Sell);
        MatchingEngine::process_order(&mut book, sell);

        // Add matching buy order
        let buy = make_order(2, 2, 100, 10, Side::Buy);
        let result = MatchingEngine::process_order(&mut book, buy);

        assert_eq!(result.trades.len(), 1);
        assert_eq!(result.trades[0].qty, 10);
        assert_eq!(result.trades[0].price, 100);
        assert_eq!(result.order.status, OrderStatus::FILLED);
    }

    #[test]
    fn test_partial_match() {
        let mut book = OrderBook::new();

        let sell = make_order(1, 1, 100, 10, Side::Sell);
        MatchingEngine::process_order(&mut book, sell);

        let buy = make_order(2, 2, 100, 15, Side::Buy);
        let result = MatchingEngine::process_order(&mut book, buy);

        assert_eq!(result.trades.len(), 1);
        assert_eq!(result.trades[0].qty, 10);
        assert_eq!(result.order.filled_qty, 10);
        assert_eq!(result.order.status, OrderStatus::PARTIALLY_FILLED);
        // Remaining 5 should be rested
        assert_eq!(book.best_bid(), Some(100));
    }

    #[test]
    fn test_price_priority() {
        let mut book = OrderBook::new();

        // Add sells at different prices
        MatchingEngine::process_order(&mut book, make_order(1, 1, 102, 5, Side::Sell));
        MatchingEngine::process_order(&mut book, make_order(2, 2, 100, 5, Side::Sell));
        MatchingEngine::process_order(&mut book, make_order(3, 3, 101, 5, Side::Sell));

        // Buy should match lowest price first
        let buy = make_order(4, 4, 102, 12, Side::Buy);
        let result = MatchingEngine::process_order(&mut book, buy);

        assert_eq!(result.trades.len(), 3);
        assert_eq!(result.trades[0].price, 100); // Best (lowest) first
        assert_eq!(result.trades[1].price, 101);
        assert_eq!(result.trades[2].price, 102);
    }

    #[test]
    fn test_fifo_at_same_price() {
        let mut book = OrderBook::new();

        // Add two sells at same price
        MatchingEngine::process_order(&mut book, make_order(1, 1, 100, 5, Side::Sell));
        MatchingEngine::process_order(&mut book, make_order(2, 2, 100, 5, Side::Sell));

        // Buy should match first order (FIFO)
        let buy = make_order(3, 3, 100, 3, Side::Buy);
        let result = MatchingEngine::process_order(&mut book, buy);

        assert_eq!(result.trades.len(), 1);
        assert_eq!(result.trades[0].seller_order_id, 1); // First order
    }

    #[test]
    fn test_spread() {
        let mut book = OrderBook::new();

        MatchingEngine::process_order(&mut book, make_order(1, 1, 100, 10, Side::Buy));
        MatchingEngine::process_order(&mut book, make_order(2, 2, 102, 10, Side::Sell));

        assert_eq!(book.best_bid(), Some(100));
        assert_eq!(book.best_ask(), Some(102));
        assert_eq!(book.spread(), Some(2));
    }

    #[test]
    fn test_multiple_trades_single_order() {
        let mut book = OrderBook::new();

        // Add multiple small sells
        MatchingEngine::process_order(&mut book, make_order(1, 1, 100, 3, Side::Sell));
        MatchingEngine::process_order(&mut book, make_order(2, 2, 101, 4, Side::Sell));
        MatchingEngine::process_order(&mut book, make_order(3, 3, 102, 5, Side::Sell));

        // One large buy
        let buy = make_order(4, 4, 102, 10, Side::Buy);
        let result = MatchingEngine::process_order(&mut book, buy);

        assert_eq!(result.trades.len(), 3);
        assert_eq!(result.order.filled_qty, 10); // 3 + 4 + 3 from third
    }
}
