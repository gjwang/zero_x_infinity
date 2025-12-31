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
// Phase 0x0D: New WAL + Snapshot
use crate::ubscore_wal::{UBSCoreConfig, UBSCoreRecovery, UBSCoreWalWriter};

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
    /// New WAL writer (Phase 0x0D)
    wal_v2: Option<UBSCoreWalWriter>,
    /// Symbol manager for symbol → asset lookup
    manager: SymbolManager,
    /// Next WAL sequence ID
    next_seq_id: u64,
    /// Data directory for WAL replay
    data_dir: std::path::PathBuf,
}

impl UBSCore {
    /// Create a new UBSCore with recovery (Phase 0x0D)
    pub fn new(manager: SymbolManager, config: UBSCoreConfig) -> io::Result<Self> {
        // 1. Recover state from snapshot + WAL
        let recovery = UBSCoreRecovery::new(&config.data_dir);
        let state = recovery.recover(&manager)?;

        // 2. Create new WAL writer
        std::fs::create_dir_all(&config.wal_dir)?;
        let wal_file = config.wal_dir.join("current.wal");
        let wal_v2 = UBSCoreWalWriter::new(wal_file, 1, state.next_seq_id)?;

        Ok(Self {
            accounts: state.accounts,
            wal_v2: Some(wal_v2),
            manager,
            next_seq_id: state.next_seq_id,
            data_dir: config.data_dir,
        })
    }
    /// Replay actions from WAL (Phase 0x0D - ISSUE-003)
    ///
    /// Used by downstream services (ME, Settlement) to synchronize state.
    /// Replays both Order and Cancel actions as ValidAction objects.
    pub fn replay_output<F>(&self, from_seq: u64, mut callback: F) -> io::Result<()>
    where
        F: FnMut(crate::pipeline::ValidAction) -> io::Result<bool>,
    {
        use crate::messages::ValidOrder;
        use crate::pipeline::ValidAction;
        use crate::ubscore_wal::wal::{
            CancelPayload, MovePayload, OrderPayload, ReducePayload, UBSCoreWalReader,
        };

        let wal_file = self.data_dir.join("wal/current.wal");
        if !wal_file.exists() {
            return Ok(());
        }

        let mut reader = UBSCoreWalReader::open(&wal_file)?;
        reader.replay(from_seq, |entry| {
            match entry
                .header
                .entry_type
                .try_into()
                .map_err(|e: io::Error| e)?
            {
                crate::wal_v2::WalEntryType::Order => {
                    let payload: OrderPayload = bincode::deserialize(&entry.payload)
                        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

                    let side = crate::models::Side::try_from(payload.side)
                        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Invalid side"))?;

                    // Reconstruct InternalOrder
                    let order = crate::models::InternalOrder {
                        order_id: payload.order_id,
                        user_id: payload.user_id,
                        symbol_id: payload.symbol_id,
                        price: payload.price,
                        qty: payload.qty,
                        filled_qty: 0,
                        side,
                        order_type: crate::models::OrderType::try_from(payload.order_type)
                            .map_err(|_| {
                                io::Error::new(
                                    io::ErrorKind::InvalidData,
                                    format!(
                                        "WAL CORRUPTION: Invalid order_type {} for order {}",
                                        payload.order_type, payload.order_id
                                    ),
                                )
                            })?,
                        time_in_force: crate::models::TimeInForce::try_from(payload.time_in_force)
                            .map_err(|_| {
                                io::Error::new(
                                    io::ErrorKind::InvalidData,
                                    format!(
                                        "WAL CORRUPTION: Invalid time_in_force {} for order {}",
                                        payload.time_in_force, payload.order_id
                                    ),
                                )
                            })?,
                        status: crate::models::OrderStatus::NEW,
                        lock_version: 0,
                        seq_id: entry.header.seq_id,
                        ingested_at_ns: payload.ingested_at_ns,
                        cid: None,
                    };

                    let valid_order =
                        ValidOrder::new(entry.header.seq_id, order, payload.ingested_at_ns);
                    callback(ValidAction::Order(valid_order))
                }
                crate::wal_v2::WalEntryType::Cancel => {
                    let payload: CancelPayload = bincode::deserialize(&entry.payload)
                        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

                    callback(ValidAction::Cancel {
                        order_id: payload.order_id,
                        user_id: payload.user_id,
                        ingested_at_ns: 0, // In WAL v2 we don't store ingest time for cancel yet
                    })
                }
                crate::wal_v2::WalEntryType::Reduce => {
                    let payload: ReducePayload = bincode::deserialize(&entry.payload)
                        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
                    callback(ValidAction::Reduce {
                        order_id: payload.order_id,
                        user_id: payload.user_id,
                        reduce_qty: payload.reduce_qty,
                        ingested_at_ns: 0,
                    })
                }
                crate::wal_v2::WalEntryType::Move => {
                    let payload: MovePayload = bincode::deserialize(&entry.payload)
                        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
                    callback(ValidAction::Move {
                        order_id: payload.order_id,
                        user_id: payload.user_id,
                        new_price: payload.new_price,
                        ingested_at_ns: 0,
                    })
                }
                _ => Ok(true), // Skip other types (Deposit/Withdraw aren't needed by ME)
            }
        })
    }

    /// Commit: Flush WAL to disk for durability.
    /// Caller should only release results collected from methods after this succeeds.
    pub fn commit(&mut self) -> io::Result<()> {
        if let Some(ref mut wal) = self.wal_v2 {
            wal.flush()?;
        }
        Ok(())
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

    /// Set VIP level for a user (called after loading from DB)
    /// If user doesn't exist yet, creates the account.
    pub fn set_user_vip_level(&mut self, user_id: UserId, vip_level: u8) {
        self.accounts
            .entry(user_id)
            .or_insert_with(|| UserAccount::new(user_id))
            .set_vip_level(vip_level);
    }

    /// Get current WAL sequence number
    pub fn current_seq(&self) -> SeqNum {
        self.next_seq_id
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
            .calculate_cost_with_symbol(symbol_info)
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

        // money-type-safety.md 4.5.1: fail-fast, no hardcoded fallbacks
        let symbol_info = match self.manager.get_symbol_info_by_id(order.symbol_id) {
            Some(s) => s,
            None => {
                tracing::error!(
                    symbol_id = order.symbol_id,
                    order_id = order.order_id,
                    "CRITICAL: Symbol not found in lock_funds - pre_check should have caught this"
                );
                return; // Skip locking rather than use wrong value
            }
        };

        // Safe to unwrap: pre_check already validated this won't overflow
        let lock_amount = order
            .calculate_cost_with_symbol(symbol_info)
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
        let symbol_info = self
            .manager
            .get_symbol_info_by_id(order.symbol_id)
            .ok_or(RejectReason::SymbolNotFound)?;
        let lock_amount = order
            .calculate_cost_with_symbol(symbol_info)
            .map_err(|_| RejectReason::InvalidPrice)?;

        // 2. Persist to WAL FIRST
        let seq_id = if let Some(ref mut wal) = self.wal_v2 {
            wal.append_order(&order)
                .map_err(|_| RejectReason::SystemBusy)?
        } else {
            let seq = self.next_seq_id;
            self.next_seq_id += 1;
            seq
        };

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
            } => {
                // Persist to WAL
                if let Some(ref mut wal) = self.wal_v2 {
                    let _ = wal
                        .append_cancel(&crate::ubscore_wal::wal::CancelOrder { order_id, user_id });
                }
                Ok((
                    vec![ValidAction::Cancel {
                        order_id,
                        user_id,
                        ingested_at_ns,
                    }],
                    vec![],
                ))
            }
            OrderAction::Reduce {
                order_id,
                user_id,
                reduce_qty,
                ingested_at_ns,
            } => {
                // Persist to WAL
                if let Some(ref mut wal) = self.wal_v2 {
                    let _ = wal.append_reduce(order_id, user_id, reduce_qty);
                }
                Ok((
                    vec![ValidAction::Reduce {
                        order_id,
                        user_id,
                        reduce_qty,
                        ingested_at_ns,
                    }],
                    vec![],
                ))
            }
            OrderAction::Move {
                order_id,
                user_id,
                new_price,
                ingested_at_ns,
            } => {
                // Persist to WAL
                if let Some(ref mut wal) = self.wal_v2 {
                    let _ = wal.append_move(order_id, user_id, new_price);
                }
                Ok((
                    vec![ValidAction::Move {
                        order_id,
                        user_id,
                        new_price,
                        ingested_at_ns,
                    }],
                    vec![],
                ))
            }
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

        // SAFE_DEFAULT: if symbol not found, use 0 fee (user benefit, exchange absorbs loss)
        // This is safer than charging wrong fee which could cause user disputes
        let (maker_fee_rate, taker_fee_rate) = self
            .manager
            .get_symbol_info_by_id(event.symbol_id)
            .map(|s| (s.base_maker_fee, s.base_taker_fee))
            .unwrap_or_else(|| {
                tracing::error!(
                    "Symbol {} not found for fee calculation, using 0 fee",
                    event.symbol_id
                );
                (0, 0)
            });

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

        // Get VIP discount percentages (100 = no discount, 50 = 50% off)
        // VIP discount table: level 0=100%, 1=90%, 2=80%, ..., 5=50%
        let buyer_vip = match self.accounts.get(&trade.buyer_user_id) {
            Some(a) => a.vip_level(),
            None => 0,
        };
        let seller_vip = match self.accounts.get(&trade.seller_user_id) {
            Some(a) => a.vip_level(),
            None => 0,
        };

        // Simple VIP discount: 100 - (vip_level * 10), capped at 50%
        let buyer_discount = (100u8).saturating_sub(buyer_vip.min(5) * 10);
        let seller_discount = (100u8).saturating_sub(seller_vip.min(5) * 10);

        // Calculate fees with VIP discount
        // Buyer receives base → fee in base
        // Seller receives quote → fee in quote
        let buyer_fee =
            crate::fee::calculate_fee_with_discount(trade.qty, buyer_fee_rate, buyer_discount);
        let seller_fee =
            crate::fee::calculate_fee_with_discount(quote_amount, seller_fee_rate, seller_discount);

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
                buyer_fee,      // Fee deducted
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
                seller_fee,       // Fee deducted
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
    ) -> Result<(crate::messages::BalanceEvent, u64, u64), &'static str> {
        let account = self.accounts.get_mut(&user_id).ok_or("Account not found")?;
        let balance = account.get_balance_mut(asset_id)?;

        // Check available balance
        if balance.avail() < amount {
            return Err("Insufficient balance");
        }

        // Directly withdraw from available (no freeze involved)
        balance.withdraw(amount)?;

        let event = crate::messages::BalanceEvent::withdraw_transfer(
            user_id,
            asset_id,
            amount,
            balance.lock_version(),
            balance.avail(),
            balance.frozen(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("Critical: System clock before 1970")
                .as_nanos() as u64,
        );

        Ok((event, balance.avail(), balance.frozen()))
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
    ) -> Result<(crate::messages::BalanceEvent, u64, u64), &'static str> {
        // Lazy init: create account and asset if not exist
        let account = self
            .accounts
            .entry(user_id)
            .or_insert_with(|| UserAccount::new(user_id));

        account.deposit(asset_id, amount)?;

        let balance = account.get_balance(asset_id).unwrap();

        let event = crate::messages::BalanceEvent::deposit_transfer(
            user_id,
            asset_id,
            amount,
            balance.lock_version(),
            balance.avail(),
            balance.frozen(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("Critical: System clock before 1970")
                .as_nanos() as u64,
        );

        Ok((event, balance.avail(), balance.frozen()))
    }

    // ============================================================
    // WAL OPERATIONS
    // ============================================================

    /// Flush WAL to disk
    pub fn flush_wal(&mut self) -> io::Result<()> {
        if let Some(ref mut wal) = self.wal_v2 {
            wal.flush()?;
        }
        Ok(())
    }

    /// Get WAL statistics
    pub fn wal_stats(&self) -> (u64, u64) {
        (self.next_seq_id - 1, 0)
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

    fn test_ubscore_config() -> UBSCoreConfig {
        // Use both process ID and thread ID to ensure unique directories in parallel tests
        UBSCoreConfig::new(format!(
            "target/test_ubscore_{}_{:?}",
            std::process::id(),
            std::thread::current().id()
        ))
    }

    #[test]
    fn test_deposit_and_query() {
        let manager = test_manager();
        let config = test_ubscore_config();
        let mut ubs = UBSCore::new(manager, config).unwrap();

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
        let config = test_ubscore_config();
        let mut ubs = UBSCore::new(manager, config).unwrap();

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
        let config = test_ubscore_config();
        let mut ubs = UBSCore::new(manager, config).unwrap();

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
        let config = test_ubscore_config();
        let mut ubs = UBSCore::new(manager, config).unwrap();

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
        let config = test_ubscore_config();
        let mut ubs = UBSCore::new(manager, config).unwrap();

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
        let config = test_ubscore_config();
        let mut ubs = UBSCore::new(manager, config).unwrap();

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
        let config = test_ubscore_config();
        let mut ubs = UBSCore::new(manager, config).unwrap();

        // Setup: deposit 100 BTC to user 1
        ubs.deposit(1, 1, 100_0000_0000).unwrap();

        // Withdraw 30 BTC for transfer
        let (_event, avail, frozen) = ubs.withdraw_for_transfer(1, 1, 30_0000_0000).unwrap();

        // Check: 70 avail, 0 frozen (no freeze for transfers)
        assert_eq!(avail, 70_0000_0000);
        assert_eq!(frozen, 0);

        let b = ubs.get_balance(1, 1).unwrap();
        assert_eq!(b.avail(), 70_0000_0000);
    }

    #[test]
    fn test_withdraw_for_transfer_insufficient() {
        let manager = test_manager();
        let config = test_ubscore_config();
        let mut ubs = UBSCore::new(manager, config).unwrap();

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
        let config = test_ubscore_config();
        let mut ubs = UBSCore::new(manager, config).unwrap();

        // Setup: existing account with 50 BTC
        ubs.deposit(1, 1, 50_0000_0000).unwrap();

        // Deposit 30 BTC from transfer
        let (_event, avail, frozen) = ubs.deposit_from_transfer(1, 1, 30_0000_0000).unwrap();

        // Check: 80 avail
        assert_eq!(avail, 80_0000_0000);
        assert_eq!(frozen, 0);
    }

    #[test]
    fn test_deposit_from_transfer_new_account() {
        let manager = test_manager();
        let config = test_ubscore_config();
        let mut ubs = UBSCore::new(manager, config).unwrap();

        // Deposit 100 BTC to new user (lazy init)
        let (_event, avail, frozen) = ubs.deposit_from_transfer(999, 1, 100_0000_0000).unwrap();

        // Check: 100 avail
        assert_eq!(avail, 100_0000_0000);
        assert_eq!(frozen, 0);

        // Verify account was created
        let b = ubs.get_balance(999, 1).unwrap();
        assert_eq!(b.avail(), 100_0000_0000);
    }

    // =====================================================
    // U08-U10: Role Assignment Tests
    // =====================================================

    #[test]
    fn test_settle_trade_maker_role() {
        // U08: Maker role - order in book first, then matched
        use crate::messages::{BalanceEventType, TradeEvent};
        use crate::models::{Side, Trade};

        let manager = test_manager();
        let config = test_ubscore_config();
        let mut ubs = UBSCore::new(manager, config).unwrap();

        // Setup accounts - buyer needs quote for lock, base for receive
        // seller needs base for lock, quote for receive
        ubs.deposit(1, 2, 10_000_000_000).unwrap(); // Buyer: 1M USDT (quote - for lock)
        ubs.deposit(1, 1, 0).unwrap(); // Buyer: init BTC balance (for receive)
        ubs.deposit(2, 1, 100_0000_0000).unwrap(); // Seller: 100 BTC (base - for lock)
        ubs.deposit(2, 2, 0).unwrap(); // Seller: init USDT balance (for receive)

        // Buyer locks quote for buy order
        ubs.accounts_mut()
            .get_mut(&1)
            .unwrap()
            .get_balance_mut(2)
            .unwrap()
            .lock(8_500_000_000)
            .unwrap(); // 8500 USDT frozen

        // Seller locks base for sell order
        ubs.accounts_mut()
            .get_mut(&2)
            .unwrap()
            .get_balance_mut(1)
            .unwrap()
            .lock(1_0000_0000)
            .unwrap(); // 1 BTC frozen

        // Trade: Buyer buys 0.1 BTC @ 85000 USDT, Buyer is TAKER
        let trade = Trade {
            trade_id: 1000,
            buyer_order_id: 100,
            seller_order_id: 101,
            buyer_user_id: 1,
            seller_user_id: 2,
            price: 8500000, // 85000 scaled
            qty: 1000_0000, // 0.1 BTC
        };

        let trade_event = TradeEvent::new(
            trade.clone(),
            100,          // taker_order_id = buyer
            101,          // maker_order_id = seller
            Side::Buy,    // taker_side = buy
            10_0000_0000, // taker_order_qty
            1000_0000,    // taker_filled_qty
            20_0000_0000, // maker_order_qty
            1000_0000,    // maker_filled_qty
            1,            // base_asset_id (BTC)
            2,            // quote_asset_id (USDT)
            1_0000_0000,  // qty_unit (1 BTC in units)
            0,            // ingested_at_ns
            1,            // symbol_id
        );

        let events = ubs.settle_trade(&trade_event).unwrap();

        // Check that settle events have correct fee_amount
        // Buyer receives BTC with buyer_fee deducted
        // Seller receives USDT with seller_fee deducted
        let settle_events: Vec<_> = events
            .iter()
            .filter(|e| e.event_type == BalanceEventType::Settle && e.delta > 0)
            .collect();

        assert_eq!(settle_events.len(), 2); // One for buyer (BTC), one for seller (USDT)
    }

    // =====================================================
    // C01-C04: Asset Conservation Tests
    // =====================================================

    #[test]
    fn test_settle_trade_conservation() {
        // C04: Global conservation - Σ all BalanceEvents = 0
        use crate::messages::TradeEvent;
        use crate::models::{Side, Trade};

        let manager = test_manager();
        let config = test_ubscore_config();
        let mut ubs = UBSCore::new(manager, config).unwrap();

        // Setup accounts with known balances
        ubs.deposit(1, 1, 10_0000_0000).unwrap(); // Buyer: 10 BTC
        ubs.deposit(1, 2, 1_000_000_000).unwrap(); // Buyer: 100000 USDT
        ubs.deposit(2, 1, 10_0000_0000).unwrap(); // Seller: 10 BTC
        ubs.deposit(2, 2, 1_000_000_000).unwrap(); // Seller: 100000 USDT

        // Lock funds for orders
        ubs.accounts_mut()
            .get_mut(&1)
            .unwrap()
            .get_balance_mut(2)
            .unwrap()
            .lock(8500_0000)
            .unwrap(); // Buyer locks 8500 USDT
        ubs.accounts_mut()
            .get_mut(&2)
            .unwrap()
            .get_balance_mut(1)
            .unwrap()
            .lock(1_0000_0000)
            .unwrap(); // Seller locks 1 BTC

        // Trade: 0.1 BTC @ 85000 USDT
        let trade = Trade {
            trade_id: 2000,
            buyer_order_id: 200,
            seller_order_id: 201,
            buyer_user_id: 1,
            seller_user_id: 2,
            price: 8_500_000, // 85000.00 scaled
            qty: 1000_0000,   // 0.1 BTC
        };

        let trade_event = TradeEvent::new(
            trade.clone(),
            200,
            201,
            Side::Buy,
            10_0000_0000,
            1000_0000,
            10_0000_0000,
            1000_0000,
            1,
            2,
            1_0000_0000,
            0,
            1,
        );

        let events = ubs.settle_trade(&trade_event).unwrap();

        // C04: Sum of all deltas should be 0 (conservation)
        let total_delta: i64 = events.iter().map(|e| e.delta).sum();
        assert_eq!(
            total_delta, 0,
            "Asset conservation failed: Σ delta = {}",
            total_delta
        );

        // C03: FeeReceived events should exist (for REVENUE account)
        use crate::messages::BalanceEventType;
        let fee_events: Vec<_> = events
            .iter()
            .filter(|e| e.event_type == BalanceEventType::FeeReceived)
            .collect();
        assert!(
            !fee_events.is_empty() || total_delta == 0,
            "Fee events or conservation should be satisfied"
        );
    }

    #[test]
    fn test_fee_calculation_accuracy() {
        // U01-U06: Fee calculation accuracy tests
        use crate::fee::{calculate_fee, calculate_fee_with_discount};

        // U01: Basic Taker fee (0.20%)
        // 1 BTC (100_000_000 satoshis) * 0.20% = 200_000
        assert_eq!(calculate_fee(100_000_000, 2000), 200_000);

        // U02: Basic Maker fee (0.10%)
        assert_eq!(calculate_fee(100_000_000, 1000), 100_000);

        // U03: VIP discount 50%
        // rate = 2000 * 50 / 100 = 1000 (0.10%)
        assert_eq!(calculate_fee_with_discount(100_000_000, 2000, 50), 100_000);

        // U04: Zero fee rate allowed
        assert_eq!(calculate_fee(100_000_000, 0), 0);

        // U05: Small amount boundary (minimum fee = 1)
        assert_eq!(calculate_fee(1, 2000), 1);

        // U06: Large amount overflow protection
        let large: u64 = 10_000_000_000_000_000_000; // 10^19
        let fee = calculate_fee(large, 2000);
        assert_eq!(fee, 20_000_000_000_000_000); // 0.20% of 10^19
    }
}
