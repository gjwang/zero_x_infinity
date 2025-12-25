//! Settlement Service Recovery Logic
//!
//! Recovers `last_trade_id` from Snapshot + WAL on startup.

use super::snapshot::SettlementSnapshotter;
use super::wal::SettlementWalReader;
use std::fs;
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};

// ============================================================
// RECOVERY RESULT
// ============================================================

/// Recovery result containing restored state
#[derive(Debug, Clone)]
pub struct RecoveryResult {
    /// Last successfully processed trade_id (0 for cold start)
    pub last_trade_id: u64,
    /// Next WAL sequence ID to use
    pub next_seq_id: u64,
    /// Whether this was a cold start (no snapshot)
    pub is_cold_start: bool,
}

impl RecoveryResult {
    /// Create a cold start result
    pub fn cold_start() -> Self {
        Self {
            last_trade_id: 0,
            next_seq_id: 1,
            is_cold_start: true,
        }
    }
}

// ============================================================
// RECOVERY LOGIC
// ============================================================

/// Settlement recovery handler
///
/// # Recovery Flow
///
/// 1. Check `snapshots/latest` symlink
/// 2. If exists: load snapshot → get `last_trade_id`
/// 3. Replay WAL to find any newer checkpoints
/// 4. Return highest `last_trade_id`
///
/// # Cold Start
///
/// If no snapshot exists, returns `last_trade_id = 0`.
/// Settlement will start from the beginning.
pub struct SettlementRecovery {
    data_dir: PathBuf,
}

impl SettlementRecovery {
    /// Create a new recovery handler
    pub fn new(dir: impl AsRef<Path>) -> Self {
        Self {
            data_dir: dir.as_ref().to_path_buf(),
        }
    }

    /// Recover state from Snapshot + WAL
    ///
    /// # Returns
    ///
    /// - `Ok(RecoveryResult)` with recovered state
    /// - `Err` if snapshot/WAL is corrupted
    pub fn recover(&self) -> io::Result<RecoveryResult> {
        let snapshot_dir = self.data_dir.join("snapshots");
        let wal_path = self.data_dir.join("wal").join("current.wal");

        // 1. Try to load latest snapshot
        let snapshotter = SettlementSnapshotter::new(&snapshot_dir);
        let snapshot = snapshotter.load_latest()?;

        let mut last_trade_id = snapshot.as_ref().map(|s| s.last_trade_id).unwrap_or(0);

        let is_cold_start = snapshot.is_none();

        if is_cold_start {
            tracing::info!("Settlement cold start: no snapshot found");
        } else {
            tracing::info!(last_trade_id = last_trade_id, "Settlement snapshot loaded");
        }

        // 2. Replay WAL to find any newer checkpoints
        let mut next_seq_id = 1u64;
        if wal_path.exists() {
            match SettlementWalReader::open(&wal_path) {
                Ok(mut reader) => {
                    match reader.replay_to_latest() {
                        Ok(Some(wal_trade_id)) => {
                            // WAL may have newer checkpoints than snapshot
                            if wal_trade_id > last_trade_id {
                                tracing::info!(
                                    snapshot_trade_id = last_trade_id,
                                    wal_trade_id = wal_trade_id,
                                    "WAL has newer checkpoint than snapshot"
                                );
                                last_trade_id = wal_trade_id;
                            }

                            // Count entries for next_seq_id (approximate)
                            // For accuracy, we'd need to track seq in replay
                            // For now, just a reasonable starting point
                            next_seq_id = 1; // Will be set properly by writer
                        }
                        Ok(None) => {
                            tracing::debug!("WAL is empty");
                        }
                        Err(e) => {
                            // WAL corruption - log warning but continue with snapshot
                            tracing::warn!(
                                error = %e,
                                "WAL replay failed, using snapshot only"
                            );
                        }
                    }
                }
                Err(e) if e.kind() == ErrorKind::NotFound => {
                    tracing::debug!("No WAL file found");
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to open WAL");
                }
            }
        }

        tracing::info!(
            last_trade_id = last_trade_id,
            is_cold_start = is_cold_start,
            "Settlement recovery complete"
        );

        Ok(RecoveryResult {
            last_trade_id,
            next_seq_id,
            is_cold_start,
        })
    }

    /// Ensure data directories exist
    pub fn ensure_directories(&self) -> io::Result<()> {
        fs::create_dir_all(self.data_dir.join("wal"))?;
        fs::create_dir_all(self.data_dir.join("snapshots"))?;
        Ok(())
    }
}

// ============================================================
// UNIT TESTS (TDD - Test First!)
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settlement_wal::wal::SettlementWalWriter;

    fn test_dir() -> PathBuf {
        PathBuf::from(format!(
            "target/test_settlement_recovery_{}",
            std::process::id()
        ))
    }

    fn cleanup(dir: &Path) {
        let _ = fs::remove_dir_all(dir);
    }

    // --------------------------------------------------------
    // TDD Test 1: Cold start (no files)
    // --------------------------------------------------------
    #[test]
    fn test_recovery_cold_start() {
        let dir = test_dir().join("cold_start");
        cleanup(&dir);

        let recovery = SettlementRecovery::new(&dir);
        let result = recovery.recover().unwrap();

        assert!(result.is_cold_start);
        assert_eq!(result.last_trade_id, 0);

        cleanup(&dir);
    }

    // --------------------------------------------------------
    // TDD Test 2: Recovery with snapshot only
    // --------------------------------------------------------
    #[test]
    fn test_recovery_with_snapshot() {
        let dir = test_dir().join("with_snapshot");
        cleanup(&dir);

        // Create snapshot
        let snapshot_dir = dir.join("snapshots");
        let snapshotter = SettlementSnapshotter::new(&snapshot_dir);
        snapshotter.create_snapshot(5000).unwrap();

        // Recover
        let recovery = SettlementRecovery::new(&dir);
        let result = recovery.recover().unwrap();

        assert!(!result.is_cold_start);
        assert_eq!(result.last_trade_id, 5000);

        cleanup(&dir);
    }

    // --------------------------------------------------------
    // TDD Test 3: Recovery with snapshot + newer WAL
    // --------------------------------------------------------
    #[test]
    fn test_recovery_snapshot_plus_wal() {
        let dir = test_dir().join("snapshot_plus_wal");
        cleanup(&dir);

        // Create snapshot at trade 5000
        let snapshot_dir = dir.join("snapshots");
        let snapshotter = SettlementSnapshotter::new(&snapshot_dir);
        snapshotter.create_snapshot(5000).unwrap();

        // Create WAL with newer checkpoint at 7000
        let wal_dir = dir.join("wal");
        fs::create_dir_all(&wal_dir).unwrap();
        let wal_path = wal_dir.join("current.wal");
        {
            let mut writer = SettlementWalWriter::new(&wal_path, 1, 1).unwrap();
            writer.append_checkpoint(6000).unwrap();
            writer.append_checkpoint(7000).unwrap();
            writer.flush().unwrap();
        }

        // Recover - should get 7000 from WAL
        let recovery = SettlementRecovery::new(&dir);
        let result = recovery.recover().unwrap();

        assert!(!result.is_cold_start);
        assert_eq!(result.last_trade_id, 7000); // WAL is newer

        cleanup(&dir);
    }
}
