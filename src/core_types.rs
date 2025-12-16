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
