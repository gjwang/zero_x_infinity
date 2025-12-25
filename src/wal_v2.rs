//! Universal WAL Format v2 (Phase 0x0D)
//!
//! A type-extensible, binary WAL format with:
//! - 20-byte naturally aligned header
//! - CRC32 checksum for integrity
//! - Epoch-based gap recovery
//! - bincode serialization for payloads
//!
//! # Header Layout (20 bytes)
//!
//! ```text
//! ┌────────────┬───────────┬────────────────────────────────────┐
//! │ payload_len│ 2 bytes   │ Payload size (max 64KB)            │
//! │ entry_type │ 1 byte    │ Event type (Order/Trade/...)       │
//! │ version    │ 1 byte    │ Payload format version (0-255)     │
//! │ epoch      │ 4 bytes   │ EPOCH (restarts from new epoch)    │
//! │ seq_id     │ 8 bytes   │ Monotonic sequence within EPOCH    │
//! │ checksum   │ 4 bytes   │ CRC32 of payload                   │
//! └────────────┴───────────┴────────────────────────────────────┘
//! ```

use crc32fast::Hasher;
use serde::{Deserialize, Serialize};
use std::io::{self, Read, Write};

// ============================================================
// CONSTANTS
// ============================================================

/// WAL Header size in bytes (20 bytes, naturally aligned)
pub const WAL_HEADER_SIZE: usize = 20;

// ============================================================
// WAL HEADER (20 bytes)
// ============================================================

/// Universal WAL header (20 bytes, naturally aligned)
///
/// Field order is optimized for natural alignment (no padding):
/// - seq_id (u64) = 8 bytes (8-byte aligned)
/// - epoch (u32) + checksum (u32) = 8 bytes (4-byte aligned)  
/// - payload_len (u16) + entry_type (u8) + version (u8) = 4 bytes
/// Total = 20 bytes with #[repr(C, packed)]
#[repr(C, packed)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WalHeader {
    /// Monotonic sequence within EPOCH (8 bytes)
    pub seq_id: u64,
    /// EPOCH number (increments on recovery from gap) (4 bytes)
    pub epoch: u32,
    /// CRC32 checksum of payload (4 bytes)
    pub checksum: u32,
    /// Payload size in bytes (max 64KB) (2 bytes)
    pub payload_len: u16,
    /// Entry type (see WalEntryType enum) (1 byte)
    pub entry_type: u8,
    /// Payload format version (0-255) (1 byte)
    pub version: u8,
}

impl WalHeader {
    /// Create a new header with CRC32 checksum calculated from payload
    pub fn new(entry_type: WalEntryType, epoch: u32, seq_id: u64, payload: &[u8]) -> Self {
        let checksum = crc32_checksum(payload);
        Self {
            payload_len: payload.len() as u16,
            entry_type: entry_type as u8,
            version: 0,
            epoch,
            seq_id,
            checksum,
        }
    }

    /// Serialize header to bytes (20 bytes)
    pub fn to_bytes(&self) -> [u8; WAL_HEADER_SIZE] {
        let mut buf = [0u8; WAL_HEADER_SIZE];
        buf[0..2].copy_from_slice(&self.payload_len.to_le_bytes());
        buf[2] = self.entry_type;
        buf[3] = self.version;
        buf[4..8].copy_from_slice(&self.epoch.to_le_bytes());
        buf[8..16].copy_from_slice(&self.seq_id.to_le_bytes());
        buf[16..20].copy_from_slice(&self.checksum.to_le_bytes());
        buf
    }

    /// Deserialize header from bytes
    pub fn from_bytes(buf: &[u8; WAL_HEADER_SIZE]) -> Self {
        Self {
            payload_len: u16::from_le_bytes([buf[0], buf[1]]),
            entry_type: buf[2],
            version: buf[3],
            epoch: u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]),
            seq_id: u64::from_le_bytes([
                buf[8], buf[9], buf[10], buf[11], buf[12], buf[13], buf[14], buf[15],
            ]),
            checksum: u32::from_le_bytes([buf[16], buf[17], buf[18], buf[19]]),
        }
    }

    /// Verify CRC32 checksum against payload
    pub fn verify_checksum(&self, payload: &[u8]) -> bool {
        self.checksum == crc32_checksum(payload)
    }
}

// ============================================================
// ENTRY TYPES
// ============================================================

/// WAL entry types
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WalEntryType {
    Order = 1,
    Cancel = 2,
    Trade = 3,
    BalanceSettle = 4,
    Deposit = 5,
    Withdraw = 6,
    SnapshotMarker = 7,
}

impl TryFrom<u8> for WalEntryType {
    type Error = io::Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Order),
            2 => Ok(Self::Cancel),
            3 => Ok(Self::Trade),
            4 => Ok(Self::BalanceSettle),
            5 => Ok(Self::Deposit),
            6 => Ok(Self::Withdraw),
            7 => Ok(Self::SnapshotMarker),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Unknown WalEntryType: {}", value),
            )),
        }
    }
}

// ============================================================
// PAYLOAD DEFINITIONS
// ============================================================

/// Order payload (entry_type = 1)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OrderPayload {
    pub order_id: u64,
    pub user_id: u64,
    pub symbol_id: u32,
    pub price: u64,
    pub qty: u64,
    pub side: u8,       // 0=Buy, 1=Sell
    pub order_type: u8, // 0=Limit, 1=Market
    pub ingested_at_ns: u64,
}

/// Cancel payload (entry_type = 2)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CancelPayload {
    pub order_id: u64,
    pub user_id: u64,
}

/// Funding payload (entry_type = 5, 6: Deposit/Withdraw)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FundingPayload {
    pub user_id: u64,
    pub asset_id: u32,
    pub amount: u64,
    pub request_id: u64,
}

/// Snapshot marker payload (entry_type = 7)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SnapshotMarkerPayload {
    pub snapshot_dir: String,
    pub timestamp_ns: u64,
}

// ============================================================
// CRC32 HELPER
// ============================================================

/// Calculate CRC32 checksum of data
#[inline]
pub fn crc32_checksum(data: &[u8]) -> u32 {
    let mut hasher = Hasher::new();
    hasher.update(data);
    hasher.finalize()
}

// ============================================================
// WAL WRITER V2
// ============================================================

/// WAL Writer v2 with binary format
pub struct WalWriterV2<W: Write> {
    writer: W,
    epoch: u32,
    next_seq: u64,
}

impl<W: Write> WalWriterV2<W> {
    /// Create a new WAL writer
    pub fn new(writer: W, epoch: u32, start_seq: u64) -> Self {
        Self {
            writer,
            epoch,
            next_seq: start_seq,
        }
    }

    /// Write an entry to WAL
    pub fn write_entry(&mut self, entry_type: WalEntryType, payload: &[u8]) -> io::Result<u64> {
        let seq_id = self.next_seq;
        self.next_seq += 1;

        let header = WalHeader::new(entry_type, self.epoch, seq_id, payload);
        self.writer.write_all(&header.to_bytes())?;
        self.writer.write_all(payload)?;

        Ok(seq_id)
    }

    /// Flush buffered writes
    pub fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }

    /// Get current epoch
    pub fn epoch(&self) -> u32 {
        self.epoch
    }

    /// Get next sequence number
    pub fn next_seq(&self) -> u64 {
        self.next_seq
    }
}

// ============================================================
// WAL READER V2
// ============================================================

/// WAL Reader v2 with binary format
pub struct WalReaderV2<R: Read> {
    reader: R,
}

/// A single WAL entry (header + payload)
#[derive(Debug)]
pub struct WalEntry {
    pub header: WalHeader,
    pub payload: Vec<u8>,
}

impl<R: Read> WalReaderV2<R> {
    /// Create a new WAL reader
    pub fn new(reader: R) -> Self {
        Self { reader }
    }

    /// Read the next entry, returns None at EOF
    pub fn read_entry(&mut self) -> io::Result<Option<WalEntry>> {
        // Read header
        let mut header_buf = [0u8; WAL_HEADER_SIZE];
        match self.reader.read_exact(&mut header_buf) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
            Err(e) => return Err(e),
        }

        let header = WalHeader::from_bytes(&header_buf);

        // Read payload
        let mut payload = vec![0u8; header.payload_len as usize];
        self.reader.read_exact(&mut payload)?;

        // Verify checksum
        if !header.verify_checksum(&payload) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "CRC32 checksum mismatch at seq_id={}, expected={}, got={}",
                    header.seq_id,
                    header.checksum,
                    crc32_checksum(&payload)
                ),
            ));
        }

        Ok(Some(WalEntry { header, payload }))
    }

    /// Iterate over all entries
    pub fn iter(&mut self) -> WalEntryIterator<'_, R> {
        WalEntryIterator { reader: self }
    }
}

/// Iterator over WAL entries
pub struct WalEntryIterator<'a, R: Read> {
    reader: &'a mut WalReaderV2<R>,
}

impl<R: Read> Iterator for WalEntryIterator<'_, R> {
    type Item = io::Result<WalEntry>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.reader.read_entry() {
            Ok(Some(entry)) => Some(Ok(entry)),
            Ok(None) => None,
            Err(e) => Some(Err(e)),
        }
    }
}

// ============================================================
// TESTS
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    // --------------------------------------------------------
    // TDD Test 1: Header size must be exactly 20 bytes
    // --------------------------------------------------------
    #[test]
    fn test_wal_header_size_20_bytes() {
        assert_eq!(
            std::mem::size_of::<WalHeader>(),
            WAL_HEADER_SIZE,
            "WalHeader must be exactly 20 bytes"
        );
    }

    // --------------------------------------------------------
    // TDD Test 2: CRC32 checksum calculation
    // --------------------------------------------------------
    #[test]
    fn test_crc32_checksum() {
        let data = b"hello world";
        let checksum = crc32_checksum(data);

        // Verify it's deterministic
        assert_eq!(checksum, crc32_checksum(data));

        // Verify it changes with different data
        let checksum2 = crc32_checksum(b"hello worlD");
        assert_ne!(checksum, checksum2);
    }

    // --------------------------------------------------------
    // TDD Test 3: Header serialization round-trip
    // --------------------------------------------------------
    #[test]
    fn test_header_serialization_round_trip() {
        let payload = b"test payload data";
        let header = WalHeader::new(WalEntryType::Order, 1, 42, payload);

        let bytes = header.to_bytes();
        assert_eq!(bytes.len(), WAL_HEADER_SIZE);

        let header2 = WalHeader::from_bytes(&bytes);
        assert_eq!(header, header2);
    }

    // --------------------------------------------------------
    // TDD Test 4: Checksum verification
    // --------------------------------------------------------
    #[test]
    fn test_checksum_verification() {
        let payload = b"test payload";
        let header = WalHeader::new(WalEntryType::Order, 1, 1, payload);

        // Correct payload should verify
        assert!(header.verify_checksum(payload));

        // Wrong payload should fail
        assert!(!header.verify_checksum(b"wrong payload"));
    }

    // --------------------------------------------------------
    // TDD Test 5: Binary read/write round-trip
    // --------------------------------------------------------
    #[test]
    fn test_binary_round_trip() {
        let mut buffer = Vec::new();

        // Write entries
        {
            let mut writer = WalWriterV2::new(&mut buffer, 1, 1);

            let payload1 = bincode::serialize(&OrderPayload {
                order_id: 100,
                user_id: 1,
                symbol_id: 0,
                price: 50000,
                qty: 1000,
                side: 0,
                order_type: 0,
                ingested_at_ns: 0,
            })
            .unwrap();

            let payload2 = bincode::serialize(&CancelPayload {
                order_id: 100,
                user_id: 1,
            })
            .unwrap();

            let seq1 = writer.write_entry(WalEntryType::Order, &payload1).unwrap();
            let seq2 = writer.write_entry(WalEntryType::Cancel, &payload2).unwrap();

            assert_eq!(seq1, 1);
            assert_eq!(seq2, 2);
            writer.flush().unwrap();
        }

        // Read entries back
        {
            let cursor = Cursor::new(&buffer);
            let mut reader = WalReaderV2::new(cursor);

            // Entry 1
            let entry1 = reader.read_entry().unwrap().expect("should have entry 1");
            assert_eq!(entry1.header.entry_type, WalEntryType::Order as u8);
            assert_eq!(entry1.header.seq_id, 1);
            assert_eq!(entry1.header.epoch, 1);

            let order: OrderPayload = bincode::deserialize(&entry1.payload).unwrap();
            assert_eq!(order.order_id, 100);
            assert_eq!(order.price, 50000);

            // Entry 2
            let entry2 = reader.read_entry().unwrap().expect("should have entry 2");
            assert_eq!(entry2.header.entry_type, WalEntryType::Cancel as u8);
            assert_eq!(entry2.header.seq_id, 2);

            let cancel: CancelPayload = bincode::deserialize(&entry2.payload).unwrap();
            assert_eq!(cancel.order_id, 100);

            // EOF
            assert!(reader.read_entry().unwrap().is_none());
        }
    }

    // --------------------------------------------------------
    // TDD Test 6: All entry types
    // --------------------------------------------------------
    #[test]
    fn test_all_entry_types() {
        let types = [
            (WalEntryType::Order, 1u8),
            (WalEntryType::Cancel, 2),
            (WalEntryType::Trade, 3),
            (WalEntryType::BalanceSettle, 4),
            (WalEntryType::Deposit, 5),
            (WalEntryType::Withdraw, 6),
            (WalEntryType::SnapshotMarker, 7),
        ];

        for (entry_type, expected_value) in types {
            assert_eq!(entry_type as u8, expected_value);
            assert_eq!(WalEntryType::try_from(expected_value).unwrap(), entry_type);
        }
    }

    // --------------------------------------------------------
    // TDD Test 7: Corrupted checksum detection
    // --------------------------------------------------------
    #[test]
    fn test_corrupted_checksum_detection() {
        let mut buffer = Vec::new();

        // Write entry
        {
            let mut writer = WalWriterV2::new(&mut buffer, 1, 1);
            let payload = b"test data";
            writer.write_entry(WalEntryType::Order, payload).unwrap();
            writer.flush().unwrap();
        }

        // Corrupt the payload (byte after header)
        buffer[WAL_HEADER_SIZE] ^= 0xFF;

        // Read should fail with checksum error
        {
            let cursor = Cursor::new(&buffer);
            let mut reader = WalReaderV2::new(cursor);

            let result = reader.read_entry();
            assert!(result.is_err());
            assert!(
                result
                    .unwrap_err()
                    .to_string()
                    .contains("CRC32 checksum mismatch")
            );
        }
    }
}
