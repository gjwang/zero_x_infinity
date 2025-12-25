//! Settlement Service Checkpoint WAL Writer/Reader
//!
//! Lightweight WAL that only records processing progress (last_trade_id).
//! All actual business data (trades, balances) is persisted to TDengine.

use crate::wal_v2::{WalEntryType, WalReaderV2, WalWriterV2};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufReader, BufWriter, Result};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

// ============================================================
// CHECKPOINT PAYLOAD (for WAL serialization)
// ============================================================

/// Checkpoint payload - extremely lightweight (~16 bytes)
///
/// Records the last processed trade_id for crash recovery.
/// No business data stored - all trades are in TDengine.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CheckpointPayload {
    /// Last successfully processed trade_id
    pub last_trade_id: u64,
    /// Timestamp when checkpoint was created (nanoseconds since UNIX epoch)
    pub timestamp_ns: u64,
}

impl CheckpointPayload {
    /// Create a new checkpoint with current timestamp
    pub fn new(last_trade_id: u64) -> Self {
        let timestamp_ns = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);

        Self {
            last_trade_id,
            timestamp_ns,
        }
    }
}

// ============================================================
// SETTLEMENT WAL WRITER
// ============================================================

/// WAL writer for Settlement Service checkpoints
///
/// # Usage
///
/// ```rust,ignore
/// let mut writer = SettlementWalWriter::new("data/settlement/wal/checkpoint.wal", 1, 1)?;
///
/// // After processing trades, record checkpoint
/// writer.append_checkpoint(1000)?;
/// writer.flush()?;
/// ```
pub struct SettlementWalWriter {
    writer: WalWriterV2<BufWriter<File>>,
    next_seq_id: u64,
    #[allow(dead_code)]
    epoch: u32,
}

impl SettlementWalWriter {
    /// Create a new WAL writer
    ///
    /// # Arguments
    /// * `path` - Path to WAL file
    /// * `epoch` - WAL epoch (for rotation, future use)
    /// * `start_seq` - Starting sequence ID
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

    /// Append a checkpoint to WAL
    ///
    /// Returns the assigned seq_id for this checkpoint.
    pub fn append_checkpoint(&mut self, last_trade_id: u64) -> Result<u64> {
        let payload = CheckpointPayload::new(last_trade_id);

        let payload_bytes = bincode::serialize(&payload)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        // Use SettlementCheckpoint entry type (0x10)
        let seq_id = self
            .writer
            .write_entry(WalEntryType::SettlementCheckpoint, &payload_bytes)?;
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
// SETTLEMENT WAL READER
// ============================================================

/// WAL reader for Settlement Service checkpoints
pub struct SettlementWalReader {
    reader: WalReaderV2<BufReader<File>>,
}

impl SettlementWalReader {
    /// Open a WAL file for reading
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let file = File::open(path)?;
        let buf_reader = BufReader::new(file);
        let reader = WalReaderV2::new(buf_reader);

        Ok(Self { reader })
    }

    /// Replay WAL entries and return the highest last_trade_id found
    ///
    /// This is simpler than Matching WAL because we only care about
    /// the final checkpoint value, not individual entries.
    pub fn replay_to_latest(&mut self) -> Result<Option<u64>> {
        let mut highest_trade_id: Option<u64> = None;

        loop {
            match self.reader.read_entry()? {
                None => break,
                Some(entry) => {
                    if entry.header.entry_type == WalEntryType::SettlementCheckpoint as u8 {
                        let payload: CheckpointPayload = bincode::deserialize(&entry.payload)
                            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

                        highest_trade_id =
                            Some(highest_trade_id.map_or(payload.last_trade_id, |prev| {
                                prev.max(payload.last_trade_id)
                            }));
                    }
                }
            }
        }

        Ok(highest_trade_id)
    }

    /// Replay WAL entries with callback (for debugging/testing)
    pub fn replay<F>(&mut self, mut callback: F) -> Result<()>
    where
        F: FnMut(u64, &CheckpointPayload) -> Result<bool>,
    {
        loop {
            match self.reader.read_entry()? {
                None => break,
                Some(entry) => {
                    if entry.header.entry_type == WalEntryType::SettlementCheckpoint as u8 {
                        let payload: CheckpointPayload = bincode::deserialize(&entry.payload)
                            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

                        let should_continue = callback(entry.header.seq_id, &payload)?;
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
    // TDD Test 1: Write and read a single checkpoint
    // --------------------------------------------------------
    #[test]
    fn test_write_read_checkpoint() {
        let temp_path = format!("target/test_settlement_wal_{}.wal", std::process::id());
        let _ = std::fs::remove_file(&temp_path);

        // Write a single checkpoint
        {
            let mut writer = SettlementWalWriter::new(&temp_path, 1, 1).unwrap();
            let seq_id = writer.append_checkpoint(1000).unwrap();
            assert_eq!(seq_id, 1);
            writer.flush().unwrap();
        }

        // Read it back
        {
            let mut reader = SettlementWalReader::open(&temp_path).unwrap();
            let highest = reader.replay_to_latest().unwrap();
            assert_eq!(highest, Some(1000));
        }

        let _ = std::fs::remove_file(&temp_path);
    }

    // --------------------------------------------------------
    // TDD Test 2: Sequential checkpoint writes
    // --------------------------------------------------------
    #[test]
    fn test_checkpoint_sequence() {
        let temp_path = format!("target/test_settlement_wal_seq_{}.wal", std::process::id());
        let _ = std::fs::remove_file(&temp_path);

        // Write multiple checkpoints with increasing trade_ids
        {
            let mut writer = SettlementWalWriter::new(&temp_path, 1, 1).unwrap();

            for i in 1..=5 {
                let trade_id = i * 1000;
                let seq_id = writer.append_checkpoint(trade_id).unwrap();
                assert_eq!(seq_id, i); // seq_id: 1, 2, 3, 4, 5
            }

            assert_eq!(writer.current_seq(), 6);
            writer.flush().unwrap();
        }

        // Verify highest trade_id
        {
            let mut reader = SettlementWalReader::open(&temp_path).unwrap();
            let highest = reader.replay_to_latest().unwrap();
            assert_eq!(highest, Some(5000)); // last checkpoint value
        }

        // Verify all entries with callback
        {
            let mut reader = SettlementWalReader::open(&temp_path).unwrap();
            let mut trade_ids = Vec::new();

            reader
                .replay(|_seq, payload| {
                    trade_ids.push(payload.last_trade_id);
                    Ok(true)
                })
                .unwrap();

            assert_eq!(trade_ids, vec![1000, 2000, 3000, 4000, 5000]);
        }

        let _ = std::fs::remove_file(&temp_path);
    }

    // --------------------------------------------------------
    // TDD Test 3: Checksum validation
    // --------------------------------------------------------
    #[test]
    fn test_checkpoint_crc_validation() {
        let temp_path = format!("target/test_settlement_wal_crc_{}.wal", std::process::id());
        let _ = std::fs::remove_file(&temp_path);

        // Write a checkpoint
        {
            let mut writer = SettlementWalWriter::new(&temp_path, 1, 1).unwrap();
            writer.append_checkpoint(999).unwrap();
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
            let mut reader = SettlementWalReader::open(&temp_path).unwrap();
            let result = reader.replay_to_latest();

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
