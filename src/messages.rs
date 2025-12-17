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

    /// InternalOrder expired by system (e.g., GTD timeout)
    Expired {
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
// BALANCE EVENT (Complete Event Sourcing)
// ============================================================

/// Balance event type - categorizes what triggered the balance change
///
/// This enables separated version spaces:
/// - Lock/Unlock operations use `lock_version`
/// - Settle operations use `settle_version`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BalanceEventType {
    /// Deposit - funds added (both versions increment)
    Deposit,
    /// Withdraw - funds removed
    Withdraw,
    /// Lock - funds moved from avail to frozen (order placement)
    Lock,
    /// Unlock - funds moved from frozen to avail (order cancel/partial fill refund)
    Unlock,
    /// Settle - trade settlement (spend_frozen + deposit on counterparty)
    Settle,
}

impl BalanceEventType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Deposit => "deposit",
            Self::Withdraw => "withdraw",
            Self::Lock => "lock",
            Self::Unlock => "unlock",
            Self::Settle => "settle",
        }
    }

    /// Which version space does this event type affect?
    pub fn version_space(&self) -> VersionSpace {
        match self {
            Self::Deposit => VersionSpace::Both,
            Self::Withdraw => VersionSpace::Lock,
            Self::Lock => VersionSpace::Lock,
            Self::Unlock => VersionSpace::Lock,
            Self::Settle => VersionSpace::Settle,
        }
    }
}

/// Version space indicator
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VersionSpace {
    Lock,   // Uses lock_version
    Settle, // Uses settle_version
    Both,   // Uses both (e.g., deposit)
}

/// Source type - what triggered this balance event
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceType {
    /// Triggered by an order (order placement/cancel)
    Order,
    /// Triggered by a trade (settlement)
    Trade,
    /// Triggered by external deposit/withdraw
    External,
}

impl SourceType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Order => "order",
            Self::Trade => "trade",
            Self::External => "external",
        }
    }
}

/// Balance event - complete audit record of a balance change
///
/// This is the core event type for event sourcing. Each balance change
/// generates one BalanceEvent with:
/// - Who: user_id, asset_id
/// - What: event_type, delta
/// - When: version (in the appropriate version space)
/// - Why: source_type + source_id (causal chain)
/// - Result: avail_after, frozen_after
///
/// # Event Sourcing
/// All balance state can be reconstructed by replaying BalanceEvents.
///
/// # Deterministic Verification
/// Events can be grouped by event_type and sorted by source_id for
/// deterministic comparison, even when actual execution order varies
/// in pipelined architectures.
#[derive(Debug, Clone)]
pub struct BalanceEvent {
    /// User whose balance changed
    pub user_id: UserId,
    /// Asset that changed
    pub asset_id: AssetId,
    /// Type of balance operation
    pub event_type: BalanceEventType,
    /// Version in the appropriate space (lock_version or settle_version)
    pub version: u64,
    /// What triggered this event
    pub source_type: SourceType,
    /// ID of the source (order_seq_id, trade_id, or external ref)
    pub source_id: u64,
    /// Change amount (positive = increase, negative = decrease)
    /// Note: stored as i64 for signed representation
    pub delta: i64,
    /// Available balance after this event
    pub avail_after: u64,
    /// Frozen balance after this event
    pub frozen_after: u64,
}

impl BalanceEvent {
    /// Create a new BalanceEvent
    pub fn new(
        user_id: UserId,
        asset_id: AssetId,
        event_type: BalanceEventType,
        version: u64,
        source_type: SourceType,
        source_id: u64,
        delta: i64,
        avail_after: u64,
        frozen_after: u64,
    ) -> Self {
        Self {
            user_id,
            asset_id,
            event_type,
            version,
            source_type,
            source_id,
            delta,
            avail_after,
            frozen_after,
        }
    }

    /// Create a Lock event (order placement)
    pub fn lock(
        user_id: UserId,
        asset_id: AssetId,
        order_seq_id: u64,
        amount: u64,
        lock_version: u64,
        avail_after: u64,
        frozen_after: u64,
    ) -> Self {
        Self::new(
            user_id,
            asset_id,
            BalanceEventType::Lock,
            lock_version,
            SourceType::Order,
            order_seq_id,
            -(amount as i64), // Negative: avail decreases
            avail_after,
            frozen_after,
        )
    }

    /// Create an Unlock event (order cancel/partial refund)
    pub fn unlock(
        user_id: UserId,
        asset_id: AssetId,
        order_seq_id: u64,
        amount: u64,
        lock_version: u64,
        avail_after: u64,
        frozen_after: u64,
    ) -> Self {
        Self::new(
            user_id,
            asset_id,
            BalanceEventType::Unlock,
            lock_version,
            SourceType::Order,
            order_seq_id,
            amount as i64, // Positive: avail increases
            avail_after,
            frozen_after,
        )
    }

    /// Create a Settle event (trade settlement - spend frozen)
    pub fn settle_spend(
        user_id: UserId,
        asset_id: AssetId,
        trade_id: u64,
        amount: u64,
        settle_version: u64,
        avail_after: u64,
        frozen_after: u64,
    ) -> Self {
        Self::new(
            user_id,
            asset_id,
            BalanceEventType::Settle,
            settle_version,
            SourceType::Trade,
            trade_id,
            -(amount as i64), // Negative: frozen decreases
            avail_after,
            frozen_after,
        )
    }

    /// Create a Settle event (trade settlement - receive)
    pub fn settle_receive(
        user_id: UserId,
        asset_id: AssetId,
        trade_id: u64,
        amount: u64,
        settle_version: u64,
        avail_after: u64,
        frozen_after: u64,
    ) -> Self {
        Self::new(
            user_id,
            asset_id,
            BalanceEventType::Settle,
            settle_version,
            SourceType::Trade,
            trade_id,
            amount as i64, // Positive: avail increases
            avail_after,
            frozen_after,
        )
    }

    /// Create a Deposit event
    pub fn deposit(
        user_id: UserId,
        asset_id: AssetId,
        ref_id: u64,
        amount: u64,
        lock_version: u64, // Use lock_version as primary
        avail_after: u64,
        frozen_after: u64,
    ) -> Self {
        Self::new(
            user_id,
            asset_id,
            BalanceEventType::Deposit,
            lock_version,
            SourceType::External,
            ref_id,
            amount as i64,
            avail_after,
            frozen_after,
        )
    }

    /// Format as CSV line
    pub fn to_csv(&self) -> String {
        format!(
            "{},{},{},{},{},{},{},{},{}",
            self.user_id,
            self.asset_id,
            self.event_type.as_str(),
            self.version,
            self.source_type.as_str(),
            self.source_id,
            self.delta,
            self.avail_after,
            self.frozen_after,
        )
    }

    /// CSV header
    pub fn csv_header() -> &'static str {
        "user_id,asset_id,event_type,version,source_type,source_id,delta,avail_after,frozen_after"
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
        assert_eq!(msg.order.order_id, 1);
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

    #[test]
    fn test_balance_event_type() {
        // Test version space assignment
        assert_eq!(BalanceEventType::Lock.version_space(), VersionSpace::Lock);
        assert_eq!(BalanceEventType::Unlock.version_space(), VersionSpace::Lock);
        assert_eq!(
            BalanceEventType::Settle.version_space(),
            VersionSpace::Settle
        );
        assert_eq!(
            BalanceEventType::Deposit.version_space(),
            VersionSpace::Both
        );

        // Test string representation
        assert_eq!(BalanceEventType::Lock.as_str(), "lock");
        assert_eq!(BalanceEventType::Settle.as_str(), "settle");
    }

    #[test]
    fn test_balance_event_lock() {
        let event = BalanceEvent::lock(
            100,  // user_id
            1,    // asset_id (BTC)
            5,    // order_seq_id
            1000, // amount
            3,    // lock_version
            9000, // avail_after
            1000, // frozen_after
        );

        assert_eq!(event.user_id, 100);
        assert_eq!(event.asset_id, 1);
        assert_eq!(event.event_type, BalanceEventType::Lock);
        assert_eq!(event.version, 3);
        assert_eq!(event.source_type, SourceType::Order);
        assert_eq!(event.source_id, 5);
        assert_eq!(event.delta, -1000); // Negative for lock
        assert_eq!(event.avail_after, 9000);
        assert_eq!(event.frozen_after, 1000);
    }

    #[test]
    fn test_balance_event_settle() {
        let event = BalanceEvent::settle_spend(
            100,  // user_id
            2,    // asset_id (USDT)
            42,   // trade_id
            5000, // amount
            7,    // settle_version
            0,    // avail_after
            5000, // frozen_after
        );

        assert_eq!(event.event_type, BalanceEventType::Settle);
        assert_eq!(event.source_type, SourceType::Trade);
        assert_eq!(event.source_id, 42);
        assert_eq!(event.delta, -5000); // Negative for spend
    }

    #[test]
    fn test_balance_event_csv() {
        let event = BalanceEvent::lock(100, 1, 5, 1000, 3, 9000, 1000);
        let csv = event.to_csv();

        assert_eq!(csv, "100,1,lock,3,order,5,-1000,9000,1000");
        assert_eq!(
            BalanceEvent::csv_header(),
            "user_id,asset_id,event_type,version,source_type,source_id,delta,avail_after,frozen_after"
        );
    }

    #[test]
    fn test_balance_event_unlock() {
        let event = BalanceEvent::unlock(
            100,   // user_id
            1,     // asset_id (BTC)
            5,     // order_seq_id (cancelled order)
            1000,  // amount
            4,     // lock_version (after unlock)
            10000, // avail_after (restored)
            0,     // frozen_after (released)
        );

        assert_eq!(event.user_id, 100);
        assert_eq!(event.asset_id, 1);
        assert_eq!(event.event_type, BalanceEventType::Unlock);
        assert_eq!(event.version, 4);
        assert_eq!(event.source_type, SourceType::Order); // Unlock is triggered by order
        assert_eq!(event.source_id, 5);
        assert_eq!(event.delta, 1000); // Positive: avail increases
        assert_eq!(event.avail_after, 10000);
        assert_eq!(event.frozen_after, 0);

        // Test CSV output
        let csv = event.to_csv();
        assert_eq!(csv, "100,1,unlock,4,order,5,1000,10000,0");
    }
}
