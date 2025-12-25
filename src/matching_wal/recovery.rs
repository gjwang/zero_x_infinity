//! Matching Service Recovery Logic
//!
//! Handles cold/hot start recovery from OrderBook Snapshot
//!
//! ## Key Design: No Trade Replay Needed
//!
//! Unlike UBSCore (which replays Deposit/Withdraw to rebuild balances),
//! Matching Service recovery is simpler:
//! - OrderBook state comes entirely from snapshot (resting orders)
//! - Trades are audit logs (for settlement), not OrderBook mutations
//! - Recovery = Load snapshot (or empty OrderBook if cold start)

use super::snapshot::MatchingSnapshotter;
use crate::orderbook::OrderBook;
use std::io;
use std::path::{Path, PathBuf};

// ============================================================
// Recovery State
// ============================================================

pub struct RecoveryState {
    pub orderbook: OrderBook,
    pub next_seq_id: u64, // For Trade WAL continuation
}

// ============================================================
// Matching Recovery
// ============================================================

pub struct MatchingRecovery {
    data_dir: PathBuf,
}

impl MatchingRecovery {
    /// Create a new recovery instance
    pub fn new(data_dir: impl AsRef<Path>) -> Self {
        Self {
            data_dir: data_dir.as_ref().to_path_buf(),
        }
    }

    /// Recover OrderBook state from Snapshot
    ///
    /// Recovery flow:
    /// 1. Check for latest snapshot
    /// 2. If exists: load snapshot → get snapshot_seq, orderbook
    /// 3. If not: cold start with empty OrderBook, snapshot_seq = 0
    /// 4. Return RecoveryState with next_seq_id = snapshot_seq + 1
    ///
    /// Note: No Trade WAL replay needed! Trades don't modify OrderBook.
    pub fn recover(&self) -> io::Result<RecoveryState> {
        let snapshot_dir = self.data_dir.join("snapshots");
        let snapshotter = MatchingSnapshotter::new(&snapshot_dir);

        // Load snapshot or cold start
        let (orderbook, snapshot_seq) = match snapshotter.load_latest_snapshot()? {
            Some((metadata, loaded_orderbook)) => {
                tracing::info!(
                    seq_id = metadata.wal_seq_id,
                    order_count = metadata.order_count,
                    "Loaded OrderBook snapshot"
                );
                (loaded_orderbook, metadata.wal_seq_id)
            }
            None => {
                tracing::info!("No snapshot found, cold start with empty OrderBook");
                (OrderBook::new(), 0)
            }
        };

        Ok(RecoveryState {
            orderbook,
            next_seq_id: snapshot_seq + 1,
        })
    }
}

// ============================================================
// Unit Tests (TDD - Test First!)
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::matching_wal::MatchingSnapshotter;
    use crate::models::{InternalOrder, Side};
    use std::fs;

    fn make_order(id: u64, price: u64, qty: u64, side: Side) -> InternalOrder {
        InternalOrder::new(id, 1, 0, price, qty, side)
    }

    // --------------------------------------------------------
    // TDD Test 1: Cold start recovery (no snapshot)
    // --------------------------------------------------------
    #[test]
    fn test_cold_start_no_snapshot() {
        let temp_dir = format!("target/test_matching_recovery_cold_{}", std::process::id());
        let _ = fs::remove_dir_all(&temp_dir);

        let recovery = MatchingRecovery::new(&temp_dir);
        let state = recovery.recover().unwrap();

        // Empty OrderBook
        assert_eq!(state.orderbook.depth(), (0, 0));
        assert_eq!(state.orderbook.best_bid(), None);
        assert_eq!(state.orderbook.best_ask(), None);

        // next_seq_id = 1 (start from beginning)
        assert_eq!(state.next_seq_id, 1);

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }

    // --------------------------------------------------------
    // TDD Test 2: Hot start with snapshot
    // --------------------------------------------------------
    #[test]
    fn test_hot_start_with_snapshot() {
        let temp_dir = format!("target/test_matching_recovery_hot_{}", std::process::id());
        let _ = fs::remove_dir_all(&temp_dir);

        // Create snapshot with 4 orders
        {
            let snapshot_dir = PathBuf::from(&temp_dir).join("snapshots");
            let snapshotter = MatchingSnapshotter::new(&snapshot_dir);

            let mut orderbook = OrderBook::new();
            orderbook.rest_order(make_order(1, 100, 10, Side::Buy));
            orderbook.rest_order(make_order(2, 99, 20, Side::Buy));
            orderbook.rest_order(make_order(3, 101, 15, Side::Sell));
            orderbook.rest_order(make_order(4, 102, 25, Side::Sell));

            snapshotter.create_snapshot(&orderbook, 100).unwrap();
        }

        // Recover
        let recovery = MatchingRecovery::new(&temp_dir);
        let state = recovery.recover().unwrap();

        // Verify OrderBook restored
        assert_eq!(state.orderbook.best_bid(), Some(100));
        assert_eq!(state.orderbook.best_ask(), Some(101));
        assert_eq!(state.orderbook.depth(), (2, 2));
        assert_eq!(state.orderbook.all_orders().len(), 4);

        // next_seq_id = snapshot_seq + 1 = 101
        assert_eq!(state.next_seq_id, 101);

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }

    // --------------------------------------------------------
    // TDD Test 3: Snapshot seq_id sets next_seq correctly
    // --------------------------------------------------------
    #[test]
    fn test_snapshot_sets_next_seq() {
        let temp_dir = format!("target/test_matching_recovery_seq_{}", std::process::id());
        let _ = fs::remove_dir_all(&temp_dir);

        // Create snapshot at seq 12345
        {
            let snapshot_dir = PathBuf::from(&temp_dir).join("snapshots");
            let snapshotter = MatchingSnapshotter::new(&snapshot_dir);

            let mut orderbook = OrderBook::new();
            orderbook.rest_order(make_order(1, 100, 10, Side::Buy));

            snapshotter.create_snapshot(&orderbook, 12345).unwrap();
        }

        // Recover
        let recovery = MatchingRecovery::new(&temp_dir);
        let state = recovery.recover().unwrap();

        // next_seq_id should be 12346 (for next trade)
        assert_eq!(state.next_seq_id, 12346);

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }
}
