//! UBSCore Recovery Logic
//!
//! Handles cold/hot start recovery from Snapshot + WAL

use super::snapshot::UBSCoreSnapshotter;
use super::wal::{CancelPayload, OrderPayload, UBSCoreWalReader};
use crate::models::Side;
use crate::symbol_manager::SymbolManager;
use crate::user_account::UserAccount;
use crate::wal_v2::WalEntryType;
use rustc_hash::FxHashMap;
use std::io;
use std::path::{Path, PathBuf};

// ============================================================
// Recovery State
// ============================================================

pub struct RecoveryState {
    pub accounts: FxHashMap<u64, UserAccount>,
    pub next_seq_id: u64,
}

// ============================================================
// UBSCore Recovery
// ============================================================

pub struct UBSCoreRecovery {
    data_dir: PathBuf,
}

impl UBSCoreRecovery {
    /// Create a new recovery instance
    pub fn new(data_dir: impl AsRef<Path>) -> Self {
        Self {
            data_dir: data_dir.as_ref().to_path_buf(),
        }
    }

    /// Recover UBSCore state from Snapshot + WAL
    ///
    /// Recovery flow:
    /// 1. Check for latest snapshot
    /// 2. If exists: load snapshot → get snapshot_seq
    /// 3. If not: cold start with empty state, snapshot_seq = 0
    /// 4. Replay WAL from snapshot_seq + 1
    /// 5. Return recovered state
    pub fn recover(&self, manager: &SymbolManager) -> io::Result<RecoveryState> {
        let snapshot_dir = self.data_dir.join("snapshots");
        let wal_dir = self.data_dir.join("wal");

        let snapshotter = UBSCoreSnapshotter::new(&snapshot_dir);

        // Step 1-3: Load snapshot or cold start
        let (mut accounts, snapshot_seq) = match snapshotter.load_latest_snapshot()? {
            Some((metadata, loaded_accounts)) => {
                tracing::info!(
                    seq_id = metadata.wal_seq_id,
                    user_count = metadata.user_count,
                    "Loaded snapshot"
                );
                (loaded_accounts, metadata.wal_seq_id)
            }
            None => {
                tracing::info!("No snapshot found, cold start");
                (FxHashMap::default(), 0)
            }
        };

        // Step 4: Replay WAL from snapshot_seq + 1
        let mut next_seq_id = snapshot_seq + 1;

        let wal_file = wal_dir.join("current.wal");
        if wal_file.exists() {
            let mut reader = UBSCoreWalReader::open(&wal_file)?;

            reader.replay(next_seq_id, |entry| {
                // Replay entry to update accounts
                match WalEntryType::try_from(entry.header.entry_type) {
                    Ok(WalEntryType::Order) => {
                        // Deserialize and apply lock
                        if let Ok(payload) = bincode::deserialize::<OrderPayload>(&entry.payload) {
                            let symbol_info = manager.get_symbol_info_by_id(payload.symbol_id);
                            let qty_unit = symbol_info.map(|s| s.qty_unit()).unwrap_or(100_000_000);

                            // Calculate lock asset and amount
                            let side = Side::try_from(payload.side).unwrap_or(Side::Buy);
                            let lock_asset = match (side, symbol_info) {
                                (Side::Buy, Some(s)) => s.quote_asset_id,
                                (Side::Sell, Some(s)) => s.base_asset_id,
                                _ => 0,
                            };

                            // Re-calculate cost (lock amount)
                            let lock_amount = if payload.order_type == 0 {
                                // Limit
                                payload.price * payload.qty / qty_unit
                            } else {
                                0 // Market orders handled by UBSCore business logic
                            };

                            if lock_amount > 0 && lock_asset > 0 {
                                let account = accounts
                                    .entry(payload.user_id)
                                    .or_insert_with(|| UserAccount::new(payload.user_id));
                                if let Ok(balance) = account.get_balance_mut(lock_asset) {
                                    let _ = balance.lock(lock_amount);
                                }
                            }
                        }
                        next_seq_id = entry.header.seq_id + 1;
                    }
                    Ok(WalEntryType::Cancel) => {
                        // NOTE: Cancellations often involve unlocking, but since we don't know
                        // the original unlock_amount from the WAL entry (it only has order_id),
                        // this confirms the need for Phase 4 Replay Protocol where ME tells USBC.
                        // For RECOVERY however, we usually only recover to the point of durable LOCKS.
                        // ME will re-request cancels upon its own recovery.
                        next_seq_id = entry.header.seq_id + 1;
                    }
                    Ok(WalEntryType::Deposit) => {
                        // Deserialize and apply deposit
                        if let Ok(payload) =
                            bincode::deserialize::<crate::wal_v2::FundingPayload>(&entry.payload)
                        {
                            let account = accounts
                                .entry(payload.user_id)
                                .or_insert_with(|| UserAccount::new(payload.user_id));
                            let _ = account.deposit(payload.asset_id, payload.amount);
                        }
                        next_seq_id = entry.header.seq_id + 1;
                    }
                    Ok(WalEntryType::Withdraw) => {
                        if let Ok(payload) =
                            bincode::deserialize::<crate::wal_v2::FundingPayload>(&entry.payload)
                        {
                            let account = accounts
                                .entry(payload.user_id)
                                .or_insert_with(|| UserAccount::new(payload.user_id));
                            if let Ok(balance) = account.get_balance_mut(payload.asset_id) {
                                let _ = balance.withdraw(payload.amount);
                            }
                        }
                        next_seq_id = entry.header.seq_id + 1;
                    }
                    _ => {
                        // Unknown type, skip
                        next_seq_id = entry.header.seq_id + 1;
                    }
                }

                Ok(true) // Continue replay
            })?;

            tracing::info!(
                from_seq = snapshot_seq + 1,
                to_seq = next_seq_id - 1,
                "Replayed WAL"
            );
        } else {
            tracing::info!("No WAL file found");
        }

        Ok(RecoveryState {
            accounts,
            next_seq_id,
        })
    }
}

// ============================================================
// Unit Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ubscore_wal::{UBSCoreSnapshotter, UBSCoreWalWriter};
    use std::fs;
    use std::io::Write;

    // --------------------------------------------------------
    // TDD Test 1: Cold start recovery (no snapshot, no WAL)
    // --------------------------------------------------------
    #[test]
    fn test_cold_start_recovery() {
        let temp_dir = format!("target/test_recovery_cold_{}", std::process::id());
        let _ = fs::remove_dir_all(&temp_dir);

        let recovery = UBSCoreRecovery::new(&temp_dir);
        let manager = SymbolManager::new();
        let state = recovery.recover(&manager).unwrap();

        assert_eq!(state.accounts.len(), 0);
        assert_eq!(state.next_seq_id, 1);

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }

    // --------------------------------------------------------
    // TDD Test 2: Hot start recovery (snapshot only)
    // --------------------------------------------------------
    #[test]
    fn test_hot_start_snapshot_only() {
        let temp_dir = format!("target/test_recovery_hot_{}", std::process::id());
        let _ = fs::remove_dir_all(&temp_dir);

        // Create a snapshot
        {
            let snapshot_dir = PathBuf::from(&temp_dir).join("snapshots");
            let snapshotter = UBSCoreSnapshotter::new(&snapshot_dir);

            let mut accounts = FxHashMap::default();
            for user_id in 1..=5 {
                let mut account = UserAccount::new(user_id);
                account.deposit(0, 1000_000000).unwrap();
                accounts.insert(user_id, account);
            }

            snapshotter.create_snapshot(&accounts, 100).unwrap();
        }

        // Recover
        let recovery = UBSCoreRecovery::new(&temp_dir);
        let manager = SymbolManager::new();
        let state = recovery.recover(&manager).unwrap();

        assert_eq!(state.accounts.len(), 5);
        assert_eq!(state.next_seq_id, 101); // snapshot_seq + 1

        // Verify account balances
        for user_id in 1..=5 {
            let account = &state.accounts[&user_id];
            let balance = account.get_balance(0).unwrap();
            assert_eq!(balance.avail(), 1000_000000);
        }

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }

    // --------------------------------------------------------
    // TDD Test 3: Hot start with WAL replay
    // --------------------------------------------------------
    #[test]
    fn test_hot_start_with_wal_replay() {
        let temp_dir = format!("target/test_recovery_wal_{}", std::process::id());
        let _ = fs::remove_dir_all(&temp_dir);

        // Create snapshot
        {
            let snapshot_dir = PathBuf::from(&temp_dir).join("snapshots");
            let snapshotter = UBSCoreSnapshotter::new(&snapshot_dir);

            let mut accounts = FxHashMap::default();
            let mut account = UserAccount::new(1);
            account.deposit(0, 1000_000000).unwrap();
            accounts.insert(1, account);

            snapshotter.create_snapshot(&accounts, 10).unwrap();
        }

        // Write WAL entries after snapshot
        {
            let wal_dir = PathBuf::from(&temp_dir).join("wal");
            fs::create_dir_all(&wal_dir).unwrap();
            let wal_file = wal_dir.join("current.wal");

            let mut writer = UBSCoreWalWriter::new(&wal_file, 1, 11).unwrap();

            // Deposit 500 (seq 11)
            writer.append_deposit(1, 0, 500_000000, 1).unwrap();
            // Deposit 300 (seq 12)
            writer.append_deposit(1, 0, 300_000000, 2).unwrap();

            writer.flush().unwrap();
        }

        // Recover
        let recovery = UBSCoreRecovery::new(&temp_dir);
        let mut manager = SymbolManager::new();
        // Asset 0 is needed for the test
        manager.add_asset(0, 8, 6, "BTC");
        let state = recovery.recover(&manager).unwrap();

        assert_eq!(state.next_seq_id, 13); // last WAL seq + 1

        // Verify balance: 1000 (snapshot) + 500 + 300 = 1800
        let account = &state.accounts[&1];
        let balance = account.get_balance(0).unwrap();
        assert_eq!(balance.avail(), 1800_000000);

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }

    // --------------------------------------------------------
    // TDD Test 4: Corrupted WAL detection
    // --------------------------------------------------------
    #[test]
    fn test_corrupted_wal_detection() {
        let temp_dir = format!("target/test_recovery_corrupt_{}", std::process::id());
        let _ = fs::remove_dir_all(&temp_dir);

        // Create snapshot
        {
            let snapshot_dir = PathBuf::from(&temp_dir).join("snapshots");
            let snapshotter = UBSCoreSnapshotter::new(&snapshot_dir);

            let mut accounts = FxHashMap::default();
            let mut account = UserAccount::new(1);
            account.deposit(0, 1000_000000).unwrap();
            accounts.insert(1, account);

            snapshotter.create_snapshot(&accounts, 10).unwrap();
        }

        // Create corrupted WAL (write good header but corrupted payload)
        {
            let wal_dir = PathBuf::from(&temp_dir).join("wal");
            fs::create_dir_all(&wal_dir).unwrap();
            let wal_file = wal_dir.join("current.wal");

            // Write a proper WAL entry first
            let mut writer = UBSCoreWalWriter::new(&wal_file, 1, 11).unwrap();
            writer.append_deposit(1, 0, 500_000000, 1).unwrap();
            writer.flush().unwrap();

            // Now corrupt the payload (keep header intact but corrupt data)
            use std::io::{Seek, SeekFrom};
            let mut file = fs::OpenOptions::new().write(true).open(&wal_file).unwrap();
            file.seek(SeekFrom::Start(20)).unwrap(); // Skip header
            file.write_all(b"CORRUPTED").unwrap();
        }

        // Recover - should detect corruption
        let recovery = UBSCoreRecovery::new(&temp_dir);
        let manager = SymbolManager::new();
        let result = recovery.recover(&manager);

        // Should fail with checksum error
        assert!(result.is_err());

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }

    // --------------------------------------------------------
    // TDD Test 5: Empty WAL file
    // --------------------------------------------------------
    #[test]
    fn test_empty_wal_file() {
        let temp_dir = format!("target/test_recovery_empty_{}", std::process::id());
        let _ = fs::remove_dir_all(&temp_dir);

        // Create snapshot
        {
            let snapshot_dir = PathBuf::from(&temp_dir).join("snapshots");
            let snapshotter = UBSCoreSnapshotter::new(&snapshot_dir);

            let mut accounts = FxHashMap::default();
            let mut account = UserAccount::new(1);
            account.deposit(0, 1000_000000).unwrap();
            accounts.insert(1, account);

            snapshotter.create_snapshot(&accounts, 10).unwrap();
        }

        // Create empty WAL file
        {
            let wal_dir = PathBuf::from(&temp_dir).join("wal");
            fs::create_dir_all(&wal_dir).unwrap();
            let wal_file = wal_dir.join("current.wal");
            fs::write(&wal_file, b"").unwrap();
        }

        // Recover - should succeed with snapshot data only
        let recovery = UBSCoreRecovery::new(&temp_dir);
        let manager = SymbolManager::new();
        let state = recovery.recover(&manager).unwrap();

        assert_eq!(state.next_seq_id, 11);
        assert_eq!(state.accounts.len(), 1);

        let account = &state.accounts[&1];
        let balance = account.get_balance(0).unwrap();
        assert_eq!(balance.avail(), 1000_000000);

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }

    // --------------------------------------------------------
    // TDD Test 6: Multiple users recovery
    // --------------------------------------------------------
    #[test]
    fn test_multiple_users_recovery() {
        let temp_dir = format!("target/test_recovery_multi_{}", std::process::id());
        let _ = fs::remove_dir_all(&temp_dir);

        // Create snapshot with users 1-3
        {
            let snapshot_dir = PathBuf::from(&temp_dir).join("snapshots");
            let snapshotter = UBSCoreSnapshotter::new(&snapshot_dir);

            let mut accounts = FxHashMap::default();
            for user_id in 1..=3 {
                let mut account = UserAccount::new(user_id);
                account.deposit(0, 1000_000000).unwrap();
                accounts.insert(user_id, account);
            }

            snapshotter.create_snapshot(&accounts, 10).unwrap();
        }

        // Add WAL entries for users 4-5
        {
            let wal_dir = PathBuf::from(&temp_dir).join("wal");
            fs::create_dir_all(&wal_dir).unwrap();
            let wal_file = wal_dir.join("current.wal");

            let mut writer = UBSCoreWalWriter::new(&wal_file, 1, 11).unwrap();
            writer.append_deposit(4, 0, 2000_000000, 1).unwrap();
            writer.append_deposit(5, 0, 3000_000000, 2).unwrap();
            writer.flush().unwrap();
        }

        // Recover
        let recovery = UBSCoreRecovery::new(&temp_dir);
        let mut manager = SymbolManager::new();
        // Asset 0 is needed for the test
        manager.add_asset(0, 8, 6, "BTC");
        let state = recovery.recover(&manager).unwrap();

        assert_eq!(state.accounts.len(), 5); // 3 from snapshot + 2 from WAL

        // Verify users 1-3 (from snapshot)
        for user_id in 1..=3 {
            let balance = state.accounts[&user_id].get_balance(0).unwrap();
            assert_eq!(balance.avail(), 1000_000000);
        }

        // Verify users 4-5 (from WAL)
        assert_eq!(
            state.accounts[&4].get_balance(0).unwrap().avail(),
            2000_000000
        );
        assert_eq!(
            state.accounts[&5].get_balance(0).unwrap().avail(),
            3000_000000
        );

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }
}
