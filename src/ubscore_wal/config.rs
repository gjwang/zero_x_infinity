//! UBSCore Configuration

use std::path::PathBuf;
use std::time::Duration;

/// Configuration for UBSCore WAL + Snapshot
#[derive(Debug, Clone)]
pub struct UBSCoreConfig {
    /// Data directory root (data/ubscore-service/)
    pub data_dir: PathBuf,
    /// WAL directory (data/usbcore-service/wal/)
    pub wal_dir: PathBuf,
    /// Snapshot directory (data/ubscore-service/snapshots/)
    pub snapshot_dir: PathBuf,

    // WAL Settings
    /// Maximum WAL file size before rotation (default: 256 MB)
    pub wal_max_file_size: u64,
    /// WAL flush interval (default: 100 ms)
    pub wal_flush_interval_ms: u64,

    // Snapshot Settings
    /// Snapshot creation interval (default: 10 minutes)
    pub snapshot_interval: Duration,
    /// Event threshold for snapshot trigger (default: 100,000 events)
    pub snapshot_event_threshold: u64,
    /// Number of snapshots to keep (default: 3)
    pub snapshot_keep_count: usize,
}

impl UBSCoreConfig {
    /// Create default configuration with given data directory
    pub fn new(data_dir: impl Into<PathBuf>) -> Self {
        let data_dir = data_dir.into();
        Self {
            wal_dir: data_dir.join("wal"),
            snapshot_dir: data_dir.join("snapshots"),
            data_dir,
            wal_max_file_size: 256 * 1024 * 1024, // 256 MB
            wal_flush_interval_ms: 100,           // 100 ms
            snapshot_interval: Duration::from_secs(10 * 60), // 10 minutes
            snapshot_event_threshold: 100_000,    // 100K events
            snapshot_keep_count: 3,               // Keep 3 snapshots
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ubscore_config_defaults() {
        let config = UBSCoreConfig::new("data/ubscore-service");

        assert_eq!(config.data_dir, PathBuf::from("data/ubscore-service"));
        assert_eq!(config.wal_dir, PathBuf::from("data/ubscore-service/wal"));
        assert_eq!(
            config.snapshot_dir,
            PathBuf::from("data/ubscore-service/snapshots")
        );
        assert_eq!(config.wal_max_file_size, 256 * 1024 * 1024);
        assert_eq!(config.wal_flush_interval_ms, 100);
        assert_eq!(config.snapshot_interval, Duration::from_secs(600));
        assert_eq!(config.snapshot_event_threshold, 100_000);
        assert_eq!(config.snapshot_keep_count, 3);
    }
}
