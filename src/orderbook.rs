//! OrderBook - BTreeMap-based price-time priority order book
//!
//! This module contains only the OrderBook data structure.
//! The matching logic lives in the Engine module.

use crate::models::{InternalOrder, Side};
use std::collections::{BTreeMap, VecDeque};

/// The OrderBook using BTreeMap for O(log n) operations
///
/// # Key Design:
/// - Asks are stored with normal keys (ascending order, lowest price = best ask)
/// - Bids use negated keys `u64::MAX - price` (so highest price comes first = best bid)
///
/// # Complexity:
/// | Operation | Time |
/// |-----------|------|
/// | Insert | O(log n) |
/// | Best price | O(1) amortized |
/// | Cancel | O(log n + k) where k = orders at price |
#[derive(Debug)]
pub struct OrderBook {
    /// Sell orders: price -> orders (ascending, lowest = best)
    asks: BTreeMap<u64, VecDeque<InternalOrder>>,
    /// Buy orders: (MAX - price) -> orders (so highest price first)
    bids: BTreeMap<u64, VecDeque<InternalOrder>>,
    /// Trade ID counter
    pub(crate) trade_id_counter: u64,
}

impl OrderBook {
    /// Create a new empty order book
    pub fn new() -> Self {
        Self {
            asks: BTreeMap::new(),
            bids: BTreeMap::new(),
            trade_id_counter: 0,
        }
    }

    /// Get next trade ID (increments counter)
    #[inline]
    pub fn next_trade_id(&mut self) -> u64 {
        self.trade_id_counter += 1;
        self.trade_id_counter
    }

    /// Get the best bid price (highest buy price)
    #[inline]
    pub fn best_bid(&self) -> Option<u64> {
        self.bids.first_key_value().map(|(k, _)| u64::MAX - k)
    }

    /// Get the best ask price (lowest sell price)
    #[inline]
    pub fn best_ask(&self) -> Option<u64> {
        self.asks.first_key_value().map(|(k, _)| *k)
    }

    /// Get the spread (difference between best ask and best bid)
    pub fn spread(&self) -> Option<u64> {
        match (self.best_ask(), self.best_bid()) {
            (Some(ask), Some(bid)) if ask > bid => Some(ask - bid),
            _ => None,
        }
    }

    /// Get number of price levels on each side (bid_depth, ask_depth)
    #[inline]
    pub fn depth(&self) -> (usize, usize) {
        (self.bids.len(), self.asks.len())
    }

    /// Get mutable reference to asks (for matching engine)
    #[inline]
    pub fn asks_mut(&mut self) -> &mut BTreeMap<u64, VecDeque<InternalOrder>> {
        &mut self.asks
    }

    /// Get mutable reference to bids (for matching engine)
    #[inline]
    pub fn bids_mut(&mut self) -> &mut BTreeMap<u64, VecDeque<InternalOrder>> {
        &mut self.bids
    }

    /// Get immutable reference to asks
    #[inline]
    pub fn asks(&self) -> &BTreeMap<u64, VecDeque<InternalOrder>> {
        &self.asks
    }

    /// Get immutable reference to bids
    #[inline]
    pub fn bids(&self) -> &BTreeMap<u64, VecDeque<InternalOrder>> {
        &self.bids
    }

    /// Rest an unfilled/partially filled order in the book
    ///
    /// NOTE: The order status should already be set correctly by the caller.
    /// This method does NOT modify the order status - it just stores the order.
    pub fn rest_order(&mut self, order: InternalOrder) {
        match order.side {
            Side::Buy => {
                let key = u64::MAX - order.price;
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

    /// Get total quantity at a price level for a side
    pub fn qty_at_price(&self, price: u64, side: Side) -> u64 {
        match side {
            Side::Buy => {
                let key = u64::MAX - price;
                self.bids
                    .get(&key)
                    .map(|orders| orders.iter().map(|o| o.remaining_qty()).sum())
                    .unwrap_or(0)
            }
            Side::Sell => self
                .asks
                .get(&price)
                .map(|orders| orders.iter().map(|o| o.remaining_qty()).sum())
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

        if let Some(orders) = book {
            if let Some(pos) = orders.iter().position(|o| o.order_id == order_id) {
                orders.remove(pos);
                // Clean up empty price level
                if orders.is_empty() {
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

    /// Remove an order by ID only (searches entire book)
    ///
    /// Returns the removed order if found. Use this when you don't know
    /// the price/side of the order (e.g., cancel by order_id from API).
    ///
    /// Complexity: O(n) where n = total orders (must search entire book)
    pub fn remove_order_by_id(&mut self, order_id: u64) -> Option<InternalOrder> {
        // Search bids first
        for (key, orders) in self.bids.iter_mut() {
            if let Some(pos) = orders.iter().position(|o| o.order_id == order_id) {
                let order = orders.remove(pos).unwrap();
                let price = u64::MAX - *key;
                // Clean up empty price level
                if orders.is_empty() {
                    self.bids.remove(&(u64::MAX - price));
                }
                return Some(order);
            }
        }

        // Search asks
        for (key, orders) in self.asks.iter_mut() {
            if let Some(pos) = orders.iter().position(|o| o.order_id == order_id) {
                let order = orders.remove(pos).unwrap();
                let price = *key;
                // Clean up empty price level
                if orders.is_empty() {
                    self.asks.remove(&price);
                }
                return Some(order);
            }
        }

        None
    }

    /// Get all orders as a Vec (for final dump/snapshot)
    ///
    /// Returns orders in price priority order:
    /// - Bids first (highest price first, then FIFO within price)
    /// - Asks second (lowest price first, then FIFO within price)
    ///
    /// This matches the natural market depth view.
    pub fn all_orders(&self) -> Vec<&InternalOrder> {
        self.bids
            .values()
            .flat_map(|level| level.iter())
            .chain(self.asks.values().flat_map(|level| level.iter()))
            .collect()
    }

    /// Iterate over all orders in the book
    pub fn iter_orders(&self) -> impl Iterator<Item = (&InternalOrder,)> + '_ {
        self.bids
            .values()
            .flat_map(|v| v.iter())
            .chain(self.asks.values().flat_map(|v| v.iter()))
            .map(|o| (o,))
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

    fn make_order(id: u64, price: u64, qty: u64, side: Side) -> InternalOrder {
        InternalOrder::new(id, 1, 0, price, qty, side) // symbol_id=0
    }

    #[test]
    fn test_rest_order() {
        let mut book = OrderBook::new();

        let order = make_order(1, 100, 10, Side::Buy);
        book.rest_order(order);

        assert_eq!(book.best_bid(), Some(100));
        assert_eq!(book.best_ask(), None);
    }

    #[test]
    fn test_best_bid_ask() {
        let mut book = OrderBook::new();

        book.rest_order(make_order(1, 100, 10, Side::Buy));
        book.rest_order(make_order(2, 99, 10, Side::Buy));
        book.rest_order(make_order(3, 101, 10, Side::Sell));
        book.rest_order(make_order(4, 102, 10, Side::Sell));

        assert_eq!(book.best_bid(), Some(100));
        assert_eq!(book.best_ask(), Some(101));
        assert_eq!(book.spread(), Some(1));
    }

    #[test]
    fn test_cancel_order() {
        let mut book = OrderBook::new();

        book.rest_order(make_order(1, 100, 10, Side::Buy));

        assert!(book.cancel_order(1, 100, Side::Buy));
        assert_eq!(book.best_bid(), None);
    }

    #[test]
    fn test_depth() {
        let mut book = OrderBook::new();

        book.rest_order(make_order(1, 100, 10, Side::Buy));
        book.rest_order(make_order(2, 99, 10, Side::Buy));
        book.rest_order(make_order(3, 101, 10, Side::Sell));

        assert_eq!(book.depth(), (2, 1));
    }

    #[test]
    fn test_remove_order_by_id() {
        let mut book = OrderBook::new();

        book.rest_order(make_order(1, 100, 10, Side::Buy));
        book.rest_order(make_order(2, 101, 20, Side::Sell));
        book.rest_order(make_order(3, 99, 30, Side::Buy));

        // Remove buy order by id only
        let removed = book.remove_order_by_id(1);
        assert!(removed.is_some());
        let order = removed.unwrap();
        assert_eq!(order.order_id, 1);
        assert_eq!(order.price, 100);
        assert_eq!(order.qty, 10);

        // Order 1 should be gone, best bid should be 99
        assert_eq!(book.best_bid(), Some(99));

        // Remove sell order by id only
        let removed = book.remove_order_by_id(2);
        assert!(removed.is_some());
        let order = removed.unwrap();
        assert_eq!(order.order_id, 2);
        assert_eq!(order.price, 101);

        // No asks should remain
        assert_eq!(book.best_ask(), None);

        // Try to remove non-existent order
        let removed = book.remove_order_by_id(999);
        assert!(removed.is_none());
    }
}
