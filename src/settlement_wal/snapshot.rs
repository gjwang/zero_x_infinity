//! Settlement Service Progress Snapshot
//!
//! Lightweight JSON snapshot that only stores `last_trade_id`.
//! All actual business data is in TDengine.

use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{self, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

// ============================================================
// SNAPSHOT FORMAT
// ============================================================

/// Settlement snapshot - extremely lightweight
///
/// Only stores processing progress (`last_trade_id`).
/// All trades and balance events are in TDengine.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SettlementSnapshot {
    /// Snapshot format version (for future compatibility)
    pub format_version: u32,
    /// Last successfully processed trade_id
    pub last_trade_id: u64,
    /// Snapshot creation timestamp (nanoseconds since UNIX epoch)
    pub created_at_ns: u64,
}

impl SettlementSnapshot {
    /// Current format version
    pub const FORMAT_VERSION: u32 = 1;

    /// Create a new snapshot with current timestamp
    pub fn new(last_trade_id: u64) -> Self {
        let created_at_ns = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);

        Self {
            format_version: Self::FORMAT_VERSION,
            last_trade_id,
            created_at_ns,
        }
    }
}

// ============================================================
// SNAPSHOTTER
// ============================================================

/// Settlement snapshot creator/loader
///
/// # Directory Structure
///
/// ```text
/// snapshots/
/// ├── .tmp-{timestamp}/       # Temporary during creation
/// │   └── metadata.json
/// ├── snapshot-{trade_id}/    # Completed snapshot
/// │   ├── metadata.json
/// │   └── COMPLETE
/// └── latest -> snapshot-{trade_id}/  # Symlink to latest
/// ```
pub struct SettlementSnapshotter {
    snapshot_dir: PathBuf,
}

impl SettlementSnapshotter {
    /// Create a new snapshotter
    pub fn new(dir: impl AsRef<Path>) -> Self {
        Self {
            snapshot_dir: dir.as_ref().to_path_buf(),
        }
    }

    /// Create a snapshot atomically
    ///
    /// # Atomic Creation Process
    ///
    /// 1. Create temp directory `.tmp-{timestamp}`
    /// 2. Write `metadata.json`
    /// 3. Write `COMPLETE` marker
    /// 4. Rename to `snapshot-{trade_id}`
    /// 5. Update `latest` symlink
    ///
    /// # Returns
    ///
    /// Path to the created snapshot directory
    pub fn create_snapshot(&self, last_trade_id: u64) -> io::Result<PathBuf> {
        // Ensure base directory exists
        fs::create_dir_all(&self.snapshot_dir)?;

        // 1. Create temp directory
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let temp_dir = self.snapshot_dir.join(format!(".tmp-{}", timestamp));
        fs::create_dir_all(&temp_dir)?;

        // 2. Write metadata.json
        let snapshot = SettlementSnapshot::new(last_trade_id);
        let metadata_path = temp_dir.join("metadata.json");
        {
            let file = File::create(&metadata_path)?;
            let mut writer = BufWriter::new(file);
            serde_json::to_writer_pretty(&mut writer, &snapshot)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            writer.flush()?;
        }

        // 3. Write COMPLETE marker
        let complete_path = temp_dir.join("COMPLETE");
        fs::write(&complete_path, "")?;

        // 4. Atomic rename to final directory
        let final_dir = self
            .snapshot_dir
            .join(format!("snapshot-{}", last_trade_id));

        // Remove existing if present (shouldn't happen normally)
        let _ = fs::remove_dir_all(&final_dir);
        fs::rename(&temp_dir, &final_dir)?;

        // 5. Update latest symlink
        let latest_path = self.snapshot_dir.join("latest");
        let _ = fs::remove_file(&latest_path); // Remove old symlink

        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            let relative_target = format!("snapshot-{}", last_trade_id);
            symlink(&relative_target, &latest_path)?;
        }

        #[cfg(not(unix))]
        {
            // Windows: write target path to file as fallback
            fs::write(&latest_path, format!("snapshot-{}", last_trade_id))?;
        }

        tracing::info!(
            last_trade_id = last_trade_id,
            path = %final_dir.display(),
            "Settlement snapshot created"
        );

        Ok(final_dir)
    }

    /// Load the latest snapshot
    ///
    /// Returns `None` if no valid snapshot exists (cold start).
    pub fn load_latest(&self) -> io::Result<Option<SettlementSnapshot>> {
        let latest_path = self.snapshot_dir.join("latest");

        // Check if latest exists
        if !latest_path.exists() {
            return Ok(None);
        }

        // Resolve symlink or read fallback file
        let snapshot_dir = if latest_path.is_symlink() {
            fs::read_link(&latest_path)?
        } else {
            // Windows fallback: read target from file
            let target = fs::read_to_string(&latest_path)?;
            PathBuf::from(target.trim())
        };

        // Build full path (symlink target is relative)
        let snapshot_dir = self.snapshot_dir.join(snapshot_dir);

        // Verify COMPLETE marker exists
        let complete_path = snapshot_dir.join("COMPLETE");
        if !complete_path.exists() {
            tracing::warn!(
                path = %snapshot_dir.display(),
                "Snapshot incomplete (missing COMPLETE marker)"
            );
            return Ok(None);
        }

        // Load metadata.json
        let metadata_path = snapshot_dir.join("metadata.json");
        let file = File::open(&metadata_path)?;
        let reader = BufReader::new(file);
        let snapshot: SettlementSnapshot = serde_json::from_reader(reader)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        tracing::info!(
            last_trade_id = snapshot.last_trade_id,
            "Settlement snapshot loaded"
        );

        Ok(Some(snapshot))
    }
}

// ============================================================
// UNIT TESTS (TDD - Test First!)
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn test_dir() -> PathBuf {
        PathBuf::from(format!(
            "target/test_settlement_snapshot_{}",
            std::process::id()
        ))
    }

    fn cleanup(dir: &Path) {
        let _ = fs::remove_dir_all(dir);
    }

    // --------------------------------------------------------
    // TDD Test 1: Create and load snapshot
    // --------------------------------------------------------
    #[test]
    fn test_snapshot_create_load() {
        let dir = test_dir().join("create_load");
        cleanup(&dir);

        let snapshotter = SettlementSnapshotter::new(&dir);

        // Create snapshot
        let path = snapshotter.create_snapshot(5000).unwrap();
        assert!(path.exists());
        assert!(path.join("metadata.json").exists());
        assert!(path.join("COMPLETE").exists());

        // Load snapshot
        let loaded = snapshotter.load_latest().unwrap();
        assert!(loaded.is_some());
        let snapshot = loaded.unwrap();
        assert_eq!(snapshot.last_trade_id, 5000);
        assert_eq!(snapshot.format_version, SettlementSnapshot::FORMAT_VERSION);

        cleanup(&dir);
    }

    // --------------------------------------------------------
    // TDD Test 2: Cold start (no snapshot)
    // --------------------------------------------------------
    #[test]
    fn test_snapshot_cold_start() {
        let dir = test_dir().join("cold_start");
        cleanup(&dir);

        let snapshotter = SettlementSnapshotter::new(&dir);

        // No snapshot exists
        let loaded = snapshotter.load_latest().unwrap();
        assert!(loaded.is_none());

        cleanup(&dir);
    }

    // --------------------------------------------------------
    // TDD Test 3: Atomic creation (incomplete snapshot ignored)
    // --------------------------------------------------------
    #[test]
    fn test_snapshot_atomic_creation() {
        let dir = test_dir().join("atomic");
        cleanup(&dir);

        // Simulate interrupted snapshot (no COMPLETE marker)
        let incomplete_dir = dir.join("snapshot-1000");
        fs::create_dir_all(&incomplete_dir).unwrap();
        fs::write(
            incomplete_dir.join("metadata.json"),
            r#"{"format_version":1,"last_trade_id":1000,"created_at_ns":0}"#,
        )
        .unwrap();
        // Note: NO COMPLETE marker

        // Create latest symlink pointing to incomplete
        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            symlink("snapshot-1000", dir.join("latest")).unwrap();
        }
        #[cfg(not(unix))]
        {
            fs::write(dir.join("latest"), "snapshot-1000").unwrap();
        }

        let snapshotter = SettlementSnapshotter::new(&dir);

        // Should return None because snapshot is incomplete
        let loaded = snapshotter.load_latest().unwrap();
        assert!(loaded.is_none());

        cleanup(&dir);
    }
}
