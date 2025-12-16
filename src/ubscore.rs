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
use crate::core_types::{AssetId, SeqNum, UserId};
use crate::messages::{RejectReason, TradeEvent, ValidOrder};
use crate::models::{InternalOrder, Side};
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
/// Trust Balance.frozen to track locked amounts - no separate tracking needed.
pub struct UBSCore {
    /// User accounts - the authoritative balance state
    accounts: FxHashMap<UserId, UserAccount>,
    /// Write-Ahead Log for order persistence
    wal: WalWriter,
    /// Trading configuration (for symbol → asset lookup)
    config: TradingConfig,
}

impl UBSCore {
    /// Create a new UBSCore with given config
    ///
    /// Use `deposit()` to add initial balances after creation.
    pub fn new(config: TradingConfig, wal_config: WalConfig) -> io::Result<Self> {
        let wal = WalWriter::new(wal_config)?;

        Ok(Self {
            accounts: FxHashMap::default(),
            wal,
            config,
        })
    }

    // ============================================================
    // QUERY OPERATIONS (Read-Only)
    // ============================================================

    /// Get user's balance for an asset (read-only)
    #[inline]
    pub fn get_balance(&self, user_id: UserId, asset_id: AssetId) -> Option<&crate::Balance> {
        self.accounts
            .get(&user_id)
            .and_then(|a| a.get_balance(asset_id))
    }

    /// Get all accounts (read-only, for serialization)
    pub fn accounts(&self) -> &FxHashMap<UserId, UserAccount> {
        &self.accounts
    }

    /// Get mutable accounts (for deposit during initialization)
    pub fn accounts_mut(&mut self) -> &mut FxHashMap<UserId, UserAccount> {
        &mut self.accounts
    }

    /// Get current WAL sequence number
    pub fn current_seq(&self) -> SeqNum {
        self.wal.current_seq()
    }

    // ============================================================
    // ORDER PROCESSING
    // ============================================================

    /// Get asset to lock based on order side
    fn lock_asset_for_order(&self, order: &InternalOrder) -> AssetId {
        match order.side {
            Side::Buy => self.config.quote_asset_id(),
            Side::Sell => self.config.base_asset_id(),
        }
    }

    /// Pre-check order (validation only, NO state mutation)
    ///
    /// Performs:
    /// 1. Order validation (price, qty)
    /// 2. Account exists check
    /// 3. Balance sufficiency check (read-only)
    fn pre_check(&self, order: &InternalOrder) -> Result<(), RejectReason> {
        // 1. Validate order fields
        if order.price == 0 && order.order_type == crate::models::OrderType::Limit {
            return Err(RejectReason::InvalidPrice);
        }
        if order.qty == 0 {
            return Err(RejectReason::InvalidQuantity);
        }

        // 2. Check account exists
        let account = self
            .accounts
            .get(&order.user_id)
            .ok_or(RejectReason::UserNotFound)?;

        // 3. Calculate cost and check balance (read-only!)
        let lock_asset = self.lock_asset_for_order(order);
        let lock_amount = order.calculate_cost();

        // Check balance is sufficient (no mutation)
        let balance = account
            .get_balance(lock_asset)
            .ok_or(RejectReason::InsufficientBalance)?;
        if balance.avail() < lock_amount {
            return Err(RejectReason::InsufficientBalance);
        }

        Ok(())
    }

    /// Lock funds (state mutation, call AFTER WAL write)
    fn lock_funds(&mut self, order: &InternalOrder) {
        // This should never fail if pre_check passed
        let lock_asset = self.lock_asset_for_order(order);
        let lock_amount = order.calculate_cost();

        if let Some(account) = self.accounts.get_mut(&order.user_id) {
            if let Ok(balance) = account.get_balance_mut(lock_asset) {
                let _ = balance.lock(lock_amount);
            }
        }
    }

    /// Process an incoming order
    ///
    /// Flow (CRITICAL ordering):
    /// 1. pre_check - validation only, NO state mutation
    /// 2. WAL write - persist order
    /// 3. lock_funds - state mutation (safe, WAL already written)
    ///
    /// TODO: Add deduplication guard (prevent replay attacks)
    /// - Check order_id not seen before
    /// - Use time-windowed bloom filter or LRU cache
    pub fn process_order(&mut self, order: InternalOrder) -> Result<ValidOrder, RejectReason> {
        // TODO: dedup_guard.check_and_record(order.id)?;

        // 1. Pre-check (no side effects, can safely reject)
        self.pre_check(&order)?;

        // 2. Persist to WAL FIRST (before any state mutation!)
        let seq_id = self
            .wal
            .append(&order)
            .map_err(|_| RejectReason::SystemBusy)?;

        // 3. Lock funds (safe now, order is persisted)
        self.lock_funds(&order);

        Ok(ValidOrder::new(seq_id, order))
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
        let quote_amount = event.quote_amount(); // Self-contained!

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
        }

        Ok(())
    }

    // ============================================================
    // BALANCE OPERATIONS (Direct unlock for cancel)
    // ============================================================

    /// Unlock frozen balance (e.g., for order cancellation)
    ///
    /// The caller (ME or OrderBook) knows how much to unlock.
    /// We trust Balance.frozen, no separate tracking needed.
    pub fn unlock(
        &mut self,
        user_id: UserId,
        asset_id: AssetId,
        amount: u64,
    ) -> Result<(), &'static str> {
        let account = self.accounts.get_mut(&user_id).ok_or("User not found")?;
        account.get_balance_mut(asset_id)?.unlock(amount)?;
        Ok(())
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
    use crate::config::{AssetConfig, SymbolConfig};
    use crate::models::InternalOrder;

    fn test_config() -> TradingConfig {
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
        let b = ubs.get_balance(1, 1).unwrap();
        assert_eq!(b.avail(), 100_0000_0000);
        assert_eq!(b.frozen(), 0);
    }

    #[test]
    fn test_process_order_success() {
        let config = test_config();
        let wal_config = test_wal_config();
        let mut ubs = UBSCore::new(config, wal_config).unwrap();

        // Setup: deposit 100 BTC to user 1
        ubs.deposit(1, 1, 100_0000_0000).unwrap();

        // Create sell order for 10 BTC (symbol_id=0)
        let order = InternalOrder::new(1, 1, 0, 10000, 10_0000_0000, Side::Sell);

        // Process order
        let result = ubs.process_order(order);
        assert!(result.is_ok());

        let valid_order = result.unwrap();
        assert_eq!(valid_order.seq_id, 1);
        assert_eq!(valid_order.order.qty, 10_0000_0000);

        // Check balance: 90 avail, 10 frozen
        let b = ubs.get_balance(1, 1).unwrap();
        assert_eq!(b.avail(), 90_0000_0000);
        assert_eq!(b.frozen(), 10_0000_0000);
    }

    #[test]
    fn test_process_order_insufficient_balance() {
        let config = test_config();
        let wal_config = test_wal_config();
        let mut ubs = UBSCore::new(config, wal_config).unwrap();

        // Setup: deposit only 5 BTC
        ubs.deposit(1, 1, 5_0000_0000).unwrap();

        // Try to sell 10 BTC
        let order = InternalOrder::new(1, 1, 0, 10000, 10_0000_0000, Side::Sell);
        let result = ubs.process_order(order);

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), RejectReason::InsufficientBalance);
    }

    #[test]
    fn test_unlock() {
        let config = test_config();
        let wal_config = test_wal_config();
        let mut ubs = UBSCore::new(config, wal_config).unwrap();

        // Setup and process order
        ubs.deposit(1, 1, 100_0000_0000).unwrap();
        let order = InternalOrder::new(1, 1, 0, 10000, 10_0000_0000, Side::Sell);
        ubs.process_order(order).unwrap();

        // Check: 90 avail, 10 frozen
        let b = ubs.get_balance(1, 1).unwrap();
        assert_eq!(b.avail(), 90_0000_0000);
        assert_eq!(b.frozen(), 10_0000_0000);

        // Unlock (e.g., order cancelled by ME)
        ubs.unlock(1, 1, 10_0000_0000).unwrap();

        // Balance should be restored
        let b = ubs.get_balance(1, 1).unwrap();
        assert_eq!(b.avail(), 100_0000_0000);
        assert_eq!(b.frozen(), 0);
    }
}
