//! UBSCore Snapshot Creation/Loading
//!
//! Atomic snapshot creation with COMPLETE marker and checksum verification.

use crate::user_account::UserAccount;
use chrono::{DateTime, Utc};
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{self, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

// ============================================================
// Snapshot Metadata
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotMetadata {
    pub format_version: u32,
    pub wal_seq_id: u64,
    pub user_count: usize,
    pub accounts_checksum: String,
    pub created_at: DateTime<Utc>,
}

// ============================================================
// UBSCore Snapshotter
// ============================================================

pub struct UBSCoreSnapshotter {
    snapshot_dir: PathBuf,
}

impl UBSCoreSnapshotter {
    /// Create a new snapshotter
    pub fn new(snapshot_dir: impl AsRef<Path>) -> Self {
        Self {
            snapshot_dir: snapshot_dir.as_ref().to_path_buf(),
        }
    }

    /// Create an atomic snapshot
    ///
    /// Protocol:
    /// 1. Create .tmp-{timestamp}/
    /// 2. Write accounts.bin (bincode)
    /// 3. Calculate CRC64 checksum
    /// 4. Write metadata.json
    /// 5. Write COMPLETE marker
    /// 6. Atomic rename to snapshot-{seq}/
    /// 7. Update latest symlink
    pub fn create_snapshot(
        &self,
        accounts: &FxHashMap<u64, UserAccount>,
        wal_seq_id: u64,
    ) -> io::Result<PathBuf> {
        // Ensure snapshot directory exists
        fs::create_dir_all(&self.snapshot_dir)?;

        // 1. Create temporary directory
        let timestamp = Utc::now().timestamp_millis();
        let tmp_dir = self.snapshot_dir.join(format!(".tmp-{}", timestamp));
        fs::create_dir_all(&tmp_dir)?;

        // 2. Serialize accounts to accounts.bin
        let accounts_path = tmp_dir.join("accounts.bin");
        let accounts_bytes = bincode::serialize(accounts)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        {
            let file = File::create(&accounts_path)?;
            let mut writer = BufWriter::new(file);
            writer.write_all(&accounts_bytes)?;
            writer.flush()?;
        }

        // 3. Calculate CRC64 checksum
        let checksum = calculate_crc64(&accounts_bytes);

        // 4. Write metadata.json
        let metadata = SnapshotMetadata {
            format_version: 1,
            wal_seq_id,
            user_count: accounts.len(),
            accounts_checksum: checksum.clone(),
            created_at: Utc::now(),
        };

        let metadata_path = tmp_dir.join("metadata.json");
        let metadata_json = serde_json::to_string_pretty(&metadata)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        fs::write(&metadata_path, metadata_json)?;

        // 5. Write COMPLETE marker
        let complete_path = tmp_dir.join("COMPLETE");
        fs::write(&complete_path, "")?;

        // 6. Atomic rename
        let snapshot_dir = self.snapshot_dir.join(format!("snapshot-{}", wal_seq_id));
        if snapshot_dir.exists() {
            fs::remove_dir_all(&snapshot_dir)?;
        }
        fs::rename(&tmp_dir, &snapshot_dir)?;

        // 7. Update latest symlink
        let latest_link = self.snapshot_dir.join("latest");
        if latest_link.exists() {
            fs::remove_file(&latest_link)?;
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            symlink(format!("snapshot-{}", wal_seq_id), &latest_link)?;
        }

        #[cfg(windows)]
        {
            use std::os::windows::fs::symlink_dir;
            symlink_dir(format!("snapshot-{}", wal_seq_id), &latest_link)?;
        }

        Ok(snapshot_dir)
    }

    /// Load the latest snapshot
    pub fn load_latest_snapshot(
        &self,
    ) -> io::Result<Option<(SnapshotMetadata, FxHashMap<u64, UserAccount>)>> {
        let latest_link = self.snapshot_dir.join("latest");

        if !latest_link.exists() {
            return Ok(None);
        }

        // Check COMPLETE marker
        let complete_path = latest_link.join("COMPLETE");
        if !complete_path.exists() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Incomplete snapshot (missing COMPLETE marker)",
            ));
        }

        // Load metadata
        let metadata_path = latest_link.join("metadata.json");
        let metadata_json = fs::read_to_string(&metadata_path)?;
        let metadata: SnapshotMetadata = serde_json::from_str(&metadata_json)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        // Load accounts.bin
        let accounts_path = latest_link.join("accounts.bin");
        let mut file = File::open(&accounts_path)?;
        let mut accounts_bytes = Vec::new();
        file.read_to_end(&mut accounts_bytes)?;

        // Verify checksum
        let calculated_checksum = calculate_crc64(&accounts_bytes);
        if calculated_checksum != metadata.accounts_checksum {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Checksum mismatch: expected {}, got {}",
                    metadata.accounts_checksum, calculated_checksum
                ),
            ));
        }

        // Deserialize accounts
        let accounts: FxHashMap<u64, UserAccount> = bincode::deserialize(&accounts_bytes)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        Ok(Some((metadata, accounts)))
    }
}

// ============================================================
// CRC64 Checksum
// ============================================================

fn calculate_crc64(data: &[u8]) -> String {
    use crc::{CRC_64_ECMA_182, Crc};

    const CRC64: Crc<u64> = Crc::<u64>::new(&CRC_64_ECMA_182);
    let checksum = CRC64.checksum(data);
    format!("{:016x}", checksum)
}

// ============================================================
// Unit Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Balance;

    fn create_test_accounts() -> FxHashMap<u64, UserAccount> {
        let mut accounts = FxHashMap::default();

        for user_id in 1..=10 {
            let mut account = UserAccount::new(user_id);
            // Add some test balances using public API
            account.deposit(0, 1000_000000).unwrap();
            account.deposit(1, 500_000000).unwrap();
            accounts.insert(user_id, account);
        }

        accounts
    }

    // --------------------------------------------------------
    // TDD Test 1: Create snapshot atomically
    // --------------------------------------------------------
    #[test]
    fn test_create_snapshot_atomic() {
        let temp_dir = format!("target/test_snapshot_{}", std::process::id());
        let _ = fs::remove_dir_all(&temp_dir);

        let snapshotter = UBSCoreSnapshotter::new(&temp_dir);
        let accounts = create_test_accounts();

        let snapshot_path = snapshotter.create_snapshot(&accounts, 12345).unwrap();

        // Verify directory structure
        assert!(snapshot_path.join("metadata.json").exists());
        assert!(snapshot_path.join("accounts.bin").exists());
        assert!(snapshot_path.join("COMPLETE").exists());

        // Verify latest symlink
        let latest_link = PathBuf::from(&temp_dir).join("latest");
        assert!(latest_link.exists());

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }

    // --------------------------------------------------------
    // TDD Test 2: Load snapshot validates checksum
    // --------------------------------------------------------
    #[test]
    fn test_load_snapshot_validates_checksum() {
        let temp_dir = format!("target/test_snapshot_load_{}", std::process::id());
        let _ = fs::remove_dir_all(&temp_dir);

        let snapshotter = UBSCoreSnapshotter::new(&temp_dir);
        let accounts = create_test_accounts();

        // Create snapshot
        snapshotter.create_snapshot(&accounts, 12345).unwrap();

        // Load snapshot
        let loaded = snapshotter.load_latest_snapshot().unwrap();
        assert!(loaded.is_some());

        let (metadata, loaded_accounts) = loaded.unwrap();
        assert_eq!(metadata.wal_seq_id, 12345);
        assert_eq!(metadata.user_count, 10);
        assert_eq!(loaded_accounts.len(), 10);

        // Verify account data
        for user_id in 1..=10 {
            assert!(loaded_accounts.contains_key(&user_id));
            let account = &loaded_accounts[&user_id];
            let balance = account.get_balance(0).unwrap();
            assert_eq!(balance.avail(), 1000_000000);
        }

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }

    // --------------------------------------------------------
    // TDD Test 3: Incomplete snapshot ignored
    // --------------------------------------------------------
    #[test]
    fn test_incomplete_snapshot_ignored() {
        let temp_dir = format!("target/test_snapshot_incomplete_{}", std::process::id());
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();

        let snapshotter = UBSCoreSnapshotter::new(&temp_dir);

        // Create incomplete snapshot (no COMPLETE marker)
        let incomplete_dir = PathBuf::from(&temp_dir).join("snapshot-999");
        fs::create_dir_all(&incomplete_dir).unwrap();
        fs::write(incomplete_dir.join("metadata.json"), "{}").unwrap();
        fs::write(incomplete_dir.join("accounts.bin"), "").unwrap();

        // Create latest symlink pointing to incomplete snapshot
        let latest_link = PathBuf::from(&temp_dir).join("latest");
        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            symlink("snapshot-999", &latest_link).unwrap();
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::symlink_dir;
            symlink_dir("snapshot-999", &latest_link).unwrap();
        }

        // Try to load - should error
        let result = snapshotter.load_latest_snapshot();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("COMPLETE"));

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }

    // --------------------------------------------------------
    // TDD Test 4: Checksum corruption detection
    // --------------------------------------------------------
    #[test]
    fn test_checksum_corruption_detection() {
        let temp_dir = format!("target/test_snapshot_corrupt_{}", std::process::id());
        let _ = fs::remove_dir_all(&temp_dir);

        let snapshotter = UBSCoreSnapshotter::new(&temp_dir);
        let accounts = create_test_accounts();

        // Create snapshot
        snapshotter.create_snapshot(&accounts, 12345).unwrap();

        // Corrupt the accounts.bin file
        let accounts_path = PathBuf::from(&temp_dir).join("latest/accounts.bin");
        let mut file = fs::OpenOptions::new()
            .write(true)
            .open(&accounts_path)
            .unwrap();
        file.write_all(b"CORRUPTED_DATA").unwrap();

        // Try to load - should error on checksum mismatch
        let result = snapshotter.load_latest_snapshot();
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Checksum mismatch")
        );

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }
}
