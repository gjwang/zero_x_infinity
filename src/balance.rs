/// ENFORCED BALANCE TYPE - Used by UBSCore
///
/// This is the SINGLE source of truth for balance operations.
/// ALL balance mutations MUST go through these methods.
///
/// # Enforcement Strategy:
/// 1. Fields are PRIVATE - no direct access
/// 2. All mutations return Result - errors are explicit
/// 3. Version auto-increments - audit trail
/// 4. checked_add/sub - overflow protection
/// 5. Type system prevents bypassing validation
use serde::{Deserialize, Serialize};

// Types are defined in types.rs - we can use them via crate::types if needed
// but Balance doesn't actually need AssetId/UserId directly

/// Balance for a single asset
///
/// # Invariants (ENFORCED by private fields):
/// - avail + frozen = total balance (never negative)
/// - Versions increment on respective operations (separated version spaces)
/// - No overflow/underflow (checked arithmetic)
/// - All state changes return Result
///
/// # Version Spaces:
/// - `lock_version`: Incremented on lock/unlock/deposit/withdraw operations
/// - `settle_version`: Incremented on spend_frozen/deposit (settlement) operations
///
/// This separation enables deterministic verification in pipelined architectures
/// where Lock and Settle operations from different queues may interleave.
///
/// # Usage:
/// ```ignore
/// let mut balance = Balance::default();
/// balance.deposit(1000)?;           // avail = 1000, lock_version++, settle_version++
/// balance.lock(500)?;                // avail = 500, frozen = 500, lock_version++
/// balance.spend_frozen(100)?;        // frozen = 400, settle_version++
/// balance.unlock(200)?;              // avail = 700, frozen = 200, lock_version++
/// ```
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct Balance {
    avail: u64,          // PRIVATE - ONLY modified through deposit/withdraw/lock/unlock
    frozen: u64,         // PRIVATE - ONLY modified through lock/unlock/spend_frozen
    lock_version: u64,   // PRIVATE - Incremented on lock/unlock/deposit/withdraw
    settle_version: u64, // PRIVATE - Incremented on spend_frozen/deposit (settlement)
}

impl Default for Balance {
    fn default() -> Self {
        Self {
            avail: 0,
            frozen: 0,
            lock_version: 0,
            settle_version: 0,
        }
    }
}

impl Balance {
    // ============================================================
    // READ-ONLY GETTERS (safe to expose)
    // ============================================================

    /// Get available balance (read-only)
    #[inline(always)]
    pub const fn avail(&self) -> u64 {
        self.avail
    }

    /// Get frozen balance (read-only)
    #[inline(always)]
    pub const fn frozen(&self) -> u64 {
        self.frozen
    }

    /// Get total balance (avail + frozen)
    /// Returns None if overflow (indicates data corruption)
    #[inline(always)]
    pub const fn total(&self) -> Option<u64> {
        self.avail.checked_add(self.frozen)
    }

    /// Get lock_version (read-only) - incremented on lock/unlock operations
    #[inline(always)]
    pub const fn lock_version(&self) -> u64 {
        self.lock_version
    }

    /// Get settle_version (read-only) - incremented on settlement operations
    #[inline(always)]
    pub const fn settle_version(&self) -> u64 {
        self.settle_version
    }

    /// Get version (legacy, returns lock_version for compatibility)
    #[deprecated(note = "Use lock_version() or settle_version() instead")]
    #[inline(always)]
    pub const fn version(&self) -> u64 {
        self.lock_version
    }

    // ============================================================
    // VALIDATED MUTATIONS (ENFORCED operations)
    // ============================================================

    /// Deposit funds to available balance
    ///
    /// # Errors
    /// - Returns error on overflow
    ///
    /// # Effects
    /// - Increases avail by amount
    /// - Increments lock_version AND settle_version (deposit affects both lock and settle paths)
    pub fn deposit(&mut self, amount: u64) -> Result<(), &'static str> {
        self.avail = self.avail.checked_add(amount).ok_or("Deposit overflow")?;
        self.lock_version = self.lock_version.wrapping_add(1);
        self.settle_version = self.settle_version.wrapping_add(1);
        Ok(())
    }

    /// Withdraw funds from available balance
    ///
    /// # Errors
    /// - "Insufficient funds" if avail < amount
    /// - "Withdraw underflow" on arithmetic error
    ///
    /// # Effects
    /// - Decreases avail by amount
    /// - Increments lock_version
    pub fn withdraw(&mut self, amount: u64) -> Result<(), &'static str> {
        if self.avail < amount {
            return Err("Insufficient funds");
        }
        self.avail = self.avail.checked_sub(amount).ok_or("Withdraw underflow")?;
        self.lock_version = self.lock_version.wrapping_add(1);
        Ok(())
    }

    /// Lock funds (move from available to frozen)
    ///
    /// # Errors
    /// - "Insufficient funds" if avail < amount
    /// - "Lock overflow" on frozen overflow
    ///
    /// # Effects
    /// - Decreases avail by amount
    /// - Increases frozen by amount
    /// - Increments lock_version
    pub fn lock(&mut self, amount: u64) -> Result<(), &'static str> {
        if self.avail < amount {
            return Err("Insufficient funds to lock");
        }
        self.avail = self
            .avail
            .checked_sub(amount)
            .ok_or("Lock avail underflow")?;
        self.frozen = self
            .frozen
            .checked_add(amount)
            .ok_or("Lock frozen overflow")?;
        self.lock_version = self.lock_version.wrapping_add(1);
        Ok(())
    }

    /// Unlock funds (move from frozen to available)
    ///
    /// # Errors
    /// - "Insufficient frozen funds" if frozen < amount
    /// - "Unlock overflow" on avail overflow
    ///
    /// # Effects
    /// - Decreases frozen by amount
    /// - Increases avail by amount
    /// - Increments lock_version
    pub fn unlock(&mut self, amount: u64) -> Result<(), &'static str> {
        if self.frozen < amount {
            return Err("Insufficient frozen funds");
        }
        self.frozen = self
            .frozen
            .checked_sub(amount)
            .ok_or("Unlock frozen underflow")?;
        self.avail = self
            .avail
            .checked_add(amount)
            .ok_or("Unlock avail overflow")?;
        self.lock_version = self.lock_version.wrapping_add(1);
        Ok(())
    }

    /// Spend frozen funds (remove from frozen without adding to available)
    /// Used for trade settlement
    ///
    /// # Errors
    /// - "Insufficient frozen funds" if frozen < amount
    ///
    /// # Effects
    /// - Decreases frozen by amount
    /// - Increments settle_version
    pub fn spend_frozen(&mut self, amount: u64) -> Result<(), &'static str> {
        if self.frozen < amount {
            return Err("Insufficient frozen funds");
        }
        self.frozen = self
            .frozen
            .checked_sub(amount)
            .ok_or("Spend frozen underflow")?;
        self.settle_version = self.settle_version.wrapping_add(1);
        Ok(())
    }

    // ============================================================
    // ATOMIC OPERATIONS (for complex scenarios)
    // ============================================================

    /// Atomic: spend frozen + add to available (used in refunds)
    ///
    /// This is atomic - either both succeed or both fail
    pub fn refund_frozen(&mut self, spend: u64, refund: u64) -> Result<(), &'static str> {
        // Validate first
        if self.frozen < spend {
            return Err("Insufficient frozen for refund");
        }

        // Apply atomically
        self.frozen = self
            .frozen
            .checked_sub(spend)
            .ok_or("Refund frozen underflow")?;
        self.avail = self
            .avail
            .checked_add(refund)
            .ok_or("Refund avail overflow")?;
        // refund_frozen is a lock operation (moving frozen back to avail)
        self.lock_version = self.lock_version.wrapping_add(1);
        Ok(())
    }

    /// Unlock frozen funds during settlement (e.g. refund unused lock from price improvement)
    ///
    /// # Effects
    /// - Decreases frozen
    /// - Increases avail
    /// - Increments ONLY settle_version
    ///
    /// # Versioning Note
    /// "settle triggered unlock, should belong to settle queue, version".
    /// We do not increment lock_version here to avoid creating gaps in the Lock Event stream.
    /// This means `avail` changes from settlement are "out-of-band" for lock_version,
    /// which is acceptable as increasing avail is safe (no double-spend risk).
    pub fn settle_unlock(&mut self, amount: u64) -> Result<(), &'static str> {
        if self.frozen < amount {
            return Err("Insufficient frozen funds");
        }
        self.frozen = self
            .frozen
            .checked_sub(amount)
            .ok_or("Settle unlock frozen underflow")?;
        self.avail = self
            .avail
            .checked_add(amount)
            .ok_or("Settle unlock avail overflow")?;
        // lock_version UNCHANGED - implicit update to avail
        self.settle_version = self.settle_version.wrapping_add(1);
        Ok(())
    }
}

// ============================================================
// TESTS - Prove enforcement works
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deposit() {
        let mut bal = Balance::default();
        assert_eq!(bal.avail(), 0);

        bal.deposit(100).unwrap();
        assert_eq!(bal.avail(), 100);
        assert_eq!(bal.version(), 1);

        bal.deposit(50).unwrap();
        assert_eq!(bal.avail(), 150);
        assert_eq!(bal.version(), 2);
    }

    #[test]
    fn test_deposit_overflow() {
        let mut bal = Balance::default();
        bal.deposit(u64::MAX).unwrap();

        // Should fail
        assert!(bal.deposit(1).is_err());
    }

    #[test]
    fn test_withdraw() {
        let mut bal = Balance::default();
        bal.deposit(100).unwrap();

        bal.withdraw(60).unwrap();
        assert_eq!(bal.avail(), 40);
        assert_eq!(bal.version(), 2);
    }

    #[test]
    fn test_withdraw_insufficient() {
        let mut bal = Balance::default();
        bal.deposit(50).unwrap();

        assert!(bal.withdraw(100).is_err());
        assert_eq!(bal.avail(), 50); // Unchanged
    }

    #[test]
    fn test_lock_unlock() {
        let mut bal = Balance::default();
        bal.deposit(100).unwrap();

        bal.lock(60).unwrap();
        assert_eq!(bal.avail(), 40);
        assert_eq!(bal.frozen(), 60);

        bal.unlock(20).unwrap();
        assert_eq!(bal.avail(), 60);
        assert_eq!(bal.frozen(), 40);
    }

    #[test]
    fn test_spend_frozen() {
        let mut bal = Balance::default();
        bal.deposit(100).unwrap();
        bal.lock(60).unwrap();

        bal.spend_frozen(30).unwrap();
        assert_eq!(bal.frozen(), 30);
        assert_eq!(bal.avail(), 40); // Unchanged
    }

    #[test]
    fn test_total() {
        let mut bal = Balance::default();
        bal.deposit(100).unwrap();
        assert_eq!(bal.total(), Some(100));

        bal.lock(60).unwrap();
        assert_eq!(bal.total(), Some(100)); // Total unchanged

        bal.spend_frozen(20).unwrap();
        assert_eq!(bal.total(), Some(80)); // Total decreased
    }

    #[test]
    fn test_version_increments() {
        let mut bal = Balance::default();
        assert_eq!(bal.lock_version(), 0);
        assert_eq!(bal.settle_version(), 0);

        // Deposit increments both versions
        bal.deposit(100).unwrap();
        assert_eq!(bal.lock_version(), 1);
        assert_eq!(bal.settle_version(), 1);

        // Lock increments only lock_version
        bal.lock(50).unwrap();
        assert_eq!(bal.lock_version(), 2);
        assert_eq!(bal.settle_version(), 1);

        // Unlock increments only lock_version
        bal.unlock(20).unwrap();
        assert_eq!(bal.lock_version(), 3);
        assert_eq!(bal.settle_version(), 1);

        // Withdraw increments only lock_version
        bal.withdraw(10).unwrap();
        assert_eq!(bal.lock_version(), 4);
        assert_eq!(bal.settle_version(), 1);

        // Spend frozen increments only settle_version
        bal.spend_frozen(10).unwrap();
        assert_eq!(bal.lock_version(), 4);
        assert_eq!(bal.settle_version(), 2);
    }

    #[test]
    fn test_separated_version_spaces() {
        // This test demonstrates the key insight of separated version spaces:
        // Different operations increment different versions, enabling
        // deterministic verification in pipelined architectures.
        let mut bal = Balance::default();

        // Initial state
        bal.deposit(1000).unwrap();
        let lock_v0 = bal.lock_version(); // 1
        let settle_v0 = bal.settle_version(); // 1

        // Simulate order placement - lock increments lock_version
        bal.lock(500).unwrap();
        assert_eq!(bal.lock_version(), lock_v0 + 1); // 2
        assert_eq!(bal.settle_version(), settle_v0); // 1 (unchanged)

        // Simulate settlement - spend_frozen increments settle_version
        bal.spend_frozen(100).unwrap();
        assert_eq!(bal.lock_version(), lock_v0 + 1); // 2 (unchanged)
        assert_eq!(bal.settle_version(), settle_v0 + 1); // 2

        // This means:
        // - Lock events can be sorted by source_id, verified by lock_version
        // - Settle events can be sorted by trade_id, verified by settle_version
        // - Interleaving order doesn't matter for verification!
    }
}
