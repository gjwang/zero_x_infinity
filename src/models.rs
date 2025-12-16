// models.rs - Core order and trade types

/// Order side: Buy or Sell
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Buy,
    Sell,
}

/// Order type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderType {
    Limit,  // Limit order: must specify price
    Market, // Market order: execute at best avail price
}

/// Order status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderStatus {
    New,             // Just created
    PartiallyFilled, // Some quantity filled
    Filled,          // Fully filled
    Cancelled,       // Cancelled by user
}

// ============================================================
// INTERNAL ORDER (the core order type used throughout the system)
// ============================================================

/// Internal order - used by ME, OrderBook, UBSCore, WAL
///
/// "Internal" = trusted, from Gateway (already validated and scaled)
/// price and qty are raw u64 (already scaled by Gateway)
#[derive(Debug, Clone)]
pub struct InternalOrder {
    pub id: u64,
    pub user_id: u64,
    pub symbol_id: u32, // Trading pair
    pub price: u64,     // Raw u64 (already scaled by Gateway)
    pub qty: u64,       // Raw u64 (already scaled by Gateway)
    pub filled_qty: u64,
    pub side: Side,
    pub order_type: OrderType,
    pub status: OrderStatus,
}

impl InternalOrder {
    /// Create a new limit order
    pub fn new(id: u64, user_id: u64, symbol_id: u32, price: u64, qty: u64, side: Side) -> Self {
        Self {
            id,
            user_id,
            symbol_id,
            price,
            qty,
            filled_qty: 0,
            side,
            order_type: OrderType::Limit,
            status: OrderStatus::New,
        }
    }

    /// Create a market order
    pub fn market(id: u64, user_id: u64, symbol_id: u32, qty: u64, side: Side) -> Self {
        Self {
            id,
            user_id,
            symbol_id,
            price: if side == Side::Buy { u64::MAX } else { 0 },
            qty,
            filled_qty: 0,
            side,
            order_type: OrderType::Market,
            status: OrderStatus::New,
        }
    }

    /// Remaining quantity to fill
    #[inline]
    pub fn remaining_qty(&self) -> u64 {
        self.qty - self.filled_qty
    }

    /// Check if order is fully filled
    #[inline]
    pub fn is_filled(&self) -> bool {
        self.filled_qty >= self.qty
    }

    /// Calculate order cost (amount to lock) - no parameters needed!
    ///
    /// Uses checked_mul to detect overflow:
    /// - Buy: price × qty (both already scaled by Gateway)
    /// - Sell: qty
    /// - overflow → u64::MAX → order rejected later
    #[inline]
    pub fn calculate_cost(&self) -> u64 {
        match self.side {
            Side::Buy => self.price.checked_mul(self.qty).unwrap_or(u64::MAX),
            Side::Sell => self.qty,
        }
    }
}

// ============================================================
// TRADE
// ============================================================

/// A trade that occurred when orders matched
#[derive(Debug, Clone)]
pub struct Trade {
    pub id: u64,
    pub buyer_order_id: u64,
    pub seller_order_id: u64,
    pub buyer_user_id: u64,
    pub seller_user_id: u64,
    pub price: u64,
    pub qty: u64,
}

impl Trade {
    pub fn new(
        id: u64,
        buyer_order_id: u64,
        seller_order_id: u64,
        buyer_user_id: u64,
        seller_user_id: u64,
        price: u64,
        qty: u64,
    ) -> Self {
        Self {
            id,
            buyer_order_id,
            seller_order_id,
            buyer_user_id,
            seller_user_id,
            price,
            qty,
        }
    }
}

// ============================================================
// ORDER RESULT
// ============================================================

/// Result of adding an order to the book
#[derive(Debug)]
pub struct OrderResult {
    pub order: InternalOrder,
    pub trades: Vec<Trade>,
}
