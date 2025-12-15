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
    Market, // Market order: execute at best available price
}

/// Order status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderStatus {
    New,             // Just created
    PartiallyFilled, // Some quantity filled
    Filled,          // Fully filled
    Cancelled,       // Cancelled by user
}

/// An order in the order book
#[derive(Debug, Clone)]
pub struct Order {
    pub id: u64,
    pub price: u64,      // Price in internal units (e.g., 8 decimals)
    pub qty: u64,        // Original quantity
    pub filled_qty: u64, // How much has been filled
    pub side: Side,
    pub order_type: OrderType,
    pub status: OrderStatus,
}

impl Order {
    /// Create a new limit order
    pub fn new(id: u64, price: u64, qty: u64, side: Side) -> Self {
        Self {
            id,
            price,
            qty,
            filled_qty: 0,
            side,
            order_type: OrderType::Limit,
            status: OrderStatus::New,
        }
    }

    /// Create a market order (price = 0 means "any price")
    pub fn market(id: u64, qty: u64, side: Side) -> Self {
        Self {
            id,
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
}

/// A trade that occurred when orders matched
#[derive(Debug, Clone)]
pub struct Trade {
    pub id: u64,
    pub buyer_order_id: u64,
    pub seller_order_id: u64,
    pub price: u64,
    pub qty: u64,
}

impl Trade {
    pub fn new(id: u64, buyer_order_id: u64, seller_order_id: u64, price: u64, qty: u64) -> Self {
        Self {
            id,
            buyer_order_id,
            seller_order_id,
            price,
            qty,
        }
    }
}

/// Result of adding an order to the book
#[derive(Debug)]
pub struct OrderResult {
    pub order: Order,
    pub trades: Vec<Trade>,
}
