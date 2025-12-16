//! Messages - Inter-service communication types
//!
//! These types are used to pass data between services via crossbeam queues.
//!
//! # Message Flow
//!
//! ```text
//! Gateway → OrderMessage → UBSCore → ValidOrder → ME → TradeEvent → Settlement
//!                                                  ↓
//!                                            BalanceUpdate → UBSCore
//! ```

use crate::core_types::{AssetId, OrderId, SeqNum, TradeId, UserId};
use crate::models::{InternalOrder, Side, Trade};

// ============================================================
// ORDER MESSAGES (Gateway → UBSCore)
// ============================================================

/// InternalOrder message - submitted by Gateway, processed by UBSCore
///
/// This is the raw order before WAL persistence and balance locking.
#[derive(Debug, Clone)]
pub struct OrderMessage {
    /// Sequence number assigned by UBSCore after WAL write
    pub seq_id: SeqNum,
    /// The order content
    pub order: InternalOrder,
    /// Timestamp in nanoseconds (from std::time::Instant or TSC)
    pub timestamp_ns: u64,
}

impl OrderMessage {
    pub fn new(seq_id: SeqNum, order: InternalOrder, timestamp_ns: u64) -> Self {
        Self {
            seq_id,
            order,
            timestamp_ns,
        }
    }
}

// ============================================================
// VALID ORDER (UBSCore → ME)
// ============================================================

/// Valid order - balance locked, ready for matching
///
/// Only orders that pass balance lock reach this stage.
#[derive(Debug, Clone)]
pub struct ValidOrder {
    /// Sequence number (from WAL)
    pub seq_id: SeqNum,
    /// The order (balance already locked)
    pub order: InternalOrder,
}

impl ValidOrder {
    pub fn new(seq_id: SeqNum, order: InternalOrder) -> Self {
        Self { seq_id, order }
    }
}

// ============================================================
// TRADE EVENT (ME → UBSCore + Settlement)
// ============================================================

/// Trade event - output from Matching Engine
///
/// Contains all information needed to:
/// 1. Update balances (UBSCore)
/// 2. Persist to storage (Settlement)
#[derive(Debug, Clone)]
pub struct TradeEvent {
    /// The executed trade
    pub trade: Trade,
    /// Taker's order ID
    pub taker_order_id: OrderId,
    /// Maker's order ID  
    pub maker_order_id: OrderId,
    /// Taker side (Buy or Sell)
    pub taker_side: Side,
    /// Base asset ID (e.g., BTC in BTC_USDT)
    pub base_asset_id: AssetId,
    /// Quote asset ID (e.g., USDT in BTC_USDT)
    pub quote_asset_id: AssetId,
    /// qty_unit for quote amount calculation
    pub qty_unit: u64,
}

impl TradeEvent {
    pub fn new(
        trade: Trade,
        taker_order_id: OrderId,
        maker_order_id: OrderId,
        taker_side: Side,
        base_asset_id: AssetId,
        quote_asset_id: AssetId,
        qty_unit: u64,
    ) -> Self {
        Self {
            trade,
            taker_order_id,
            maker_order_id,
            taker_side,
            base_asset_id,
            quote_asset_id,
            qty_unit,
        }
    }

    /// Calculate quote amount (price * qty / qty_unit) - self-contained
    #[inline]
    pub fn quote_amount(&self) -> u64 {
        self.trade.price * self.trade.qty / self.qty_unit
    }
}

// ============================================================
// ORDER EVENT (状态变更事件)
// ============================================================

/// InternalOrder event - order state changes
///
/// Used for:
/// 1. Audit logging
/// 2. Client notifications
/// 3. Settlement persistence
#[derive(Debug, Clone)]
pub enum OrderEvent {
    /// InternalOrder accepted and balance locked
    Accepted {
        seq_id: SeqNum,
        order_id: OrderId,
        user_id: UserId,
    },

    /// InternalOrder rejected (insufficient balance, etc.)
    Rejected {
        seq_id: SeqNum,
        order_id: OrderId,
        user_id: UserId,
        reason: RejectReason,
    },

    /// InternalOrder fully filled
    Filled {
        order_id: OrderId,
        user_id: UserId,
        filled_qty: u64,
        avg_price: u64,
    },

    /// InternalOrder partially filled
    PartialFilled {
        order_id: OrderId,
        user_id: UserId,
        filled_qty: u64,
        remaining_qty: u64,
    },

    /// InternalOrder cancelled by user
    Cancelled {
        order_id: OrderId,
        user_id: UserId,
        unfilled_qty: u64,
    },
}

// ============================================================
// REJECT REASON
// ============================================================

/// Reason for order rejection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RejectReason {
    /// Insufficient available balance
    InsufficientBalance,
    /// Invalid price (e.g., zero for limit order)
    InvalidPrice,
    /// Invalid quantity (zero)
    InvalidQuantity,
    /// User account not found
    UserNotFound,
    /// Asset not found
    AssetNotFound,
    /// Symbol not found
    SymbolNotFound,
    /// InternalOrder already exists
    DuplicateOrderId,
    /// System overloaded
    SystemBusy,
}

impl RejectReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::InsufficientBalance => "Insufficient balance",
            Self::InvalidPrice => "Invalid price",
            Self::InvalidQuantity => "Invalid quantity",
            Self::UserNotFound => "User not found",
            Self::AssetNotFound => "Asset not found",
            Self::SymbolNotFound => "Symbol not found",
            Self::DuplicateOrderId => "Duplicate order ID",
            Self::SystemBusy => "System busy",
        }
    }
}

// ============================================================
// BALANCE UPDATE (from TradeEvent)
// ============================================================

/// Balance update operation
///
/// Generated from TradeEvent for UBSCore to execute
#[derive(Debug, Clone)]
pub struct BalanceUpdate {
    pub trade_id: TradeId,
    pub user_id: UserId,
    pub asset_id: AssetId,
    pub operation: BalanceOp,
    pub amount: u64,
}

/// Balance operation type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BalanceOp {
    /// Spend from frozen (trade settlement)
    SpendFrozen,
    /// Add to available (receive from trade)
    Deposit,
    /// Move frozen to available (refund on cancel/partial fill)
    Unlock,
}

impl BalanceUpdate {
    pub fn spend_frozen(
        trade_id: TradeId,
        user_id: UserId,
        asset_id: AssetId,
        amount: u64,
    ) -> Self {
        Self {
            trade_id,
            user_id,
            asset_id,
            operation: BalanceOp::SpendFrozen,
            amount,
        }
    }

    pub fn deposit(trade_id: TradeId, user_id: UserId, asset_id: AssetId, amount: u64) -> Self {
        Self {
            trade_id,
            user_id,
            asset_id,
            operation: BalanceOp::Deposit,
            amount,
        }
    }

    pub fn unlock(trade_id: TradeId, user_id: UserId, asset_id: AssetId, amount: u64) -> Self {
        Self {
            trade_id,
            user_id,
            asset_id,
            operation: BalanceOp::Unlock,
            amount,
        }
    }
}

// ============================================================
// TESTS
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_order_message() {
        let order = InternalOrder::new(1, 100, 0, 10000, 1000, Side::Buy);
        let msg = OrderMessage::new(1, order, 1234567890);

        assert_eq!(msg.seq_id, 1);
        assert_eq!(msg.order.id, 1);
        assert_eq!(msg.timestamp_ns, 1234567890);
    }

    #[test]
    fn test_reject_reason_str() {
        assert_eq!(
            RejectReason::InsufficientBalance.as_str(),
            "Insufficient balance"
        );
        assert_eq!(RejectReason::SystemBusy.as_str(), "System busy");
    }

    #[test]
    fn test_balance_update() {
        let update = BalanceUpdate::spend_frozen(1, 100, 1, 5000);
        assert_eq!(update.operation, BalanceOp::SpendFrozen);
        assert_eq!(update.amount, 5000);
    }
}
