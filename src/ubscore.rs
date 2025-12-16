//! UBSCore - User Balance Core Service
//!
//! The single-threaded core service that handles ALL balance operations.
//!
//! # Responsibilities
//!
//! 1. **Balance State Management** - In-memory balance state for all users
//! 2. **Order WAL Writing** - Persist orders BEFORE execution
//! 3. **Balance Operations** - lock/unlock/spend_frozen/deposit
//!
//! # Thread Safety
//!
//! UBSCore is designed for SINGLE-THREADED execution. This provides:
//! - Natural atomicity (no locks needed)
//! - No double-spend risk
//! - Predictable latency
//!
//! # Data Flow
//!
//! ```text
//! Order → process_order() → WAL → Lock Balance → Ok(seq_id) → ME
//!                                       ↓
//!                                 Err(Reject) → OrderEvent::Rejected
//!
//! TradeEvent → settle_trade() → spend_frozen + deposit → BalanceUpdate events
//! ```

use crate::config::TradingConfig;
use crate::core_types::{AssetId, OrderId, SeqNum, UserId};
use crate::messages::{OrderEvent, RejectReason, TradeEvent, ValidOrder};
use crate::models::{Order, Side};
use crate::user_account::UserAccount;
use crate::wal::{WalConfig, WalWriter};

use rustc_hash::FxHashMap;
use std::io;

// ============================================================
// UBSCORE SERVICE
// ============================================================

/// UBSCore - User Balance Core Service
///
/// ALL balance operations go through this service.
pub struct UBSCore {
    /// User accounts - the authoritative balance state
    accounts: FxHashMap<UserId, UserAccount>,
    /// Write-Ahead Log for order persistence
    wal: WalWriter,
    /// Trading configuration (symbols, assets)
    config: TradingConfig,
    /// Pending orders (locked but not yet matched)
    pending_orders: FxHashMap<OrderId, PendingOrder>,
    /// Statistics
    stats: UBSCoreStats,
}

/// Pending order info - tracks locked amount for settlement/cancel
#[derive(Debug, Clone)]
struct PendingOrder {
    #[allow(dead_code)] // Will be used for logging/debugging
    order_id: OrderId,
    user_id: UserId,
    locked_asset_id: AssetId,
    locked_amount: u64,
}

/// UBSCore statistics
#[derive(Debug, Default, Clone)]
pub struct UBSCoreStats {
    pub orders_processed: u64,
    pub orders_accepted: u64,
    pub orders_rejected: u64,
    pub trades_settled: u64,
    pub balance_operations: u64,
}

impl UBSCore {
    /// Create a new UBSCore with given configuration
    pub fn new(config: TradingConfig, wal_config: WalConfig) -> io::Result<Self> {
        let wal = WalWriter::new(wal_config)?;

        Ok(Self {
            accounts: FxHashMap::default(),
            wal,
            config,
            pending_orders: FxHashMap::default(),
            stats: UBSCoreStats::default(),
        })
    }

    /// Create UBSCore with pre-existing accounts (for testing/recovery)
    pub fn with_accounts(
        accounts: FxHashMap<UserId, UserAccount>,
        config: TradingConfig,
        wal_config: WalConfig,
    ) -> io::Result<Self> {
        let wal = WalWriter::new(wal_config)?;

        Ok(Self {
            accounts,
            wal,
            config,
            pending_orders: FxHashMap::default(),
            stats: UBSCoreStats::default(),
        })
    }

    // ============================================================
    // QUERY OPERATIONS (Read-Only)
    // ============================================================

    /// Query user's balance for an asset (read-only)
    #[inline]
    pub fn query_balance(&self, user_id: UserId, asset_id: AssetId) -> Option<(u64, u64)> {
        self.accounts.get(&user_id).and_then(|account| {
            account
                .get_balance(asset_id)
                .map(|b| (b.avail(), b.frozen()))
        })
    }

    /// Query user's available balance for an asset
    #[inline]
    pub fn query_avail(&self, user_id: UserId, asset_id: AssetId) -> u64 {
        self.accounts
            .get(&user_id)
            .and_then(|a| a.get_balance(asset_id))
            .map(|b| b.avail())
            .unwrap_or(0)
    }

    /// Get all accounts (read-only, for serialization)
    pub fn accounts(&self) -> &FxHashMap<UserId, UserAccount> {
        &self.accounts
    }

    /// Get mutable accounts (for deposit during initialization)
    pub fn accounts_mut(&mut self) -> &mut FxHashMap<UserId, UserAccount> {
        &mut self.accounts
    }

    /// Get statistics
    pub fn stats(&self) -> &UBSCoreStats {
        &self.stats
    }

    /// Get current WAL sequence number
    pub fn current_seq(&self) -> SeqNum {
        self.wal.current_seq()
    }

    // ============================================================
    // ORDER PROCESSING
    // ============================================================

    /// Process an incoming order
    ///
    /// # Flow
    /// 1. Write to WAL (persist first!)
    /// 2. Calculate required amount
    /// 3. Lock balance
    /// 4. Return ValidOrder on success, OrderEvent::Rejected on failure
    ///
    /// # Returns
    /// - `Ok(ValidOrder)` - Order accepted, balance locked, ready for ME
    /// - `Err(OrderEvent::Rejected)` - Order rejected (still logged to WAL)
    pub fn process_order(&mut self, order: Order) -> Result<ValidOrder, OrderEvent> {
        self.stats.orders_processed += 1;

        // Step 1: Write to WAL FIRST (persist before any state change)
        let seq_id = match self.wal.append(&order) {
            Ok(seq) => seq,
            Err(_) => {
                // WAL write failure is critical - should never happen in prod
                return Err(OrderEvent::Rejected {
                    seq_id: 0,
                    order_id: order.id,
                    user_id: order.user_id,
                    reason: RejectReason::SystemBusy,
                });
            }
        };

        // Step 2: Validate order
        if order.price == 0 && order.order_type == crate::models::OrderType::Limit {
            self.stats.orders_rejected += 1;
            return Err(OrderEvent::Rejected {
                seq_id,
                order_id: order.id,
                user_id: order.user_id,
                reason: RejectReason::InvalidPrice,
            });
        }

        if order.qty == 0 {
            self.stats.orders_rejected += 1;
            return Err(OrderEvent::Rejected {
                seq_id,
                order_id: order.id,
                user_id: order.user_id,
                reason: RejectReason::InvalidQuantity,
            });
        }

        // Step 3: Get account
        let account = match self.accounts.get_mut(&order.user_id) {
            Some(a) => a,
            None => {
                self.stats.orders_rejected += 1;
                return Err(OrderEvent::Rejected {
                    seq_id,
                    order_id: order.id,
                    user_id: order.user_id,
                    reason: RejectReason::UserNotFound,
                });
            }
        };

        // Step 4: Calculate required amount and determine asset to lock
        let (locked_asset_id, locked_amount) = match order.side {
            Side::Buy => {
                // Buy: lock quote asset (e.g., USDT)
                let quote_asset_id = self.config.quote_asset_id();
                let cost = order.price * order.qty / self.config.qty_unit();
                (quote_asset_id, cost)
            }
            Side::Sell => {
                // Sell: lock base asset (e.g., BTC)
                let base_asset_id = self.config.base_asset_id();
                (base_asset_id, order.qty)
            }
        };

        // Step 5: Try to lock balance
        let lock_result = account
            .get_balance_mut(locked_asset_id)
            .and_then(|balance| balance.lock(locked_amount));

        match lock_result {
            Ok(()) => {
                self.stats.orders_accepted += 1;
                self.stats.balance_operations += 1;

                // Track pending order for settlement/cancel
                self.pending_orders.insert(
                    order.id,
                    PendingOrder {
                        order_id: order.id,
                        user_id: order.user_id,
                        locked_asset_id,
                        locked_amount,
                    },
                );

                Ok(ValidOrder::new(
                    seq_id,
                    order,
                    locked_amount,
                    locked_asset_id,
                ))
            }
            Err(_) => {
                self.stats.orders_rejected += 1;
                Err(OrderEvent::Rejected {
                    seq_id,
                    order_id: order.id,
                    user_id: order.user_id,
                    reason: RejectReason::InsufficientBalance,
                })
            }
        }
    }

    // ============================================================
    // TRADE SETTLEMENT
    // ============================================================

    /// Settle a trade
    ///
    /// # Operations for each party:
    /// - Buyer: spend_frozen(quote), deposit(base)
    /// - Seller: spend_frozen(base), deposit(quote)
    pub fn settle_trade(&mut self, event: &TradeEvent) -> Result<(), &'static str> {
        let trade = &event.trade;
        let quote_amount = trade.price * trade.qty / self.config.qty_unit();

        // Buyer settlement: spend USDT, receive BTC
        {
            let buyer = self
                .accounts
                .get_mut(&trade.buyer_user_id)
                .ok_or("Buyer not found")?;

            buyer
                .get_balance_mut(event.quote_asset_id)?
                .spend_frozen(quote_amount)?;
            buyer
                .get_balance_mut(event.base_asset_id)?
                .deposit(trade.qty)?;

            self.stats.balance_operations += 2;
        }

        // Seller settlement: spend BTC, receive USDT
        {
            let seller = self
                .accounts
                .get_mut(&trade.seller_user_id)
                .ok_or("Seller not found")?;

            seller
                .get_balance_mut(event.base_asset_id)?
                .spend_frozen(trade.qty)?;
            seller
                .get_balance_mut(event.quote_asset_id)?
                .deposit(quote_amount)?;

            self.stats.balance_operations += 2;
        }

        self.stats.trades_settled += 1;
        Ok(())
    }

    // ============================================================
    // ORDER COMPLETION
    // ============================================================

    /// Handle order filled (cleanup pending)
    pub fn order_filled(&mut self, order_id: OrderId) {
        self.pending_orders.remove(&order_id);
    }

    /// Handle order partially filled - update remaining locked amount
    pub fn order_partial_filled(
        &mut self,
        order_id: OrderId,
        filled_qty: u64,
        side: Side,
    ) -> Result<(), &'static str> {
        let pending = self
            .pending_orders
            .get_mut(&order_id)
            .ok_or("Pending order not found")?;

        // Calculate consumed amount
        let consumed = match side {
            Side::Buy => {
                // For buy orders, we need to calculate based on actual trade price
                // This is a simplification - in reality, we'd track exact amounts
                filled_qty * pending.locked_amount / filled_qty.max(1)
            }
            Side::Sell => filled_qty,
        };

        pending.locked_amount = pending.locked_amount.saturating_sub(consumed);
        Ok(())
    }

    /// Cancel an order - unlock remaining frozen balance
    pub fn cancel_order(&mut self, order_id: OrderId) -> Result<u64, &'static str> {
        let pending = self
            .pending_orders
            .remove(&order_id)
            .ok_or("Pending order not found")?;

        if pending.locked_amount > 0 {
            let account = self
                .accounts
                .get_mut(&pending.user_id)
                .ok_or("User not found")?;

            account
                .get_balance_mut(pending.locked_asset_id)?
                .unlock(pending.locked_amount)?;

            self.stats.balance_operations += 1;
        }

        Ok(pending.locked_amount)
    }

    // ============================================================
    // DEPOSIT (External funds coming in)
    // ============================================================

    /// Deposit funds to a user's account
    ///
    /// This is the only way to add new funds to the system.
    pub fn deposit(
        &mut self,
        user_id: UserId,
        asset_id: AssetId,
        amount: u64,
    ) -> Result<(), &'static str> {
        let account = self
            .accounts
            .entry(user_id)
            .or_insert_with(|| UserAccount::new(user_id));

        account.deposit(asset_id, amount)?;
        self.stats.balance_operations += 1;
        Ok(())
    }

    // ============================================================
    // WAL OPERATIONS
    // ============================================================

    /// Flush WAL to disk
    pub fn flush_wal(&mut self) -> io::Result<()> {
        self.wal.flush()
    }

    /// Get WAL statistics
    pub fn wal_stats(&self) -> (u64, u64) {
        (self.wal.total_entries(), self.wal.total_bytes())
    }
}

// ============================================================
// TESTS
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Side;

    fn test_config() -> TradingConfig {
        // Create a minimal test config
        use crate::config::{AssetConfig, SymbolConfig};

        let mut assets = FxHashMap::default();
        assets.insert(
            1,
            AssetConfig {
                asset_id: 1,
                asset: "BTC".to_string(),
                decimals: 8,
                display_decimals: 6,
            },
        );
        assets.insert(
            2,
            AssetConfig {
                asset_id: 2,
                asset: "USDT".to_string(),
                decimals: 6,
                display_decimals: 4,
            },
        );

        let active_symbol = SymbolConfig {
            symbol_id: 0,
            symbol: "BTC_USDT".to_string(),
            base_asset_id: 1,
            quote_asset_id: 2,
            price_decimal: 6,
            price_display_decimal: 2,
        };

        TradingConfig {
            assets,
            symbols: vec![active_symbol.clone()],
            active_symbol,
            base_decimals: 8,
            quote_decimals: 6,
            qty_display_decimals: 6,
            price_display_decimals: 2,
        }
    }

    fn test_wal_config() -> WalConfig {
        WalConfig {
            path: format!("target/test_ubscore_{}.wal", std::process::id()),
            flush_interval_entries: 0,
            sync_on_flush: false,
        }
    }

    #[test]
    fn test_deposit_and_query() {
        let config = test_config();
        let wal_config = test_wal_config();
        let mut ubs = UBSCore::new(config, wal_config).unwrap();

        // Deposit 100 BTC (in satoshi)
        ubs.deposit(1, 1, 100_0000_0000).unwrap();

        // Query balance
        let (avail, frozen) = ubs.query_balance(1, 1).unwrap();
        assert_eq!(avail, 100_0000_0000);
        assert_eq!(frozen, 0);
    }

    #[test]
    fn test_process_order_success() {
        let config = test_config();
        let wal_config = test_wal_config();
        let mut ubs = UBSCore::new(config, wal_config).unwrap();

        // Setup: deposit 100 BTC to user 1
        ubs.deposit(1, 1, 100_0000_0000).unwrap();

        // Create sell order for 10 BTC
        let order = Order::new(1, 1, 10000, 10_0000_0000, Side::Sell);

        // Process order
        let result = ubs.process_order(order);
        assert!(result.is_ok());

        let valid_order = result.unwrap();
        assert_eq!(valid_order.seq_id, 1);
        assert_eq!(valid_order.locked_amount, 10_0000_0000);
        assert_eq!(valid_order.locked_asset_id, 1); // BTC

        // Check balance: 90 avail, 10 frozen
        let (avail, frozen) = ubs.query_balance(1, 1).unwrap();
        assert_eq!(avail, 90_0000_0000);
        assert_eq!(frozen, 10_0000_0000);
    }

    #[test]
    fn test_process_order_insufficient_balance() {
        let config = test_config();
        let wal_config = test_wal_config();
        let mut ubs = UBSCore::new(config, wal_config).unwrap();

        // Setup: deposit only 5 BTC
        ubs.deposit(1, 1, 5_0000_0000).unwrap();

        // Try to sell 10 BTC
        let order = Order::new(1, 1, 10000, 10_0000_0000, Side::Sell);
        let result = ubs.process_order(order);

        assert!(result.is_err());
        if let Err(OrderEvent::Rejected { reason, .. }) = result {
            assert_eq!(reason, RejectReason::InsufficientBalance);
        }
    }

    #[test]
    fn test_cancel_order() {
        let config = test_config();
        let wal_config = test_wal_config();
        let mut ubs = UBSCore::new(config, wal_config).unwrap();

        // Setup and process order
        ubs.deposit(1, 1, 100_0000_0000).unwrap();
        let order = Order::new(1, 1, 10000, 10_0000_0000, Side::Sell);
        ubs.process_order(order).unwrap();

        // Cancel order
        let refunded = ubs.cancel_order(1).unwrap();
        assert_eq!(refunded, 10_0000_0000);

        // Balance should be restored
        let (avail, frozen) = ubs.query_balance(1, 1).unwrap();
        assert_eq!(avail, 100_0000_0000);
        assert_eq!(frozen, 0);
    }
}
