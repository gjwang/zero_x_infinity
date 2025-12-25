//! UBSCore WAL module (Phase 0x0D)

pub mod config;
pub mod recovery;
pub mod snapshot;
pub mod wal;

// Re-export main types
pub use config::UBSCoreConfig;
pub use recovery::{RecoveryState, UBSCoreRecovery};
pub use snapshot::{SnapshotMetadata, UBSCoreSnapshotter};
pub use wal::{UBSCoreWalReader, UBSCoreWalWriter};
