//! UBSCore WAL Writer/Reader
//!
//! Business-layer WAL operations for UBSCore, built on top of wal_v2.

use crate::models::InternalOrder;
use crate::wal_v2::{WalEntry, WalEntryType, WalReaderV2, WalWriterV2};
use std::fs::File;
use std::io::{BufReader, BufWriter, Result};
use std::path::Path;

// Re-export from wal_v2 for convenience
pub use crate::wal_v2::{CancelPayload, FundingPayload, MovePayload, OrderPayload, ReducePayload};

// ============================================================
// Cancel Order (simplified for WAL)
// ============================================================

#[derive(Debug, Clone)]
pub struct CancelOrder {
    pub order_id: u64,
    pub user_id: u64,
}

// ============================================================
// UBSCore WAL Writer
// ============================================================

pub struct UBSCoreWalWriter {
    writer: WalWriterV2<BufWriter<File>>,
    next_seq_id: u64,
    #[allow(dead_code)] // Used in rotation/snapshot logic (future phases)
    epoch: u32,
}

impl UBSCoreWalWriter {
    /// Create a new WAL writer
    pub fn new(path: impl AsRef<Path>, epoch: u32, start_seq: u64) -> Result<Self> {
        let file = File::create(path)?;
        let buf_writer = BufWriter::new(file);
        let writer = WalWriterV2::new(buf_writer, epoch, start_seq);

        Ok(Self {
            writer,
            next_seq_id: start_seq,
            epoch,
        })
    }

    /// Append an order to WAL
    /// Returns the assigned seq_id
    pub fn append_order(&mut self, order: &InternalOrder) -> Result<u64> {
        let payload = OrderPayload {
            order_id: order.order_id,
            user_id: order.user_id,
            symbol_id: order.symbol_id,
            price: order.price,
            qty: order.qty,
            side: order.side as u8,
            order_type: order.order_type as u8,
            time_in_force: order.time_in_force as u8,
            ingested_at_ns: order.ingested_at_ns,
        };

        let payload_bytes = bincode::serialize(&payload)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        let seq_id = self
            .writer
            .write_entry(WalEntryType::Order, &payload_bytes)?;
        self.next_seq_id = seq_id + 1;

        Ok(seq_id)
    }

    /// Append a cancel order to WAL
    pub fn append_cancel(&mut self, cancel: &CancelOrder) -> Result<u64> {
        let payload = CancelPayload {
            order_id: cancel.order_id,
            user_id: cancel.user_id,
        };

        let payload_bytes = bincode::serialize(&payload)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        let seq_id = self
            .writer
            .write_entry(WalEntryType::Cancel, &payload_bytes)?;
        self.next_seq_id = seq_id + 1;

        Ok(seq_id)
    }

    /// Append a reduce order to WAL
    pub fn append_reduce(&mut self, order_id: u64, user_id: u64, reduce_qty: u64) -> Result<u64> {
        let payload = ReducePayload {
            order_id,
            user_id,
            reduce_qty,
        };

        let payload_bytes = bincode::serialize(&payload)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        let seq_id = self
            .writer
            .write_entry(crate::wal_v2::WalEntryType::Reduce, &payload_bytes)?;
        self.next_seq_id = seq_id + 1;

        Ok(seq_id)
    }

    /// Append a move order to WAL
    pub fn append_move(&mut self, order_id: u64, user_id: u64, new_price: u64) -> Result<u64> {
        let payload = MovePayload {
            order_id,
            user_id,
            new_price,
        };

        let payload_bytes = bincode::serialize(&payload)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        let seq_id = self
            .writer
            .write_entry(crate::wal_v2::WalEntryType::Move, &payload_bytes)?;
        self.next_seq_id = seq_id + 1;

        Ok(seq_id)
    }

    /// Append a deposit to WAL
    pub fn append_deposit(
        &mut self,
        user_id: u64,
        asset_id: u32,
        amount: u64,
        request_id: u64,
    ) -> Result<u64> {
        let payload = FundingPayload {
            user_id,
            asset_id,
            amount,
            request_id,
        };

        let payload_bytes = bincode::serialize(&payload)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        let seq_id = self
            .writer
            .write_entry(WalEntryType::Deposit, &payload_bytes)?;
        self.next_seq_id = seq_id + 1;

        Ok(seq_id)
    }

    /// Append a withdraw to WAL
    pub fn append_withdraw(
        &mut self,
        user_id: u64,
        asset_id: u32,
        amount: u64,
        request_id: u64,
    ) -> Result<u64> {
        let payload = FundingPayload {
            user_id,
            asset_id,
            amount,
            request_id,
        };

        let payload_bytes = bincode::serialize(&payload)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        let seq_id = self
            .writer
            .write_entry(WalEntryType::Withdraw, &payload_bytes)?;
        self.next_seq_id = seq_id + 1;

        Ok(seq_id)
    }

    /// Flush WAL to disk
    pub fn flush(&mut self) -> Result<()> {
        self.writer.flush()
    }

    /// Get current sequence ID
    pub fn current_seq(&self) -> u64 {
        self.next_seq_id
    }
}

// ============================================================
// UBSCore WAL Reader
// ============================================================

pub struct UBSCoreWalReader {
    reader: WalReaderV2<BufReader<File>>,
}

impl UBSCoreWalReader {
    /// Open a WAL file for reading
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let file = File::open(path)?;
        let buf_reader = BufReader::new(file);
        let reader = WalReaderV2::new(buf_reader);

        Ok(Self { reader })
    }

    /// Replay WAL entries starting from a given seq_id
    /// Callback returns false to stop replay
    pub fn replay<F>(&mut self, from_seq: u64, mut callback: F) -> Result<()>
    where
        F: FnMut(&WalEntry) -> Result<bool>,
    {
        loop {
            match self.reader.read_entry()? {
                None => break,
                Some(entry) => {
                    if entry.header.seq_id >= from_seq {
                        let should_continue = callback(&entry)?;
                        if !should_continue {
                            break;
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

// ============================================================
// Unit Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{OrderType, Side};

    // --------------------------------------------------------
    // TDD Test 1: append_order increments seq_id
    // --------------------------------------------------------
    #[test]
    fn test_append_order_increments_seq() {
        let temp_path = format!("target/test_ubscore_wal_{}.wal", std::process::id());
        let _ = std::fs::remove_file(&temp_path);

        let mut writer = UBSCoreWalWriter::new(&temp_path, 1, 1).unwrap();

        let order = InternalOrder {
            order_id: 100,
            user_id: 1,
            symbol_id: 0,
            price: 50000_000000,
            qty: 1_000000,
            filled_qty: 0,
            side: Side::Buy,
            order_type: OrderType::Limit,
            time_in_force: crate::models::TimeInForce::GTC,
            status: crate::models::OrderStatus::NEW,
            lock_version: 0,
            seq_id: 0,
            ingested_at_ns: 0,
            cid: None,
        };

        let seq1 = writer.append_order(&order).unwrap();
        let seq2 = writer.append_order(&order).unwrap();
        let seq3 = writer.append_order(&order).unwrap();

        assert_eq!(seq1, 1);
        assert_eq!(seq2, 2);
        assert_eq!(seq3, 3);
        assert_eq!(writer.current_seq(), 4);

        writer.flush().unwrap();
        let _ = std::fs::remove_file(&temp_path);
    }

    // --------------------------------------------------------
    // TDD Test 2: append all entry types
    // --------------------------------------------------------
    #[test]
    fn test_append_all_entry_types() {
        let temp_path = format!("target/test_ubscore_wal_types_{}.wal", std::process::id());
        let _ = std::fs::remove_file(&temp_path);

        let mut writer = UBSCoreWalWriter::new(&temp_path, 1, 1).unwrap();

        // Order
        let order = InternalOrder {
            order_id: 100,
            user_id: 1,
            symbol_id: 0,
            price: 50000_000000,
            qty: 1_000000,
            filled_qty: 0,
            side: Side::Buy,
            order_type: OrderType::Limit,
            time_in_force: crate::models::TimeInForce::GTC,
            status: crate::models::OrderStatus::NEW,
            lock_version: 0,
            seq_id: 0,
            ingested_at_ns: 0,
            cid: None,
        };
        let seq1 = writer.append_order(&order).unwrap();

        // Cancel
        let cancel = CancelOrder {
            order_id: 100,
            user_id: 1,
        };
        let seq2 = writer.append_cancel(&cancel).unwrap();

        // Deposit
        let seq3 = writer.append_deposit(1, 0, 1000_000000, 999).unwrap();

        // Withdraw
        let seq4 = writer.append_withdraw(1, 0, 500_000000, 1000).unwrap();

        assert_eq!(seq1, 1);
        assert_eq!(seq2, 2);
        assert_eq!(seq3, 3);
        assert_eq!(seq4, 4);

        writer.flush().unwrap();

        // Verify by reading back
        let mut reader = UBSCoreWalReader::open(&temp_path).unwrap();
        let mut count = 0;
        reader
            .replay(1, |_entry| {
                count += 1;
                Ok(true)
            })
            .unwrap();

        assert_eq!(count, 4);

        let _ = std::fs::remove_file(&temp_path);
    }

    // --------------------------------------------------------
    // TDD Test 3: replay from specific seq
    // --------------------------------------------------------
    #[test]
    fn test_replay_from_seq() {
        let temp_path = format!("target/test_ubscore_wal_replay_{}.wal", std::process::id());
        let _ = std::fs::remove_file(&temp_path);

        // Write 10 orders
        {
            let mut writer = UBSCoreWalWriter::new(&temp_path, 1, 1).unwrap();
            let order = InternalOrder {
                order_id: 100,
                user_id: 1,
                symbol_id: 0,
                price: 50000_000000,
                qty: 1_000000,
                filled_qty: 0,
                side: Side::Buy,
                order_type: OrderType::Limit,
                time_in_force: crate::models::TimeInForce::GTC,
                status: crate::models::OrderStatus::NEW,
                lock_version: 0,
                seq_id: 0,
                ingested_at_ns: 0,
                cid: None,
            };

            for _ in 0..10 {
                writer.append_order(&order).unwrap();
            }
            writer.flush().unwrap();
        }

        // Replay from seq 5
        let mut reader = UBSCoreWalReader::open(&temp_path).unwrap();
        let mut seqs = Vec::new();
        reader
            .replay(5, |entry| {
                seqs.push(entry.header.seq_id);
                Ok(true)
            })
            .unwrap();

        assert_eq!(seqs, vec![5, 6, 7, 8, 9, 10]);

        let _ = std::fs::remove_file(&temp_path);
    }
}
