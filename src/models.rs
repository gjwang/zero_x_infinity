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

/// An order in the order book
#[derive(Debug, Clone)]
pub struct Order {
    pub id: u64,
    pub user_id: u64,    // User who placed this order
    pub price: u64,      // Price in internal units (e.g., 8 decimals)
    pub qty: u64,        // Original quantity
    pub filled_qty: u64, // How much has been filled
    pub side: Side,
    pub order_type: OrderType,
    pub status: OrderStatus,
}

impl Order {
    /// Create a new limit order
    pub fn new(id: u64, user_id: u64, price: u64, qty: u64, side: Side) -> Self {
        Self {
            id,
            user_id,
            price,
            qty,
            filled_qty: 0,
            side,
            order_type: OrderType::Limit,
            status: OrderStatus::New,
        }
    }

    /// Create a market order (price = 0 means "any price")
    pub fn market(id: u64, user_id: u64, qty: u64, side: Side) -> Self {
        Self {
            id,
            user_id,
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

    /// Calculate order cost (amount to lock)
    ///
    /// Uses checked_mul to detect overflow:
    /// - Buy: price × qty / qty_unit (quote asset)
    /// - Sell: qty (base asset)
    /// - overflow → u64::MAX → order rejected later
    #[inline]
    pub fn calculate_cost(&self, qty_unit: u64) -> u64 {
        match self.side {
            Side::Buy => self
                .price
                .checked_mul(self.qty)
                .map(|v| v / qty_unit)
                .unwrap_or(u64::MAX),
            Side::Sell => self.qty,
        }
    }
}

// ============================================================
// INTERNAL ORDER (self-contained for UBSCore)
// ============================================================

use crate::core_types::{AssetId, OrderId, UserId};

/// Internal order - self-contained, used by UBSCore
///
/// Contains all info needed for balance operations without external config.
/// Created from Order + symbol config when entering UBSCore.
#[derive(Debug, Clone)]
pub struct InternalOrder {
    pub id: OrderId,
    pub user_id: UserId,
    pub price: u64,
    pub qty: u64,
    pub side: Side,
    pub order_type: OrderType,
    /// qty_unit for cost calculation (e.g., 10^8 for BTC)
    pub qty_unit: u64,
    /// Asset IDs for locking
    pub base_asset_id: AssetId,
    pub quote_asset_id: AssetId,
}

impl InternalOrder {
    /// Create from Order + symbol config
    pub fn from_order(
        order: &Order,
        qty_unit: u64,
        base_asset_id: AssetId,
        quote_asset_id: AssetId,
    ) -> Self {
        Self {
            id: order.id,
            user_id: order.user_id as UserId,
            price: order.price,
            qty: order.qty,
            side: order.side,
            order_type: order.order_type,
            qty_unit,
            base_asset_id,
            quote_asset_id,
        }
    }

    /// Calculate order cost (amount to lock) - no parameters needed!
    ///
    /// - Buy: price × qty / qty_unit (lock quote asset)
    /// - Sell: qty (lock base asset)
    #[inline]
    pub fn calculate_cost(&self) -> u64 {
        match self.side {
            Side::Buy => self
                .price
                .checked_mul(self.qty)
                .map(|v| v / self.qty_unit)
                .unwrap_or(u64::MAX),
            Side::Sell => self.qty,
        }
    }

    /// Asset to lock
    #[inline]
    pub fn lock_asset_id(&self) -> AssetId {
        match self.side {
            Side::Buy => self.quote_asset_id,
            Side::Sell => self.base_asset_id,
        }
    }

    /// Convert back to Order for ME
    pub fn to_order(&self) -> Order {
        Order {
            id: self.id,
            user_id: self.user_id as u64,
            price: self.price,
            qty: self.qty,
            filled_qty: 0,
            side: self.side,
            order_type: self.order_type,
            status: OrderStatus::New,
        }
    }
}

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

/// Result of adding an order to the book
#[derive(Debug)]
pub struct OrderResult {
    pub order: Order,
    pub trades: Vec<Trade>,
}
