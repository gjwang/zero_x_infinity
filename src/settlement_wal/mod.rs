//! Settlement Service WAL - Checkpoint WAL Writer/Reader
//!
//! Business-layer WAL operations for Settlement Service progress tracking.
//! This is a lightweight WAL that only records processing checkpoints.

pub mod recovery;
pub mod snapshot;
pub mod wal;

pub use recovery::{RecoveryResult, SettlementRecovery};
pub use snapshot::{SettlementSnapshot, SettlementSnapshotter};
pub use wal::{CheckpointPayload, SettlementWalReader, SettlementWalWriter};
