//! Matching Service WAL Writer/Reader
//!
//! Business-layer WAL operations for Matching Service trades, built on top of wal_v2.

use crate::models::Trade;
use crate::wal_v2::{WalEntryType, WalReaderV2, WalWriterV2};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufReader, BufWriter, Result};
use std::path::Path;

// ============================================================
// TRADE PAYLOAD (for WAL serialization)
// ============================================================

/// Trade payload for WAL
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TradePayload {
    pub trade_id: u64,
    pub buyer_order_id: u64,
    pub seller_order_id: u64,
    pub buyer_user_id: u64,
    pub seller_user_id: u64,
    pub price: u64,
    pub qty: u64,
    pub symbol_id: u32, // Required for multi-symbol replay
}

// ============================================================
// MATCHING WAL WRITER
// ============================================================

pub struct MatchingWalWriter {
    writer: WalWriterV2<BufWriter<File>>,
    next_seq_id: u64,
    #[allow(dead_code)] // Used in rotation/snapshot logic (future phases)
    epoch: u32,
}

impl MatchingWalWriter {
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

    /// Append a trade to WAL
    /// Returns the assigned seq_id
    pub fn append_trade(&mut self, trade: &Trade, symbol_id: u32) -> Result<u64> {
        let payload = TradePayload {
            trade_id: trade.trade_id,
            buyer_order_id: trade.buyer_order_id,
            seller_order_id: trade.seller_order_id,
            buyer_user_id: trade.buyer_user_id,
            seller_user_id: trade.seller_user_id,
            price: trade.price,
            qty: trade.qty,
            symbol_id,
        };

        let payload_bytes = bincode::serialize(&payload)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        let seq_id = self
            .writer
            .write_entry(WalEntryType::Trade, &payload_bytes)?;
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
// MATCHING WAL READER
// ============================================================

pub struct MatchingWalReader {
    reader: WalReaderV2<BufReader<File>>,
}

impl MatchingWalReader {
    /// Open a WAL file for reading
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let file = File::open(path)?;
        let buf_reader = BufReader::new(file);
        let reader = WalReaderV2::new(buf_reader);

        Ok(Self { reader })
    }

    /// Replay WAL entries starting from a given seq_id
    /// Callback receives TradePayload and should return true to continue
    pub fn replay<F>(&mut self, from_seq: u64, mut callback: F) -> Result<()>
    where
        F: FnMut(u64, &TradePayload) -> Result<bool>,
    {
        loop {
            match self.reader.read_entry()? {
                None => break,
                Some(entry) => {
                    if entry.header.seq_id >= from_seq {
                        let trade_payload: TradePayload = bincode::deserialize(&entry.payload)
                            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

                        let should_continue = callback(entry.header.seq_id, &trade_payload)?;
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
// UNIT TESTS (TDD - Test First!)
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    // --------------------------------------------------------
    // TDD Test 1: Write and read a single trade (RED - should fail)
    // --------------------------------------------------------
    #[test]
    fn test_write_read_single_trade() {
        let temp_path = format!("target/test_matching_wal_{}.wal", std::process::id());
        let _ = std::fs::remove_file(&temp_path);

        // Write a single trade
        {
            let mut writer = MatchingWalWriter::new(&temp_path, 1, 1).unwrap();

            let trade = Trade {
                trade_id: 1001,
                buyer_order_id: 100,
                seller_order_id: 200,
                buyer_user_id: 1,
                seller_user_id: 2,
                price: 50000_000000, // $50000.00
                qty: 1_000000,       // 1.0
            };

            let seq_id = writer.append_trade(&trade, 0).unwrap(); // symbol_id = 0
            assert_eq!(seq_id, 1);
            writer.flush().unwrap();
        }

        // Read it back
        {
            let mut reader = MatchingWalReader::open(&temp_path).unwrap();
            let mut count = 0;

            reader
                .replay(1, |seq_id, payload| {
                    count += 1;
                    assert_eq!(seq_id, 1);
                    assert_eq!(payload.trade_id, 1001);
                    assert_eq!(payload.buyer_order_id, 100);
                    assert_eq!(payload.seller_order_id, 200);
                    assert_eq!(payload.price, 50000_000000);
                    assert_eq!(payload.qty, 1_000000);
                    assert_eq!(payload.symbol_id, 0);
                    Ok(true)
                })
                .unwrap();

            assert_eq!(count, 1);
        }

        let _ = std::fs::remove_file(&temp_path);
    }

    // --------------------------------------------------------
    // TDD Test 2: Sequential trade writes (RED - should fail)
    // --------------------------------------------------------
    #[test]
    fn test_sequential_trade_writes() {
        let temp_path = format!("target/test_matching_wal_seq_{}.wal", std::process::id());
        let _ = std::fs::remove_file(&temp_path);

        // Write multiple trades
        {
            let mut writer = MatchingWalWriter::new(&temp_path, 1, 1).unwrap();

            for i in 0..5 {
                let trade = Trade {
                    trade_id: 1000 + i,
                    buyer_order_id: 100 + i,
                    seller_order_id: 200 + i,
                    buyer_user_id: 1,
                    seller_user_id: 2,
                    price: 50000_000000,
                    qty: 1_000000,
                };

                let seq_id = writer.append_trade(&trade, 0).unwrap();
                assert_eq!(seq_id, 1 + i); // seq_id should increment: 1, 2, 3, 4, 5
            }

            assert_eq!(writer.current_seq(), 6); // Next seq should be 6
            writer.flush().unwrap();
        }

        // Verify by reading back
        {
            let mut reader = MatchingWalReader::open(&temp_path).unwrap();
            let mut seqs = Vec::new();

            reader
                .replay(1, |seq_id, _payload| {
                    seqs.push(seq_id);
                    Ok(true)
                })
                .unwrap();

            assert_eq!(seqs, vec![1, 2, 3, 4, 5]);
        }

        let _ = std::fs::remove_file(&temp_path);
    }

    // --------------------------------------------------------
    // TDD Test 3: Checksum validation (RED - should fail)
    // --------------------------------------------------------
    #[test]
    fn test_trade_checksum_validation() {
        let temp_path = format!("target/test_matching_wal_crc_{}.wal", std::process::id());
        let _ = std::fs::remove_file(&temp_path);

        // Write a trade
        {
            let mut writer = MatchingWalWriter::new(&temp_path, 1, 1).unwrap();
            let trade = Trade {
                trade_id: 999,
                buyer_order_id: 100,
                seller_order_id: 200,
                buyer_user_id: 1,
                seller_user_id: 2,
                price: 50000_000000,
                qty: 1_000000,
            };
            writer.append_trade(&trade, 0).unwrap();
            writer.flush().unwrap();
        }

        // Corrupt the WAL file (flip a bit in payload area)
        {
            use std::fs::OpenOptions;
            use std::io::{Seek, SeekFrom, Write};

            let mut file = OpenOptions::new().write(true).open(&temp_path).unwrap();

            // Skip 20-byte header, corrupt first byte of payload
            file.seek(SeekFrom::Start(20)).unwrap();
            file.write_all(&[0xFF]).unwrap(); // Corrupt payload
            file.flush().unwrap();
        }

        // Reading should fail with checksum error
        {
            let mut reader = MatchingWalReader::open(&temp_path).unwrap();
            let result = reader.replay(1, |_seq, _payload| Ok(true));

            assert!(result.is_err());
            let err_msg = result.unwrap_err().to_string();
            assert!(
                err_msg.contains("CRC32 checksum mismatch"),
                "Expected CRC32 error, got: {}",
                err_msg
            );
        }

        let _ = std::fs::remove_file(&temp_path);
    }
}
