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

use crate::core_types::{AssetId, OrderId, SeqNum, UserId};
use crate::messages::{BalanceEvent, RejectReason, TradeEvent, ValidOrder};
use crate::models::{InternalOrder, Side};
use crate::pipeline::{BalanceUpdateRequest, OrderAction, ValidAction};
use crate::symbol_manager::SymbolManager;
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
    /// Symbol manager for symbol → asset lookup
    manager: SymbolManager,
}

impl UBSCore {
    /// Create a new UBSCore with given symbol manager
    pub fn new(manager: SymbolManager, wal_config: WalConfig) -> io::Result<Self> {
        let wal = WalWriter::new(wal_config)?;

        Ok(Self {
            accounts: FxHashMap::default(),
            wal,
            manager,
        })
    }

    /// Commit: Flush WAL to disk for durability.
    /// Caller should only release results collected from methods after this succeeds.
    pub fn commit(&mut self) -> io::Result<()> {
        self.wal.flush()
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

    /// Get asset to lock based on order side and symbol
    fn lock_asset_for_order(&self, order: &InternalOrder) -> AssetId {
        let symbol_info = self.manager.get_symbol_info_by_id(order.symbol_id);
        match (order.side, symbol_info) {
            (Side::Buy, Some(s)) => s.quote_asset_id,
            (Side::Sell, Some(s)) => s.base_asset_id,
            _ => 0, // Invalid symbol_id - will fail later
        }
    }

    /// Pre-check order (validation only, NO state mutation)
    ///
    /// Performs:
    /// 1. Order validation (price, qty)
    /// 2. Account exists check
    /// 3. Balance sufficiency check (read-only)
    fn pre_check(&self, order: &InternalOrder) -> Result<(), RejectReason> {
        // 0. Validate symbol exists
        let symbol_info = self
            .manager
            .get_symbol_info_by_id(order.symbol_id)
            .ok_or(RejectReason::SymbolNotFound)?;

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
        let lock_amount = order
            .calculate_cost(symbol_info.qty_unit())
            .map_err(|_| RejectReason::InvalidQuantity)?; // CostError::Overflow → InvalidQuantity

        // Check balance is sufficient (no mutation)
        let balance = account
            .get_balance(lock_asset)
            .ok_or(RejectReason::AssetNotFound)?;

        if balance.avail() < lock_amount {
            return Err(RejectReason::InsufficientBalance);
        }

        Ok(())
    }

    /// Lock funds (state mutation, call AFTER WAL write)
    fn lock_funds(&mut self, order: &InternalOrder) {
        // This should never fail if pre_check passed
        let lock_asset = self.lock_asset_for_order(order);
        let symbol_info = self.manager.get_symbol_info_by_id(order.symbol_id);
        let qty_unit = symbol_info.map(|s| s.qty_unit()).unwrap_or(100_000_000);
        // Safe to unwrap: pre_check already validated this won't overflow
        let lock_amount = order
            .calculate_cost(qty_unit)
            .expect("pre_check should have caught overflow");

        if let Some(account) = self.accounts.get_mut(&order.user_id)
            && let Ok(balance) = account.get_balance_mut(lock_asset)
        {
            let _ = balance.lock(lock_amount);
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
    pub fn process_order(
        &mut self,
        order: InternalOrder,
    ) -> Result<(ValidOrder, BalanceEvent), RejectReason> {
        // 1. Pre-check
        self.pre_check(&order)?;

        let lock_asset = self.lock_asset_for_order(&order);
        let qty_unit = self
            .manager
            .get_symbol_info_by_id(order.symbol_id)
            .map(|s| s.qty_unit())
            .unwrap_or(1);
        let lock_amount = order.calculate_cost(qty_unit).unwrap_or(0);

        // 2. Persist to WAL FIRST
        let seq_id = self
            .wal
            .append(&order)
            .map_err(|_| RejectReason::SystemBusy)?;

        let mut final_order = order;
        final_order.seq_id = seq_id;

        // 3. Lock funds
        self.lock_funds(&final_order);

        // 4. Generate Event (authoritative balance state)
        let balance = self
            .get_balance(final_order.user_id, lock_asset)
            .expect("Balance must exist after lock_funds");

        let event = BalanceEvent::lock(
            final_order.user_id,
            lock_asset,
            final_order.order_id,
            lock_amount,
            balance.lock_version(),
            balance.avail(),
            balance.frozen(),
            final_order.ingested_at_ns,
        );

        Ok((
            ValidOrder::new(seq_id, final_order, event.ingested_at_ns),
            event,
        ))
    }

    // ============================================================
    // TRADE SETTLEMENT
    // ============================================================

    /// Settle a trade
    ///
    /// # Operations for each party:
    /// - Buyer: spend_frozen(quote), deposit(base)
    /// - Seller: spend_frozen(base), deposit(quote)
    ///
    /// Apply an order action (Place or Cancel).
    #[allow(clippy::too_many_arguments)]
    pub fn apply_order_action(
        &mut self,
        action: OrderAction,
    ) -> Result<(Vec<ValidAction>, Vec<BalanceEvent>), RejectReason> {
        match action {
            OrderAction::Place(seq_order) => {
                let (valid_order, event) = self.process_order(seq_order.order)?;
                Ok((vec![ValidAction::Order(valid_order)], vec![event]))
            }
            OrderAction::Cancel {
                order_id,
                user_id,
                ingested_at_ns,
            } => Ok((
                vec![ValidAction::Cancel {
                    order_id,
                    user_id,
                    ingested_at_ns,
                }],
                vec![],
            )),
        }
    }

    /// Apply a high-level balance update request (Trade or Cancel).
    /// Internalizes logic like price improvement refunds.
    pub fn apply_balance_update(
        &mut self,
        request: BalanceUpdateRequest,
    ) -> Result<Vec<BalanceEvent>, &'static str> {
        match request {
            BalanceUpdateRequest::Trade {
                trade_event,
                price_improvement,
            } => {
                let mut events = self.settle_trade(&trade_event)?;
                if let Some(pi) = price_improvement
                    && let Ok(refund_event) = self.unlock(
                        pi.user_id,
                        pi.asset_id,
                        trade_event.trade.trade_id,
                        pi.amount,
                        trade_event.taker_ingested_at_ns,
                    )
                {
                    events.push(refund_event);
                }
                Ok(events)
            }
            BalanceUpdateRequest::Cancel {
                order_id,
                user_id,
                asset_id,
                unlock_amount,
                ingested_at_ns,
            } => {
                let event =
                    self.unlock(user_id, asset_id, order_id, unlock_amount, ingested_at_ns)?;
                Ok(vec![event])
            }
        }
    }

    pub fn settle_trade(&mut self, event: &TradeEvent) -> Result<Vec<BalanceEvent>, &'static str> {
        let trade = &event.trade;
        let quote_amount = event.quote_amount();
        let mut results = Vec::with_capacity(4);

        // Get fee rates from symbol (fallback to 0 if not found)
        let (maker_fee_rate, taker_fee_rate) = self
            .manager
            .get_symbol_info_by_id(event.symbol_id)
            .map(|s| (s.base_maker_fee, s.base_taker_fee))
            .unwrap_or((0, 0));

        // Determine who is maker/taker
        // In TradeEvent: taker_order_id tells us which order is the taker
        // Buyer is taker if taker_order_id == buyer_order_id
        let buyer_is_taker = event.taker_order_id == trade.buyer_order_id;
        let buyer_fee_rate = if buyer_is_taker {
            taker_fee_rate
        } else {
            maker_fee_rate
        };
        let seller_fee_rate = if buyer_is_taker {
            maker_fee_rate
        } else {
            taker_fee_rate
        };

        // Calculate fees (fee is deducted from RECEIVED asset)
        // Buyer receives base → fee in base
        // Seller receives quote → fee in quote
        let buyer_fee = crate::fee::calculate_fee(trade.qty, buyer_fee_rate);
        let seller_fee = crate::fee::calculate_fee(quote_amount, seller_fee_rate);

        // Net amounts after fee
        let buyer_net_base = trade.qty.saturating_sub(buyer_fee);
        let seller_net_quote = quote_amount.saturating_sub(seller_fee);

        // 1. Buyer settlement
        {
            let buyer = self
                .accounts
                .get_mut(&trade.buyer_user_id)
                .ok_or("Buyer not found")?;

            buyer
                .get_balance_mut(event.quote_asset_id)?
                .spend_frozen(quote_amount)?;
            let b_quote = buyer.get_balance(event.quote_asset_id).unwrap();
            results.push(BalanceEvent::settle_spend(
                trade.buyer_user_id,
                event.quote_asset_id,
                trade.trade_id,
                quote_amount,
                b_quote.settle_version(),
                b_quote.avail(),
                b_quote.frozen(),
                event.taker_ingested_at_ns,
            ));

            // Deposit NET amount (after fee deduction)
            buyer
                .get_balance_mut(event.base_asset_id)?
                .deposit(buyer_net_base)?;
            let b_base = buyer.get_balance(event.base_asset_id).unwrap();
            results.push(BalanceEvent::settle_receive(
                trade.buyer_user_id,
                event.base_asset_id,
                trade.trade_id,
                buyer_net_base, // Net amount after fee
                b_base.settle_version(),
                b_base.avail(),
                b_base.frozen(),
                event.taker_ingested_at_ns,
            ));
        }

        // 2. Seller settlement
        {
            let seller = self
                .accounts
                .get_mut(&trade.seller_user_id)
                .ok_or("Seller not found")?;

            seller
                .get_balance_mut(event.base_asset_id)?
                .spend_frozen(trade.qty)?;
            let s_base = seller.get_balance(event.base_asset_id).unwrap();
            results.push(BalanceEvent::settle_spend(
                trade.seller_user_id,
                event.base_asset_id,
                trade.trade_id,
                trade.qty,
                s_base.settle_version(),
                s_base.avail(),
                s_base.frozen(),
                event.taker_ingested_at_ns,
            ));

            // Deposit NET amount (after fee deduction)
            seller
                .get_balance_mut(event.quote_asset_id)?
                .deposit(seller_net_quote)?;
            let s_quote = seller.get_balance(event.quote_asset_id).unwrap();
            results.push(BalanceEvent::settle_receive(
                trade.seller_user_id,
                event.quote_asset_id,
                trade.trade_id,
                seller_net_quote, // Net amount after fee
                s_quote.settle_version(),
                s_quote.avail(),
                s_quote.frozen(),
                event.taker_ingested_at_ns,
            ));
        }

        // 3. REVENUE account fee income (for asset conservation)
        // Buyer fee in base asset, seller fee in quote asset
        if buyer_fee > 0 || seller_fee > 0 {
            use crate::core_types::REVENUE_ID;

            // Ensure REVENUE account exists
            let revenue = self
                .accounts
                .entry(REVENUE_ID)
                .or_insert_with(|| crate::user_account::UserAccount::new(REVENUE_ID));

            // Buyer fee → REVENUE (in base asset)
            if buyer_fee > 0 {
                revenue.deposit(event.base_asset_id, buyer_fee)?;
                let r_base = revenue.get_balance(event.base_asset_id).unwrap();
                results.push(BalanceEvent::fee_received(
                    REVENUE_ID,
                    event.base_asset_id,
                    trade.trade_id,
                    buyer_fee,
                    trade.buyer_user_id, // from_user
                    r_base.avail(),
                    event.taker_ingested_at_ns,
                ));
            }

            // Seller fee → REVENUE (in quote asset)
            if seller_fee > 0 {
                revenue.deposit(event.quote_asset_id, seller_fee)?;
                let r_quote = revenue.get_balance(event.quote_asset_id).unwrap();
                results.push(BalanceEvent::fee_received(
                    REVENUE_ID,
                    event.quote_asset_id,
                    trade.trade_id,
                    seller_fee,
                    trade.seller_user_id, // from_user
                    r_quote.avail(),
                    event.taker_ingested_at_ns,
                ));
            }
        }

        Ok(results)
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
        order_id: OrderId,
        amount: u64,
        ingested_at_ns: u64,
    ) -> Result<BalanceEvent, &'static str> {
        let account = self.accounts.get_mut(&user_id).ok_or("User not found")?;
        account.get_balance_mut(asset_id)?.unlock(amount)?;

        let balance = account.get_balance(asset_id).unwrap();
        Ok(BalanceEvent::unlock(
            user_id,
            asset_id,
            order_id,
            amount,
            balance.lock_version(),
            balance.avail(),
            balance.frozen(),
            ingested_at_ns,
        ))
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
    // INTERNAL TRANSFER (Phase 0x0B-a)
    // ============================================================

    /// Withdraw funds for internal transfer (Spot -> Funding)
    ///
    /// Directly deducts from available balance (no freeze).
    /// This is for internal transfers only, NOT for trading orders.
    ///
    /// # Returns
    /// - `Ok(avail, frozen)` - New balance after withdrawal
    /// - `Err("Account not found")` - User or asset doesn't exist
    /// - `Err("Insufficient balance")` - Not enough available balance
    pub fn withdraw_for_transfer(
        &mut self,
        user_id: UserId,
        asset_id: AssetId,
        amount: u64,
    ) -> Result<(u64, u64), &'static str> {
        let account = self.accounts.get_mut(&user_id).ok_or("Account not found")?;
        let balance = account.get_balance_mut(asset_id)?;

        // Check available balance
        if balance.avail() < amount {
            return Err("Insufficient balance");
        }

        // Directly withdraw from available (no freeze involved)
        balance.withdraw(amount)?;

        Ok((balance.avail(), balance.frozen()))
    }

    /// Deposit funds from internal transfer (Funding -> Spot)
    ///
    /// Credits the available balance directly.
    /// Creates account/asset if not exists (lazy init).
    ///
    /// # Returns
    /// - `Ok(avail, frozen)` - New balance after deposit
    pub fn deposit_from_transfer(
        &mut self,
        user_id: UserId,
        asset_id: AssetId,
        amount: u64,
    ) -> Result<(u64, u64), &'static str> {
        // Lazy init: create account and asset if not exist
        let account = self
            .accounts
            .entry(user_id)
            .or_insert_with(|| UserAccount::new(user_id));

        account.deposit(asset_id, amount)?;

        let balance = account.get_balance(asset_id).unwrap();
        Ok((balance.avail(), balance.frozen()))
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
    use crate::models::InternalOrder;

    fn test_manager() -> SymbolManager {
        let mut manager = SymbolManager::new();

        // Add assets first
        manager.add_asset(1, 8, 6, "BTC"); // BTC: 8 decimals
        manager.add_asset(2, 6, 4, "USDT"); // USDT: 6 decimals

        // Add symbol (BTC_USDT)
        manager
            .insert_symbol("BTC_USDT", 0, 1, 2, 6, 2)
            .expect("Failed to insert symbol");

        manager
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
        let manager = test_manager();
        let wal_config = test_wal_config();
        let mut ubs = UBSCore::new(manager, wal_config).unwrap();

        // Deposit 100 BTC (in satoshi)
        ubs.deposit(1, 1, 100_0000_0000).unwrap();

        // Query balance
        let b = ubs.get_balance(1, 1).unwrap();
        assert_eq!(b.avail(), 100_0000_0000);
        assert_eq!(b.frozen(), 0);
    }

    #[test]
    fn test_process_order_success() {
        let manager = test_manager();
        let wal_config = test_wal_config();
        let mut ubs = UBSCore::new(manager, wal_config).unwrap();

        // Setup: deposit 100 BTC to user 1
        ubs.deposit(1, 1, 100_0000_0000).unwrap();

        // Create sell order for 10 BTC (symbol_id=0)
        let order = InternalOrder::new(1, 1, 0, 10000, 10_0000_0000, Side::Sell);

        // Process order
        let result = ubs.process_order(order);
        assert!(result.is_ok());

        let (valid_order, _event) = result.unwrap();
        assert_eq!(valid_order.seq_id, 1);
        assert_eq!(valid_order.order.qty, 10_0000_0000);

        // Check balance: 90 avail, 10 frozen
        let b = ubs.get_balance(1, 1).unwrap();
        assert_eq!(b.avail(), 90_0000_0000);
        assert_eq!(b.frozen(), 10_0000_0000);
    }

    #[test]
    fn test_process_order_insufficient_balance() {
        let manager = test_manager();
        let wal_config = test_wal_config();
        let mut ubs = UBSCore::new(manager, wal_config).unwrap();

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
        let manager = test_manager();
        let wal_config = test_wal_config();
        let mut ubs = UBSCore::new(manager, wal_config).unwrap();

        // Setup and process order
        ubs.deposit(1, 1, 100_0000_0000).unwrap();
        let order = InternalOrder::new(1, 1, 0, 10000, 10_0000_0000, Side::Sell);
        let (_valid_order, _event) = ubs.process_order(order).unwrap();

        // Check: 90 avail, 10 frozen
        let b = ubs.get_balance(1, 1).unwrap();
        assert_eq!(b.avail(), 90_0000_0000);
        assert_eq!(b.frozen(), 10_0000_0000);

        // Unlock (e.g., order cancelled by ME)
        let _ = ubs.unlock(1, 1, 100, 10_0000_0000, 0).unwrap();

        // Balance should be restored
        let b = ubs.get_balance(1, 1).unwrap();
        assert_eq!(b.avail(), 100_0000_0000);
        assert_eq!(b.frozen(), 0);
    }

    #[test]
    fn test_cancel_order_flow_with_version() {
        let manager = test_manager();
        let wal_config = test_wal_config();
        let mut ubs = UBSCore::new(manager, wal_config).unwrap();

        // Setup: deposit 100 BTC
        ubs.deposit(1, 1, 100_0000_0000).unwrap();

        // Get initial version
        let b = ubs.get_balance(1, 1).unwrap();
        let initial_lock_version = b.lock_version();
        assert_eq!(initial_lock_version, 1); // deposit increments lock_version

        // Process order (Lock)
        let order = InternalOrder::new(1, 1, 0, 10000, 20_0000_0000, Side::Sell);
        let (valid_order, _event) = ubs.process_order(order).unwrap();
        assert_eq!(valid_order.order.qty, 20_0000_0000);

        // Check balance after lock
        let b = ubs.get_balance(1, 1).unwrap();
        assert_eq!(b.avail(), 80_0000_0000);
        assert_eq!(b.frozen(), 20_0000_0000);
        let lock_version_after_lock = b.lock_version();
        assert_eq!(lock_version_after_lock, 2); // lock increments lock_version

        // Simulate cancel: unlock frozen amount
        let _ = ubs.unlock(1, 1, 1, 20_0000_0000, 0).unwrap();

        // Check balance after unlock
        let b = ubs.get_balance(1, 1).unwrap();
        assert_eq!(b.avail(), 100_0000_0000);
        assert_eq!(b.frozen(), 0);
        let lock_version_after_unlock = b.lock_version();
        assert_eq!(lock_version_after_unlock, 3); // unlock increments lock_version
    }

    #[test]
    fn test_partial_fill_then_cancel() {
        let manager = test_manager();
        let wal_config = test_wal_config();
        let mut ubs = UBSCore::new(manager, wal_config).unwrap();

        // Setup: deposit 100 BTC
        ubs.deposit(1, 1, 100_0000_0000).unwrap();

        // Process sell order for 50 BTC
        let order = InternalOrder::new(1, 1, 0, 10000, 50_0000_0000, Side::Sell);
        ubs.process_order(order).unwrap();

        // Check: 50 avail, 50 frozen
        let b = ubs.get_balance(1, 1).unwrap();
        assert_eq!(b.avail(), 50_0000_0000);
        assert_eq!(b.frozen(), 50_0000_0000);

        // Simulate partial fill: spend 20 BTC frozen
        ubs.accounts_mut()
            .get_mut(&1)
            .unwrap()
            .get_balance_mut(1)
            .unwrap()
            .spend_frozen(20_0000_0000)
            .unwrap();

        // Check: 50 avail, 30 frozen
        let b = ubs.get_balance(1, 1).unwrap();
        assert_eq!(b.avail(), 50_0000_0000);
        assert_eq!(b.frozen(), 30_0000_0000);

        // Cancel remaining: unlock 30 BTC
        ubs.unlock(1, 1, 1, 30_0000_0000, 0).unwrap();

        // Final: 80 avail, 0 frozen (100 - 20 spent = 80)
        let b = ubs.get_balance(1, 1).unwrap();
        assert_eq!(b.avail(), 80_0000_0000);
        assert_eq!(b.frozen(), 0);
    }

    // ============================================================
    // INTERNAL TRANSFER TESTS (Phase 0x0B-a)
    // ============================================================

    #[test]
    fn test_withdraw_for_transfer() {
        let manager = test_manager();
        let wal_config = test_wal_config();
        let mut ubs = UBSCore::new(manager, wal_config).unwrap();

        // Setup: deposit 100 BTC to user 1
        ubs.deposit(1, 1, 100_0000_0000).unwrap();

        // Withdraw 30 BTC for transfer
        let (avail, frozen) = ubs.withdraw_for_transfer(1, 1, 30_0000_0000).unwrap();

        // Check: 70 avail, 0 frozen (no freeze for transfers)
        assert_eq!(avail, 70_0000_0000);
        assert_eq!(frozen, 0);

        let b = ubs.get_balance(1, 1).unwrap();
        assert_eq!(b.avail(), 70_0000_0000);
    }

    #[test]
    fn test_withdraw_for_transfer_insufficient() {
        let manager = test_manager();
        let wal_config = test_wal_config();
        let mut ubs = UBSCore::new(manager, wal_config).unwrap();

        // Setup: deposit only 50 BTC
        ubs.deposit(1, 1, 50_0000_0000).unwrap();

        // Try to withdraw 100 BTC
        let result = ubs.withdraw_for_transfer(1, 1, 100_0000_0000);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Insufficient balance");
    }

    #[test]
    fn test_deposit_from_transfer_existing_account() {
        let manager = test_manager();
        let wal_config = test_wal_config();
        let mut ubs = UBSCore::new(manager, wal_config).unwrap();

        // Setup: existing account with 50 BTC
        ubs.deposit(1, 1, 50_0000_0000).unwrap();

        // Deposit 30 BTC from transfer
        let (avail, frozen) = ubs.deposit_from_transfer(1, 1, 30_0000_0000).unwrap();

        // Check: 80 avail
        assert_eq!(avail, 80_0000_0000);
        assert_eq!(frozen, 0);
    }

    #[test]
    fn test_deposit_from_transfer_new_account() {
        let manager = test_manager();
        let wal_config = test_wal_config();
        let mut ubs = UBSCore::new(manager, wal_config).unwrap();

        // Deposit 100 BTC to new user (lazy init)
        let (avail, frozen) = ubs.deposit_from_transfer(999, 1, 100_0000_0000).unwrap();

        // Check: 100 avail
        assert_eq!(avail, 100_0000_0000);
        assert_eq!(frozen, 0);

        // Verify account was created
        let b = ubs.get_balance(999, 1).unwrap();
        assert_eq!(b.avail(), 100_0000_0000);
    }
}
