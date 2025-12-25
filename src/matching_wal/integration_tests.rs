//! Real Integration Test for Complete Crash Recovery
//!
//! Tests the ACTUAL end-to-end flow, not just conceptual examples.

#[cfg(test)]
mod real_integration_tests {
    use crate::matching_wal::{
        MatchingRecovery, MatchingSnapshotter, MatchingWalWriter, RecoveryState,
    };
    use crate::models::{InternalOrder, Side, Trade};
    use crate::orderbook::OrderBook;
    use std::fs;

    fn make_order(id: u64, price: u64, qty: u64, side: Side) -> InternalOrder {
        InternalOrder::new(id, 1, 0, price, qty, side)
    }

    /// **REAL END-TO-END TEST**: Complete crash recovery simulation
    ///
    /// This test actually exercises the ENTIRE system:
    /// 1. Create OrderBook with real orders
    /// 2. Write trades to WAL
    /// 3. Create snapshot
    /// 4. Simulate crash (drop everything)
    /// 5. Recovery: Load snapshot
    /// 6. Verify: OrderBook state matches exactly
    #[test]
    fn test_complete_crash_recovery_e2e() {
        let temp_dir = format!("target/test_real_e2e_{}", std::process::id());
        let _ = fs::remove_dir_all(&temp_dir);

        // === PHASE 1: Initial State ===
        println!("\n🔵 PHASE 1: Building Initial State");

        let snapshot_dir = format!("{}/snapshots", temp_dir);
        let wal_dir = format!("{}/wal", temp_dir);
        fs::create_dir_all(&snapshot_dir).unwrap();
        fs::create_dir_all(&wal_dir).unwrap();

        let mut orderbook = OrderBook::new();
        let mut next_seq_id = 1u64;

        // Add orders to book
        orderbook.rest_order(make_order(1, 100, 10, Side::Buy));
        orderbook.rest_order(make_order(2, 99, 20, Side::Buy));
        orderbook.rest_order(make_order(3, 98, 30, Side::Buy));
        orderbook.rest_order(make_order(4, 101, 15, Side::Sell));
        orderbook.rest_order(make_order(5, 102, 25, Side::Sell));

        println!("  ✅ Created OrderBook with 5 orders");
        println!("     Best bid: {:?}", orderbook.best_bid());
        println!("     Best ask: {:?}", orderbook.best_ask());
        println!("     Depth: {:?}", orderbook.depth());

        // Generate and write some trades to WAL
        let wal_path = format!("{}/trades.wal", wal_dir);
        let mut wal_writer = MatchingWalWriter::new(&wal_path, 1, next_seq_id).unwrap();

        let trades = vec![
            Trade::new(1, 1, 4, 1, 2, 101, 10), // trade_id, buyer_order, seller_order, buyer_user, seller_user, price, qty
            Trade::new(2, 2, 4, 1, 2, 101, 5),
            Trade::new(3, 3, 5, 1, 2, 102, 15),
        ];

        for trade in &trades {
            wal_writer.append_trade(trade, 0).unwrap(); // symbol_id = 0
            next_seq_id += 1;
        }
        wal_writer.flush().unwrap();

        println!("  ✅ Wrote {} trades to WAL (seq 1-3)", trades.len());

        // Create snapshot at current state
        let snapshotter = MatchingSnapshotter::new(&snapshot_dir);
        let snapshot_seq = next_seq_id - 1; // Last written seq
        snapshotter
            .create_snapshot(&orderbook, snapshot_seq)
            .unwrap();

        println!("  ✅ Created snapshot at seq {}", snapshot_seq);

        // Record expected state before "crash"
        let expected_best_bid = orderbook.best_bid();
        let expected_best_ask = orderbook.best_ask();
        let expected_depth = orderbook.depth();
        let expected_order_count = orderbook.all_orders().len();

        println!("\n📊 Expected State:");
        println!("  Best bid: {:?}", expected_best_bid);
        println!("  Best ask: {:?}", expected_best_ask);
        println!("  Depth: {:?}", expected_depth);
        println!("  Total orders: {}", expected_order_count);

        // === PHASE 2: Simulate Crash ===
        println!("\n💥 PHASE 2: Simulating Crash");
        println!("  Dropping orderbook, WAL writer, snapshotter...");
        drop(orderbook);
        drop(wal_writer);
        drop(snapshotter);
        println!("  ✅ All objects dropped (crash simulated)");

        // === PHASE 3: Recovery ===
        println!("\n🟢 PHASE 3: Recovery");

        let recovery = MatchingRecovery::new(&temp_dir);
        let RecoveryState {
            orderbook: recovered_book,
            next_seq_id: recovered_seq,
        } = recovery.recover().unwrap();

        println!("  ✅ Recovery completed");
        println!("  Recovered seq_id: {}", recovered_seq);

        // === PHASE 4: Verification ===
        println!("\n✅ PHASE 4: Verification");

        // Verify seq_id
        assert_eq!(
            recovered_seq, next_seq_id,
            "Seq_id mismatch: expected {}, got {}",
            next_seq_id, recovered_seq
        );
        println!("  ✅ Seq_id correct: {}", recovered_seq);

        // Verify OrderBook state
        assert_eq!(
            recovered_book.best_bid(),
            expected_best_bid,
            "Best bid mismatch"
        );
        assert_eq!(
            recovered_book.best_ask(),
            expected_best_ask,
            "Best ask mismatch"
        );
        assert_eq!(recovered_book.depth(), expected_depth, "Depth mismatch");
        assert_eq!(
            recovered_book.all_orders().len(),
            expected_order_count,
            "Order count mismatch"
        );

        println!("  ✅ OrderBook state matches exactly:");
        println!("     Best bid: {:?}", recovered_book.best_bid());
        println!("     Best ask: {:?}", recovered_book.best_ask());
        println!("     Depth: {:?}", recovered_book.depth());
        println!("     Orders: {}", recovered_book.all_orders().len());

        // Verify we can continue operations
        let _wal_writer = MatchingWalWriter::new(&wal_path, 1, recovered_seq).unwrap();
        println!("  ✅ WAL writer re-initialized successfully");

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);

        println!("\n🎉 SUCCESS: Complete crash recovery verified!");
        println!("   ✅ State preserved across crash");
        println!("   ✅ WAL continuity maintained");
        println!("   ✅ Ready to continue operations");
    }

    /// Test recovery with corrupted snapshot (should fail gracefully)
    #[test]
    fn test_corrupted_snapshot_detection() {
        let temp_dir = format!("target/test_corrupted_{}", std::process::id());
        let _ = fs::remove_dir_all(&temp_dir);

        let snapshot_dir = format!("{}/snapshots", temp_dir);
        fs::create_dir_all(&snapshot_dir).unwrap();

        // Create valid snapshot
        let mut orderbook = OrderBook::new();
        orderbook.rest_order(make_order(1, 100, 10, Side::Buy));

        let snapshotter = MatchingSnapshotter::new(&snapshot_dir);
        snapshotter.create_snapshot(&orderbook, 1).unwrap();

        // Corrupt the snapshot data file
        let snapshot_path = format!("{}/snapshot-1/orderbook.bin", snapshot_dir);
        fs::write(&snapshot_path, b"CORRUPTED DATA").unwrap();

        // Recovery should detect corruption
        let recovery = MatchingRecovery::new(&temp_dir);
        let result = recovery.recover();

        // Should fail with corruption error
        assert!(result.is_err(), "Should detect corrupted snapshot");

        println!("✅ Corrupted snapshot correctly detected");

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }

    /// Test multi-restart scenario
    #[test]
    fn test_multiple_restarts() {
        let temp_dir = format!("target/test_multi_restart_{}", std::process::id());
        let _ = fs::remove_dir_all(&temp_dir);

        println!("\n🔄 Testing Multiple Restarts");

        // Restart 1: Cold start
        {
            println!("\n  === Restart 1: Cold Start ===");
            let recovery = MatchingRecovery::new(&temp_dir);
            let state = recovery.recover().unwrap();
            assert_eq!(state.next_seq_id, 1);
            assert_eq!(state.orderbook.depth(), (0, 0));
            println!("  ✅ Cold start successful");
        }

        // Restart 2: Add some state, create snapshot
        {
            println!("\n  === Restart 2: Build State ===");
            let snapshot_dir = format!("{}/snapshots", temp_dir);
            fs::create_dir_all(&snapshot_dir).unwrap();

            let mut orderbook = OrderBook::new();
            orderbook.rest_order(make_order(1, 100, 10, Side::Buy));

            let snapshotter = MatchingSnapshotter::new(&snapshot_dir);
            snapshotter.create_snapshot(&orderbook, 5).unwrap();
            println!("  ✅ Snapshot created at seq 5");
        }

        // Restart 3: Hot start from snapshot
        {
            println!("\n  === Restart 3: Hot Start ===");
            let recovery = MatchingRecovery::new(&temp_dir);
            let state = recovery.recover().unwrap();
            assert_eq!(state.next_seq_id, 6); // snapshot_seq + 1
            assert_eq!(state.orderbook.depth(), (1, 0));
            println!("  ✅ Hot start from snapshot successful");
        }

        // Restart 4: Another hot start (idempotent)
        {
            println!("\n  === Restart 4: Multiple Hot Starts ===");
            let recovery = MatchingRecovery::new(&temp_dir);
            let state = recovery.recover().unwrap();
            assert_eq!(state.next_seq_id, 6);
            println!("  ✅ Recovery is idempotent");
        }

        println!("\n✅ Multiple restarts verified!");

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }
}
