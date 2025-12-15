// engine.rs - BTreeMap-based OrderBook implementation

use crate::models::{Order, OrderResult, OrderStatus, Side, Trade};
use std::collections::{BTreeMap, VecDeque};

/// A price level containing orders at the same price (FIFO queue)
type PriceLevel = VecDeque<Order>;

/// The OrderBook using BTreeMap for O(log n) operations
///
/// Key insight:
/// - Sells are stored normally (ascending order, lowest price = best ask)
/// - Buys are stored with negated keys (so highest price comes first = best bid)
///   We use `u64::MAX - price` as the key for buys
pub struct OrderBook {
    /// Sell orders: price -> orders at that price
    /// BTreeMap keeps keys sorted ascending, so first entry = best ask (lowest price)
    asks: BTreeMap<u64, PriceLevel>,

    /// Buy orders: (u64::MAX - price) -> orders at that price  
    /// This trick makes highest price appear first when iterating
    bids: BTreeMap<u64, PriceLevel>,

    /// Trade ID counter
    trade_id_counter: u64,
}

impl OrderBook {
    pub fn new() -> Self {
        Self {
            asks: BTreeMap::new(),
            bids: BTreeMap::new(),
            trade_id_counter: 0,
        }
    }

    /// Add an order to the book, performing matching if possible
    pub fn add_order(&mut self, mut order: Order) -> OrderResult {
        let mut trades = Vec::new();

        // 1. Try to match the order
        match order.side {
            Side::Buy => self.match_buy(&mut order, &mut trades),
            Side::Sell => self.match_sell(&mut order, &mut trades),
        }

        // 2. Update order status
        if order.is_filled() {
            order.status = OrderStatus::Filled;
        } else if order.filled_qty > 0 {
            order.status = OrderStatus::PartiallyFilled;
            // Rest the remaining quantity
            // INVARIANT: If order triggered a match (filled_qty > 0), then at its price level
            // on the same side, there can't be prior orders (they would have matched first)
            debug_assert!(
                self.qty_at_price(order.price, order.side) == 0,
                "Partially filled order resting at non-empty price level: price={}, side={:?}",
                order.price,
                order.side
            );
            self.rest_order(order.clone());
        } else {
            // No fills, rest the entire order
            self.rest_order(order.clone());
        }

        OrderResult { order, trades }
    }

    /// Match a buy order against asks
    fn match_buy(&mut self, buy_order: &mut Order, trades: &mut Vec<Trade>) {
        // Iterate asks in ascending order (lowest price first = best ask)
        let mut empty_prices = Vec::new();

        for (&price, level) in self.asks.iter_mut() {
            // If buy price < ask price, no more matches possible
            if buy_order.price < price {
                break;
            }

            while let Some(sell_order) = level.front_mut() {
                let trade_qty = u64::min(buy_order.remaining_qty(), sell_order.remaining_qty());

                // Create trade
                self.trade_id_counter += 1;
                trades.push(Trade::new(
                    self.trade_id_counter,
                    buy_order.id,
                    sell_order.id,
                    price, // Trade at the resting order's price (maker price)
                    trade_qty,
                ));

                // Update quantities
                buy_order.filled_qty += trade_qty;
                sell_order.filled_qty += trade_qty;

                // Remove filled sell order
                if sell_order.is_filled() {
                    level.pop_front();
                }

                // Check if buy order is done
                if buy_order.is_filled() {
                    break;
                }
            }

            // Mark empty levels for removal
            if level.is_empty() {
                empty_prices.push(price);
            }

            if buy_order.is_filled() {
                break;
            }
        }

        // Remove empty price levels (O(log n) per removal)
        for price in empty_prices {
            self.asks.remove(&price);
        }
    }

    /// Match a sell order against bids
    fn match_sell(&mut self, sell_order: &mut Order, trades: &mut Vec<Trade>) {
        let mut empty_keys = Vec::new();

        // Iterate bids - keys are (u64::MAX - price), so iteration gives highest price first
        for (&key, level) in self.bids.iter_mut() {
            let price = u64::MAX - key; // Convert back to actual price

            // If sell price > bid price, no more matches possible
            if sell_order.price > price {
                break;
            }

            while let Some(buy_order) = level.front_mut() {
                let trade_qty = u64::min(sell_order.remaining_qty(), buy_order.remaining_qty());

                // Create trade
                self.trade_id_counter += 1;
                trades.push(Trade::new(
                    self.trade_id_counter,
                    buy_order.id,
                    sell_order.id,
                    price, // Trade at the resting order's price (maker price)
                    trade_qty,
                ));

                // Update quantities
                sell_order.filled_qty += trade_qty;
                buy_order.filled_qty += trade_qty;

                // Remove filled buy order
                if buy_order.is_filled() {
                    level.pop_front();
                }

                // Check if sell order is done
                if sell_order.is_filled() {
                    break;
                }
            }

            // Mark empty levels for removal
            if level.is_empty() {
                empty_keys.push(key);
            }

            if sell_order.is_filled() {
                break;
            }
        }

        // Remove empty price levels
        for key in empty_keys {
            self.bids.remove(&key);
        }
    }

    /// Rest an unfilled/partially filled order in the book
    fn rest_order(&mut self, order: Order) {
        match order.side {
            Side::Buy => {
                let key = u64::MAX - order.price; // Invert for descending order
                self.bids
                    .entry(key)
                    .or_insert_with(VecDeque::new)
                    .push_back(order);
            }
            Side::Sell => {
                self.asks
                    .entry(order.price)
                    .or_insert_with(VecDeque::new)
                    .push_back(order);
            }
        }
    }

    /// Get the best bid price (highest buy price)
    pub fn best_bid(&self) -> Option<u64> {
        self.bids.first_key_value().map(|(&key, _)| u64::MAX - key)
    }

    /// Get the best ask price (lowest sell price)
    pub fn best_ask(&self) -> Option<u64> {
        self.asks.first_key_value().map(|(&price, _)| price)
    }

    /// Get the spread (difference between best ask and best bid)
    pub fn spread(&self) -> Option<u64> {
        match (self.best_ask(), self.best_bid()) {
            (Some(ask), Some(bid)) if ask > bid => Some(ask - bid),
            _ => None,
        }
    }

    /// Get total quantity at a price level for a side
    pub fn qty_at_price(&self, price: u64, side: Side) -> u64 {
        match side {
            Side::Buy => {
                let key = u64::MAX - price;
                self.bids
                    .get(&key)
                    .map(|level| level.iter().map(|o| o.remaining_qty()).sum())
                    .unwrap_or(0)
            }
            Side::Sell => self
                .asks
                .get(&price)
                .map(|level| level.iter().map(|o| o.remaining_qty()).sum())
                .unwrap_or(0),
        }
    }

    /// Cancel an order by ID (returns true if found and cancelled)
    pub fn cancel_order(&mut self, order_id: u64, price: u64, side: Side) -> bool {
        let book = match side {
            Side::Buy => {
                let key = u64::MAX - price;
                self.bids.get_mut(&key)
            }
            Side::Sell => self.asks.get_mut(&price),
        };

        if let Some(level) = book {
            if let Some(pos) = level.iter().position(|o| o.id == order_id) {
                level.remove(pos);
                // Clean up empty level
                if level.is_empty() {
                    match side {
                        Side::Buy => {
                            self.bids.remove(&(u64::MAX - price));
                        }
                        Side::Sell => {
                            self.asks.remove(&price);
                        }
                    }
                }
                return true;
            }
        }
        false
    }

    /// Get number of price levels on each side
    pub fn depth(&self) -> (usize, usize) {
        (self.bids.len(), self.asks.len())
    }
}

impl Default for OrderBook {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_resting_order() {
        let mut book = OrderBook::new();

        // Add a buy order - should rest since no sells
        let result = book.add_order(Order::new(1, 100, 10, Side::Buy));

        assert_eq!(result.trades.len(), 0);
        assert_eq!(result.order.filled_qty, 0);
        assert_eq!(book.best_bid(), Some(100));
        assert_eq!(book.best_ask(), None);
    }

    #[test]
    fn test_full_match() {
        let mut book = OrderBook::new();

        // Add a sell order at 100
        book.add_order(Order::new(1, 100, 10, Side::Sell));
        assert_eq!(book.best_ask(), Some(100));

        // Add a buy order at 100 - should fully match
        let result = book.add_order(Order::new(2, 100, 10, Side::Buy));

        assert_eq!(result.trades.len(), 1);
        assert_eq!(result.trades[0].qty, 10);
        assert_eq!(result.trades[0].price, 100);
        assert_eq!(result.order.status, OrderStatus::Filled);
        assert_eq!(book.best_ask(), None); // Sell order consumed
        assert_eq!(book.best_bid(), None); // Buy order fully filled
    }

    #[test]
    fn test_partial_match() {
        let mut book = OrderBook::new();

        // Add a sell order for 10 units
        book.add_order(Order::new(1, 100, 10, Side::Sell));

        // Add a buy order for 15 units - should partially match
        let result = book.add_order(Order::new(2, 100, 15, Side::Buy));

        assert_eq!(result.trades.len(), 1);
        assert_eq!(result.trades[0].qty, 10);
        assert_eq!(result.order.filled_qty, 10);
        assert_eq!(result.order.remaining_qty(), 5);
        assert_eq!(result.order.status, OrderStatus::PartiallyFilled);

        // Remaining 5 units should be resting
        assert_eq!(book.best_bid(), Some(100));
        assert_eq!(book.qty_at_price(100, Side::Buy), 5);
    }

    #[test]
    fn test_price_priority() {
        let mut book = OrderBook::new();

        // Add sells at different prices
        book.add_order(Order::new(1, 102, 5, Side::Sell));
        book.add_order(Order::new(2, 100, 5, Side::Sell)); // Best ask
        book.add_order(Order::new(3, 101, 5, Side::Sell));

        assert_eq!(book.best_ask(), Some(100));

        // Buy should match best price first
        let result = book.add_order(Order::new(4, 105, 10, Side::Buy));

        // Should match order 2 (price 100) then order 3 (price 101)
        assert_eq!(result.trades.len(), 2);
        assert_eq!(result.trades[0].price, 100);
        assert_eq!(result.trades[0].seller_order_id, 2);
        assert_eq!(result.trades[1].price, 101);
        assert_eq!(result.trades[1].seller_order_id, 3);
    }

    #[test]
    fn test_fifo_at_same_price() {
        let mut book = OrderBook::new();

        // Add sells at same price - FIFO order
        book.add_order(Order::new(1, 100, 5, Side::Sell)); // First
        book.add_order(Order::new(2, 100, 5, Side::Sell)); // Second

        // Buy should match first order first
        let result = book.add_order(Order::new(3, 100, 5, Side::Buy));

        assert_eq!(result.trades.len(), 1);
        assert_eq!(result.trades[0].seller_order_id, 1); // FIFO: order 1 matched first
    }

    #[test]
    fn test_cancel_order() {
        let mut book = OrderBook::new();

        book.add_order(Order::new(1, 100, 10, Side::Buy));
        book.add_order(Order::new(2, 100, 10, Side::Buy));

        assert_eq!(book.qty_at_price(100, Side::Buy), 20);

        // Cancel first order
        let cancelled = book.cancel_order(1, 100, Side::Buy);
        assert!(cancelled);
        assert_eq!(book.qty_at_price(100, Side::Buy), 10);

        // Cancel non-existent order
        let cancelled = book.cancel_order(999, 100, Side::Buy);
        assert!(!cancelled);
    }

    #[test]
    fn test_spread() {
        let mut book = OrderBook::new();

        // No orders - no spread
        assert_eq!(book.spread(), None);

        // Only buy - no spread
        book.add_order(Order::new(1, 99, 10, Side::Buy));
        assert_eq!(book.spread(), None);

        // Add sell - now we have spread
        book.add_order(Order::new(2, 101, 10, Side::Sell));
        assert_eq!(book.spread(), Some(2)); // 101 - 99 = 2
    }

    #[test]
    fn test_multiple_trades_single_order() {
        let mut book = OrderBook::new();

        // Add multiple small sells
        book.add_order(Order::new(1, 100, 3, Side::Sell));
        book.add_order(Order::new(2, 100, 3, Side::Sell));
        book.add_order(Order::new(3, 100, 4, Side::Sell));

        // One big buy eats them all
        let result = book.add_order(Order::new(4, 100, 10, Side::Buy));

        assert_eq!(result.trades.len(), 3);
        assert_eq!(result.order.status, OrderStatus::Filled);
        assert_eq!(book.best_ask(), None);
    }
}
