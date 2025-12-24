//! Core types used throughout the system
//!
//! These are fundamental type aliases used by all modules.
//! They provide semantic meaning and enable future type evolution.

/// Asset ID - globally unique identifier for an asset.
///
/// # Constraints:
/// - **Immutable**: Once assigned, NEVER changes
/// - **Small Values**: Enables O(1) direct array indexing
/// - **Sequential**: Assigned contiguously (0, 1, 2, ...)
///
/// # Performance:
/// Used as array index for O(1) balance lookup:
/// ```ignore
/// assets[asset_id as usize]  // Direct access, no hash needed
/// ```
pub type AssetId = u32;

/// User ID - globally unique, immutable after assignment.
///
/// # Usage:
/// - Primary key for user accounts
/// - Used in HashMap for O(1) account lookup
pub type UserId = u64;

/// Order ID - unique within the system
pub type OrderId = u64;

/// Trade ID - unique within the system
pub type TradeId = u64;

/// Sequence number for ordering
pub type SeqNum = u64;

// =============================================================================
// System Reserved User IDs
// =============================================================================

/// REVENUE account - platform fee income (user_id = 0)
pub const REVENUE_ID: UserId = 0;

/// INSURANCE account - insurance fund for future use (user_id = 1)
pub const INSURANCE_ID: UserId = 1;

/// Maximum reserved system ID (0-1023 are system accounts)
pub const SYSTEM_MAX_ID: UserId = 1023;

/// First valid user ID (users start at 1024)
pub const USER_ID_START: UserId = 1024;

/// Check if a user_id is a system account (0-1023)
#[inline]
pub fn is_system_account(user_id: UserId) -> bool {
    user_id <= SYSTEM_MAX_ID
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_account_ids() {
        assert!(is_system_account(REVENUE_ID));
        assert!(is_system_account(INSURANCE_ID));
        assert!(is_system_account(1000));
        assert!(is_system_account(SYSTEM_MAX_ID));
        assert!(!is_system_account(USER_ID_START));
        assert!(!is_system_account(10000));
    }
}
