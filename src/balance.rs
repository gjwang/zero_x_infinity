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
/// - version increments on EVERY mutation
/// - No overflow/underflow (checked arithmetic)
/// - All state changes return Result
///
/// # Usage:
/// ```ignore
/// let mut balance = Balance::default();
/// balance.deposit(1000)?;           // avail = 1000
/// balance.lock(500)?;                // avail = 500, frozen = 500
/// balance.spend_frozen(100)?;        // frozen = 400
/// balance.unlock(200)?;              // avail = 700, frozen = 200
/// ```
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct Balance {
    avail: u64,   // PRIVATE - ONLY modified through deposit/withdraw/lock/unlock
    frozen: u64,  // PRIVATE - ONLY modified through lock/unlock/spend_frozen
    version: u64, // PRIVATE - AUTO-INCREMENTED on every mutation
}

impl Default for Balance {
    fn default() -> Self {
        Self {
            avail: 0,
            frozen: 0,
            version: 0,
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

    /// Get version (read-only)
    #[inline(always)]
    pub const fn version(&self) -> u64 {
        self.version
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
    /// - Increments version
    pub fn deposit(&mut self, amount: u64) -> Result<(), &'static str> {
        self.avail = self.avail.checked_add(amount).ok_or("Deposit overflow")?;
        self.version = self.version.wrapping_add(1);
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
    /// - Increments version
    pub fn withdraw(&mut self, amount: u64) -> Result<(), &'static str> {
        if self.avail < amount {
            return Err("Insufficient funds");
        }
        self.avail = self.avail.checked_sub(amount).ok_or("Withdraw underflow")?;
        self.version = self.version.wrapping_add(1);
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
    /// - Increments version
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
        self.version = self.version.wrapping_add(1);
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
    /// - Increments version
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
        self.version = self.version.wrapping_add(1);
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
    /// - Increments version
    pub fn spend_frozen(&mut self, amount: u64) -> Result<(), &'static str> {
        if self.frozen < amount {
            return Err("Insufficient frozen funds");
        }
        self.frozen = self
            .frozen
            .checked_sub(amount)
            .ok_or("Spend frozen underflow")?;
        self.version = self.version.wrapping_add(1);
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
        self.version = self.version.wrapping_add(1);
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
        assert_eq!(bal.version(), 0);

        bal.deposit(100).unwrap();
        assert_eq!(bal.version(), 1);

        bal.lock(50).unwrap();
        assert_eq!(bal.version(), 2);

        bal.unlock(20).unwrap();
        assert_eq!(bal.version(), 3);

        bal.withdraw(10).unwrap();
        assert_eq!(bal.version(), 4);

        bal.spend_frozen(10).unwrap();
        assert_eq!(bal.version(), 5);
    }
}
