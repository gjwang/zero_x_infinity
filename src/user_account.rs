use serde::{Deserialize, Serialize};

// Import the ENFORCED Balance type
pub use crate::enforced_balance::Balance;

/// Asset ID - globally unique identifier for an asset.
///
/// # Critical Constraints:
/// 1. **Immutable**: Once assigned, NEVER changes
/// 2. **Small Values**: Must be small integers (used as direct array index)
/// 3. **Sequential**: Assigned contiguously (e.g., 1, 2, 3, ... or 0, 1, 2, ...)
///
/// # O(1) Direct Array Indexing:
/// ```ignore
/// assets[asset_id as usize]  // Direct access, no search needed
/// ```
///
/// # Performance:
/// | Method          | Lookup | Memory     | Cache |
/// |-----------------|--------|------------|-------|
/// | Vec + linear    | O(n)   | Compact    | Good  |
/// | HashMap         | O(1)*  | +Overhead  | Poor  |
/// | **Direct Index**| O(1)   | Compact    | Best  |
///
/// *HashMap O(1) has hidden constant overhead (hashing, bucket lookup)
pub type AssetId = u32;

/// User ID - globally unique, immutable after assignment.
pub type UserId = u64;

// Balance is now imported from enforced_balance module
// All fields are private, all operations enforced

/// UserAccount represents a user's account with balances across multiple assets.
///
/// # Data Structure:
/// Uses `Vec<Balance>` where `asset_id` is used directly as the array index.
/// This provides O(1) lookup with optimal cache performance.
///
/// # Why Vec<Balance> with Direct Indexing?
///
/// 1. **O(1) Lookup**: `assets[asset_id]` - no search, no hashing
///
/// 2. **Cache-Friendly**: Contiguous memory layout.
///    When CPU loads one Balance, adjacent Balances are also loaded
///    into L1/L2 cache (64-byte cache line), making subsequent
///    accesses nearly zero-latency.
///
/// 3. **High-Frequency Function**: `get_balance()` is called 5-10 times
///    per order (check balance, freeze, settle buyer/seller, refund).
///    At 10K orders/sec, that's 50-100K calls/sec.
///    O(1) + cache-friendly is critical for performance.
///
/// # Invariants (enforced by private fields):
/// 1. user_id is immutable after creation
/// 2. assets can only be accessed through get_balance methods
/// 3. All mutations go through validated operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserAccount {
    user_id: UserId,      // PRIVATE - use user_id()
    assets: Vec<Balance>, // PRIVATE - O(1) index by asset_id: assets[asset_id]
}

impl UserAccount {
    /// Create a new user account with pre-allocated asset slots.
    /// Default capacity is 8 assets, which covers most users.
    pub fn new(user_id: UserId) -> Self {
        Self {
            user_id,
            assets: Vec::with_capacity(8),
        }
    }

    /// Read-only access to user ID
    #[inline(always)]
    pub fn user_id(&self) -> UserId {
        self.user_id
    }

    /// Deposit funds to an asset.
    /// This is the ONLY way to create a new asset slot.
    /// Auto-creates the asset slot if it doesn't exist.
    ///
    /// # Errors
    /// Returns error on overflow.
    #[inline(always)]
    pub fn deposit(&mut self, asset_id: AssetId, amount: u64) -> Result<(), &'static str> {
        let idx = asset_id as usize;
        // Auto-create asset slot if needed (only deposit can do this)
        if idx >= self.assets.len() {
            self.assets.resize(idx + 1, Balance::default());
        }
        self.assets[idx].deposit(amount)
    }

    /// Get mutable reference to balance for an asset.
    /// O(1) direct array indexing.
    ///
    /// # Errors
    /// Returns error if asset doesn't exist.
    /// Use `deposit()` first to create the asset slot.
    #[inline(always)]
    pub fn get_balance_mut(&mut self, asset_id: AssetId) -> Result<&mut Balance, &'static str> {
        let idx = asset_id as usize;
        self.assets.get_mut(idx).ok_or("Asset not found")
    }

    /// Get immutable reference to balance for an asset.
    /// O(1) direct array indexing.
    /// Returns None if asset doesn't exist.
    #[inline(always)]
    pub fn get_balance(&self, asset_id: AssetId) -> Option<&Balance> {
        let idx = asset_id as usize;
        self.assets.get(idx)
    }

    /// Get read-only slice of all balances.
    /// Index corresponds to asset_id.
    #[inline(always)]
    pub fn assets(&self) -> &[Balance] {
        &self.assets
    }

    /// Get mutable access to assets (for internal ledger operations)
    #[inline(always)]
    pub(crate) fn assets_mut(&mut self) -> &mut Vec<Balance> {
        &mut self.assets
    }

    pub fn check_buyer_balance(
        &self,
        quote_asset_id: AssetId,
        spend_quote: u64,
        refund_quote: u64,
    ) -> Result<(), &'static str> {
        let quote_bal = self
            .get_balance(quote_asset_id)
            .ok_or("Quote asset not found")?;

        let required = spend_quote + refund_quote;
        if quote_bal.frozen() < required {
            return Err("Insufficient frozen quote funds");
        }
        Ok(())
    }

    pub fn check_seller_balance(
        &self,
        base_asset_id: AssetId,
        spend_base: u64,
        refund_base: u64,
    ) -> Result<(), &'static str> {
        let base_bal = self
            .get_balance(base_asset_id)
            .ok_or("Base asset not found")?;

        let required = spend_base + refund_base;
        if base_bal.frozen() < required {
            return Err("Insufficient frozen base funds");
        }
        Ok(())
    }

    pub fn settle_as_buyer(
        &mut self,
        quote_asset_id: AssetId,
        base_asset_id: AssetId,
        spend_quote: u64,
        gain_base: u64,
        refund_quote: u64,
    ) -> Result<(), &'static str> {
        // Debit Quote (Frozen)
        self.get_balance_mut(quote_asset_id)?
            .spend_frozen(spend_quote)?;

        // Credit Base (Available)
        self.get_balance_mut(base_asset_id)?.deposit(gain_base)?;

        // Refund Quote (Frozen -> Available)
        if refund_quote > 0 {
            self.get_balance_mut(quote_asset_id)?.unlock(refund_quote)?;
        }
        Ok(())
    }

    pub fn settle_as_seller(
        &mut self,
        base_asset_id: AssetId,
        quote_asset_id: AssetId,
        spend_base: u64,
        gain_quote: u64,
        refund_base: u64,
    ) -> Result<(), &'static str> {
        // Debit Base (Frozen)
        self.get_balance_mut(base_asset_id)?
            .spend_frozen(spend_base)?;

        // Credit Quote (Available)
        self.get_balance_mut(quote_asset_id)?.deposit(gain_quote)?;

        // Refund Base (Frozen -> Available)
        if refund_base > 0 {
            self.get_balance_mut(base_asset_id)?.unlock(refund_base)?;
        }
        Ok(())
    }
}
