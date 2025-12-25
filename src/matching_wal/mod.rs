//! Matching Service WAL - Trade WAL Writer/Reader
//!
//! Business-layer WAL operations for Matching Service trades.

#[cfg(test)]
mod integration_tests;
pub mod recovery;
pub mod snapshot;
pub mod wal;

pub use recovery::{MatchingRecovery, RecoveryState};
pub use snapshot::{MatchingSnapshotter, SnapshotMetadata};
pub use wal::{MatchingWalReader, MatchingWalWriter, TradePayload};
