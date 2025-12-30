//! Matching Engine - InternalOrder matching and trade execution
//!
//! The engine handles:
//! 1. Matching incoming orders against the order book
//! 2. Generating trades
//! 3. Updating order status

use crate::models::{InternalOrder, OrderResult, OrderStatus, OrderType, Side, TimeInForce, Trade};
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
    /// 2. Update order status based on fill result and TimeInForce
    /// 3. If GTC and partially filled or unfilled, rest in book
    /// 4. If IOC, expire any unfilled remainder (NEVER rest)
    /// 5. Return order status and any trades generated
    pub fn process_order(book: &mut OrderBook, mut order: InternalOrder) -> OrderResult {
        let (pending_trades, maker_orders) = match order.side {
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

        // Update status based on fill result and TimeInForce
        if order.is_filled() {
            order.status = OrderStatus::FILLED;
        } else if order.time_in_force == TimeInForce::IOC {
            // IOC: expire any unfilled remainder, NEVER rest in book
            order.status = OrderStatus::EXPIRED;
            // IOC orders NEVER rest in book
        } else {
            // GTC: rest unfilled remainder in book
            if order.filled_qty > 0 {
                order.status = OrderStatus::PARTIALLY_FILLED;
            }
            // Rest in book if LIMIT order
            if order.order_type == OrderType::Limit {
                book.rest_order(order.clone());
            }
        }

        OrderResult {
            order,
            trades,
            maker_orders,
        }
    }

    /// Reduce an order's quantity in-place (preserves time priority)
    ///
    /// This modifies the order's qty without changing its position in the queue.
    /// If reduce_by >= remaining qty, the order is removed entirely.
    ///
    /// # Arguments
    /// * `book` - The order book
    /// * `order_id` - ID of order to reduce
    /// * `reduce_by` - Amount to reduce qty by
    ///
    /// # Returns
    /// * `Some(order)` - The modified (or removed) order
    /// * `None` - Order not found
    pub fn reduce_order(
        book: &mut OrderBook,
        order_id: u64,
        reduce_by: u64,
    ) -> Option<InternalOrder> {
        // First check if reducing to zero or below
        if let Some(order) = book.get_order_mut(order_id) {
            let remaining = order.qty.saturating_sub(order.filled_qty);
            if reduce_by >= remaining {
                // Remove entirely
                book.remove_order_by_id(order_id)
            } else {
                // Reduce in place (preserves priority)
                order.qty = order.qty.saturating_sub(reduce_by);
                Some(order.clone())
            }
        } else {
            None
        }
    }

    /// Move an order to a new price (Cancel + Place, loses priority)
    ///
    /// This is implemented as atomic cancel + place at new price.
    /// The order loses its time priority (equivalent to cancel and re-submit).
    ///
    /// # Arguments
    /// * `book` - The order book
    /// * `order_id` - ID of order to move
    /// * `new_price` - New price for the order
    ///
    /// # Returns
    /// * `Some(order)` - The moved order with new price
    /// * `None` - Order not found
    pub fn move_order(
        book: &mut OrderBook,
        order_id: u64,
        new_price: u64,
    ) -> Option<InternalOrder> {
        // Cancel the existing order
        let mut order = book.remove_order_by_id(order_id)?;

        // Update price and re-insert
        order.price = new_price;
        book.rest_order(order.clone());

        Some(order)
    }

    /// Match a buy order against asks (sell orders)
    /// NOTE: Does NOT update status - that's done in process_order
    fn match_buy(
        book: &mut OrderBook,
        buy_order: &mut InternalOrder,
    ) -> (Vec<PendingTrade>, Vec<InternalOrder>) {
        let mut pending_trades = Vec::new();
        let mut maker_orders = Vec::new();
        let mut prices_to_remove = Vec::new();
        let mut filled_order_ids = Vec::new();

        // Only collect prices within matching range (optimization: avoid full key copy)
        let max_price = if buy_order.order_type == OrderType::Limit {
            buy_order.price
        } else {
            u64::MAX // Market order matches all
        };
        let prices: Vec<u64> = book.asks().range(..=max_price).map(|(&k, _)| k).collect();

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

                    // Update maker status before cloning for persistence
                    sell_order.status = if sell_order.is_filled() {
                        OrderStatus::FILLED
                    } else {
                        OrderStatus::PARTIALLY_FILLED
                    };

                    // Record maker state update
                    maker_orders.push(sell_order.clone());

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
                        filled_order_ids.push(sell_order.order_id);
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

        // Remove filled orders from index
        for order_id in filled_order_ids {
            book.remove_from_index(order_id);
        }

        (pending_trades, maker_orders)
    }

    /// Match a sell order against bids (buy orders)
    /// NOTE: Does NOT update status - that's done in process_order
    fn match_sell(
        book: &mut OrderBook,
        sell_order: &mut InternalOrder,
    ) -> (Vec<PendingTrade>, Vec<InternalOrder>) {
        let mut pending_trades = Vec::new();
        let mut maker_orders = Vec::new();
        let mut keys_to_remove = Vec::new();
        let mut filled_order_ids = Vec::new();

        // Only collect keys within matching range (optimization: avoid full key copy)
        // Bids are stored with key = u64::MAX - price, so higher prices come first
        // For a sell at price P, we match bids with price >= P, i.e., key <= u64::MAX - P
        let max_key = if sell_order.order_type == OrderType::Limit {
            u64::MAX - sell_order.price
        } else {
            u64::MAX // Market order matches all
        };
        let keys: Vec<u64> = book.bids().range(..=max_key).map(|(&k, _)| k).collect();

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

                    // Update maker status before cloning for persistence
                    buy_order.status = if buy_order.is_filled() {
                        OrderStatus::FILLED
                    } else {
                        OrderStatus::PARTIALLY_FILLED
                    };

                    // Record maker state update
                    maker_orders.push(buy_order.clone());

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
                        filled_order_ids.push(buy_order.order_id);
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

        // Remove filled orders from index
        for order_id in filled_order_ids {
            book.remove_from_index(order_id);
        }

        (pending_trades, maker_orders)
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

    // =========================================================
    // IOC (Immediate-or-Cancel) Tests
    // =========================================================

    fn make_ioc_order(id: u64, user_id: u64, price: u64, qty: u64, side: Side) -> InternalOrder {
        let mut order = InternalOrder::new(id, user_id, 0, price, qty, side);
        order.time_in_force = TimeInForce::IOC;
        order
    }

    #[test]
    fn test_ioc_full_match() {
        let mut book = OrderBook::new();

        // Add sell order (GTC, will rest)
        let sell = make_order(1, 1, 100, 10, Side::Sell);
        MatchingEngine::process_order(&mut book, sell);

        // IOC buy matches fully
        let ioc_buy = make_ioc_order(2, 2, 100, 10, Side::Buy);
        let result = MatchingEngine::process_order(&mut book, ioc_buy);

        assert_eq!(result.trades.len(), 1);
        assert_eq!(result.order.status, OrderStatus::FILLED);
        assert_eq!(result.order.filled_qty, 10);
    }

    #[test]
    fn test_ioc_partial_fill_expire() {
        let mut book = OrderBook::new();

        // Add sell order with only 60 qty
        let sell = make_order(1, 1, 100, 60, Side::Sell);
        MatchingEngine::process_order(&mut book, sell);

        // IOC buy for 100 qty - should fill 60, expire 40
        let ioc_buy = make_ioc_order(2, 2, 100, 100, Side::Buy);
        let result = MatchingEngine::process_order(&mut book, ioc_buy);

        assert_eq!(result.trades.len(), 1);
        assert_eq!(result.trades[0].qty, 60);
        assert_eq!(result.order.filled_qty, 60);
        assert_eq!(result.order.status, OrderStatus::EXPIRED); // Remainder expired
    }

    #[test]
    fn test_ioc_no_match_expire() {
        let mut book = OrderBook::new();

        // Empty book - IOC should expire immediately
        let ioc_buy = make_ioc_order(1, 1, 100, 10, Side::Buy);
        let result = MatchingEngine::process_order(&mut book, ioc_buy);

        assert!(result.trades.is_empty());
        assert_eq!(result.order.status, OrderStatus::EXPIRED);
    }

    #[test]
    fn test_ioc_never_rests_in_book() {
        let mut book = OrderBook::new();

        // IOC buy with no matching orders
        let ioc_buy = make_ioc_order(1, 1, 100, 10, Side::Buy);
        MatchingEngine::process_order(&mut book, ioc_buy);

        // Verify IOC order is NOT in the book
        assert!(book.best_bid().is_none(), "IOC should never rest in book");
        assert!(
            book.all_orders().is_empty(),
            "IOC should never be in all_orders()"
        );
    }

    #[test]
    fn test_ioc_partial_fill_never_rests() {
        let mut book = OrderBook::new();

        // Add small sell order
        let sell = make_order(1, 1, 100, 5, Side::Sell);
        MatchingEngine::process_order(&mut book, sell);

        // IOC buy for 10, fills 5, remainder should NOT rest
        let ioc_buy = make_ioc_order(2, 2, 100, 10, Side::Buy);
        MatchingEngine::process_order(&mut book, ioc_buy);

        // Book should be empty (sell consumed, IOC didn't rest)
        assert!(book.best_bid().is_none(), "IOC remainder should not rest");
        assert!(book.best_ask().is_none(), "Sell should be consumed");
    }

    // =========================================================
    // ReduceOrder Tests
    // =========================================================

    #[test]
    fn test_reduce_order_preserves_priority() {
        let mut book = OrderBook::new();

        // Add two orders at same price
        let order1 = make_order(1, 1, 100, 100, Side::Buy);
        let order2 = make_order(2, 2, 100, 50, Side::Buy);
        MatchingEngine::process_order(&mut book, order1);
        MatchingEngine::process_order(&mut book, order2);

        // Reduce order 1 by 30
        let result = MatchingEngine::reduce_order(&mut book, 1, 30);

        assert!(result.is_some());
        let reduced = result.unwrap();
        assert_eq!(reduced.order_id, 1);
        assert_eq!(reduced.qty, 70); // 100 - 30

        // Order 1 should still be first in queue (priority preserved)
        let orders = book.all_orders();
        assert_eq!(orders.len(), 2);
        assert_eq!(orders[0].order_id, 1);
        assert_eq!(orders[1].order_id, 2);
    }

    #[test]
    fn test_reduce_order_to_zero_removes() {
        let mut book = OrderBook::new();

        let order = make_order(1, 1, 100, 100, Side::Buy);
        MatchingEngine::process_order(&mut book, order);

        // Reduce by full amount
        let result = MatchingEngine::reduce_order(&mut book, 1, 100);

        assert!(result.is_some());
        assert!(book.all_orders().is_empty(), "Order should be removed");
        assert!(book.best_bid().is_none());
    }

    #[test]
    fn test_reduce_order_nonexistent() {
        let mut book = OrderBook::new();

        let result = MatchingEngine::reduce_order(&mut book, 999, 10);
        assert!(result.is_none());
    }

    // =========================================================
    // MoveOrder Tests
    // =========================================================

    #[test]
    fn test_move_order_changes_price() {
        let mut book = OrderBook::new();

        let order = make_order(1, 1, 100, 10, Side::Buy);
        MatchingEngine::process_order(&mut book, order);

        assert_eq!(book.best_bid(), Some(100));

        // Move to new price
        let result = MatchingEngine::move_order(&mut book, 1, 105);

        assert!(result.is_some());
        let moved = result.unwrap();
        assert_eq!(moved.price, 105);
        assert_eq!(book.best_bid(), Some(105));
    }

    #[test]
    fn test_move_order_loses_priority() {
        let mut book = OrderBook::new();

        // Add two orders at same price
        let order1 = make_order(1, 1, 100, 10, Side::Buy);
        let order2 = make_order(2, 2, 100, 10, Side::Buy);
        MatchingEngine::process_order(&mut book, order1);
        MatchingEngine::process_order(&mut book, order2);

        // Order 1 has priority (inserted first)
        let orders_before = book.all_orders();
        assert_eq!(orders_before[0].order_id, 1);

        // Move order 1 to same price level
        MatchingEngine::move_order(&mut book, 1, 100);

        // Order 1 should now be AFTER order 2 (priority lost)
        let orders_after = book.all_orders();
        assert_eq!(orders_after.len(), 2);
        assert_eq!(orders_after[0].order_id, 2);
        assert_eq!(orders_after[1].order_id, 1);
    }

    #[test]
    fn test_move_order_nonexistent() {
        let mut book = OrderBook::new();

        let result = MatchingEngine::move_order(&mut book, 999, 100);
        assert!(result.is_none());
    }
}
