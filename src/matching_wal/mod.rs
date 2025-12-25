//! Matching Service WAL - Trade WAL Writer/Reader
//!
//! Business-layer WAL operations for Matching Service trades.

pub mod snapshot;
pub mod wal;

pub use snapshot::{MatchingSnapshotter, SnapshotMetadata};
pub use wal::{MatchingWalReader, MatchingWalWriter, TradePayload};
