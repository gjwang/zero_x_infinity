//! Write-Ahead Log (WAL) - InternalOrder persistence
//!
//! The WAL is the Single Source of Truth for order processing.
//! All orders are persisted to WAL BEFORE balance locking and matching.
//!
//! # Design Principles
//!
//! 1. **Append-Only**: Sequential writes for maximum I/O performance
//! 2. **Group Commit**: Batch multiple orders before fsync for throughput
//! 3. **Deterministic Replay**: System state can be fully rebuilt from WAL
//!
//! # Performance
//!
//! | Strategy | Latency | Throughput |
//! |----------|---------|------------|
//! | Per-entry fsync | ~50µs | ~20K/s |
//! | Group commit (100) | ~5µs amortized | ~200K/s |
//! | Group commit (1ms) | ~1µs amortized | ~1M/s |

use crate::core_types::SeqNum;
use crate::models::InternalOrder;
use std::fs::{File, OpenOptions};
use std::io::{self, BufWriter, Write};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

// ============================================================
// WAL ENTRY FORMAT
// ============================================================

/// WAL entry - a single order record
///
/// Binary format (for future optimization):
/// - seq_id: u64 (8 bytes)
/// - timestamp_ns: u64 (8 bytes)  
/// - order_id: u64 (8 bytes)
/// - user_id: u64 (8 bytes)
/// - price: u64 (8 bytes)
/// - qty: u64 (8 bytes)
/// - side: u8 (1 byte)
/// - order_type: u8 (1 byte)
/// - checksum: u32 (4 bytes)
/// Total: 54 bytes per entry
///
/// Currently using CSV for readability during development.
#[derive(Debug, Clone)]
pub struct WalEntry {
    pub seq_id: SeqNum,
    pub timestamp_ns: u64,
    pub order: InternalOrder,
}

impl WalEntry {
    pub fn new(seq_id: SeqNum, order: InternalOrder) -> Self {
        let timestamp_ns = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;

        Self {
            seq_id,
            timestamp_ns,
            order,
        }
    }

    /// Serialize to CSV line (development format)
    pub fn to_csv_line(&self) -> String {
        format!(
            "{},{},{},{},{},{},{},{:?},{:?},{}\n",
            self.seq_id,
            self.timestamp_ns,
            self.order.order_id,
            self.order.user_id,
            self.order.symbol_id,
            self.order.price,
            self.order.qty,
            self.order.side,
            self.order.order_type,
            self.order.ingested_at_ns,
        )
    }
}

// ============================================================
// WAL WRITER
// ============================================================

/// WAL configuration
#[derive(Debug, Clone)]
pub struct WalConfig {
    /// Path to WAL file
    pub path: String,
    /// Entries to buffer before auto-flush (0 = manual flush only)
    pub flush_interval_entries: usize,
    /// Whether to sync to disk on flush
    pub sync_on_flush: bool,
}

impl Default for WalConfig {
    fn default() -> Self {
        Self {
            path: "wal/orders.wal".to_string(),
            flush_interval_entries: 100,
            sync_on_flush: true,
        }
    }
}

/// Write-Ahead Log writer
///
/// # Thread Safety
/// This is designed to be used from a SINGLE thread (UBSCore).
/// No locking is needed.
pub struct WalWriter {
    writer: BufWriter<File>,
    next_seq: SeqNum,
    pending_count: usize,
    config: WalConfig,
    // Stats
    total_entries: u64,
    total_bytes: u64,
}

impl WalWriter {
    /// Create a new WAL writer
    ///
    /// Creates the parent directory if it doesn't exist.
    pub fn new(config: WalConfig) -> io::Result<Self> {
        // Create parent directory if needed
        if let Some(parent) = Path::new(&config.path).parent() {
            std::fs::create_dir_all(parent)?;
        }

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&config.path)?;

        let writer = BufWriter::with_capacity(64 * 1024, file); // 64KB buffer

        Ok(Self {
            writer,
            next_seq: 1,
            pending_count: 0,
            config,
            total_entries: 0,
            total_bytes: 0,
        })
    }

    /// Create WAL writer for testing (in-memory stats only)
    pub fn new_with_seq(config: WalConfig, start_seq: SeqNum) -> io::Result<Self> {
        let mut wal = Self::new(config)?;
        wal.next_seq = start_seq;
        Ok(wal)
    }

    /// Append an order to the WAL
    ///
    /// Returns the assigned sequence number.
    /// Does NOT flush immediately (use group commit).
    pub fn append(&mut self, order: &InternalOrder) -> io::Result<SeqNum> {
        let seq_id = self.next_seq;
        self.next_seq += 1;

        let entry = WalEntry::new(seq_id, order.clone());
        let line = entry.to_csv_line();
        let bytes = line.as_bytes();

        self.writer.write_all(bytes)?;
        self.pending_count += 1;
        self.total_entries += 1;
        self.total_bytes += bytes.len() as u64;

        // Auto-flush if configured
        if self.config.flush_interval_entries > 0
            && self.pending_count >= self.config.flush_interval_entries
        {
            self.flush()?;
        }

        Ok(seq_id)
    }

    /// Flush buffered writes to disk
    pub fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()?;

        if self.config.sync_on_flush {
            self.writer.get_ref().sync_data()?;
        }

        self.pending_count = 0;
        Ok(())
    }

    /// Get next sequence number (without incrementing)
    pub fn peek_next_seq(&self) -> SeqNum {
        self.next_seq
    }

    /// Get current sequence number (last assigned)
    pub fn current_seq(&self) -> SeqNum {
        self.next_seq.saturating_sub(1)
    }

    /// Get number of pending (unflushed) entries  
    pub fn pending_count(&self) -> usize {
        self.pending_count
    }

    /// Get total entries written
    pub fn total_entries(&self) -> u64 {
        self.total_entries
    }

    /// Get total bytes written
    pub fn total_bytes(&self) -> u64 {
        self.total_bytes
    }
}

// ============================================================
// WAL READER (for replay/recovery)
// ============================================================

/// WAL reader for recovery
pub struct WalReader {
    path: String,
}

impl WalReader {
    pub fn new(path: &str) -> Self {
        Self {
            path: path.to_string(),
        }
    }

    /// Replay WAL entries, calling the callback for each
    ///
    /// Returns the number of entries replayed and the last seq_id.
    pub fn replay<F>(&self, mut callback: F) -> io::Result<(u64, SeqNum)>
    where
        F: FnMut(WalEntry),
    {
        use std::io::BufRead;

        let file = match File::open(&self.path) {
            Ok(f) => f,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                return Ok((0, 0)); // No WAL file, fresh start
            }
            Err(e) => return Err(e),
        };

        let reader = std::io::BufReader::new(file);
        let mut count = 0u64;
        let mut last_seq = 0;

        for line in reader.lines() {
            let line = line?;
            if line.is_empty() {
                continue;
            }

            if let Some(entry) = Self::parse_csv_line(&line) {
                last_seq = entry.seq_id;
                callback(entry);
                count += 1;
            }
        }

        Ok((count, last_seq))
    }

    /// Parse a CSV line into WalEntry
    fn parse_csv_line(line: &str) -> Option<WalEntry> {
        use crate::models::{OrderStatus, OrderType, Side};

        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() < 10 {
            return None;
        }

        let seq_id: SeqNum = parts[0].parse().ok()?;
        let timestamp_ns: u64 = parts[1].parse().ok()?;
        let order_id: u64 = parts[2].parse().ok()?;
        let user_id: u64 = parts[3].parse().ok()?;
        let symbol_id: u32 = parts[4].parse().ok()?;
        let price: u64 = parts[5].parse().ok()?;
        let qty: u64 = parts[6].parse().ok()?;

        let side = match parts[7] {
            "Buy" => Side::Buy,
            "Sell" => Side::Sell,
            _ => return None,
        };

        let order_type = match parts[8].trim() {
            "Limit" => OrderType::Limit,
            "Market" => OrderType::Market,
            _ => return None,
        };

        let ingested_at_ns: u64 = parts[9].trim().parse().unwrap_or(0);

        let order = InternalOrder {
            order_id,
            user_id,
            symbol_id,
            price,
            qty,
            filled_qty: 0,
            side,
            order_type,
            status: OrderStatus::NEW,
            ingested_at_ns,
            lock_version: 0,
            seq_id: 0,
            cid: None,
        };

        Some(WalEntry {
            seq_id,
            timestamp_ns,
            order,
        })
    }
}

// ============================================================
// TESTS
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Side;
    use std::fs;

    fn test_wal_path() -> String {
        format!("target/test_wal_{}.wal", std::process::id())
    }

    #[test]
    fn test_wal_write_and_read() {
        let path = test_wal_path();

        // Clean up from previous runs
        let _ = fs::remove_file(&path);

        // Write some orders
        {
            let config = WalConfig {
                path: path.clone(),
                flush_interval_entries: 0, // Manual flush
                sync_on_flush: false,
            };

            let mut wal = WalWriter::new(config).unwrap();

            let order1 = InternalOrder::new(1, 100, 0, 10000, 1000, Side::Buy);
            let order2 = InternalOrder::new(2, 101, 0, 11000, 2000, Side::Sell);

            let seq1 = wal.append(&order1).unwrap();
            let seq2 = wal.append(&order2).unwrap();

            assert_eq!(seq1, 1);
            assert_eq!(seq2, 2);
            assert_eq!(wal.pending_count(), 2);
            assert_eq!(wal.total_entries(), 2);

            wal.flush().unwrap();
            assert_eq!(wal.pending_count(), 0);
        }

        // Read back
        {
            let reader = WalReader::new(&path);
            let mut entries = Vec::new();

            let (count, last_seq) = reader.replay(|e| entries.push(e)).unwrap();

            assert_eq!(count, 2);
            assert_eq!(last_seq, 2);
            assert_eq!(entries[0].order.order_id, 1);
            assert_eq!(entries[0].order.price, 10000);
            assert_eq!(entries[1].order.order_id, 2);
            assert_eq!(entries[1].order.side, Side::Sell);
        }

        // Clean up
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn test_wal_auto_flush() {
        let path = test_wal_path() + "_auto";
        let _ = fs::remove_file(&path);

        let config = WalConfig {
            path: path.clone(),
            flush_interval_entries: 3, // Auto-flush every 3 entries
            sync_on_flush: false,
        };

        let mut wal = WalWriter::new(config).unwrap();

        // Add 2 entries - should not auto-flush
        wal.append(&InternalOrder::new(1, 100, 0, 10000, 1000, Side::Buy))
            .unwrap();
        wal.append(&InternalOrder::new(2, 100, 0, 10000, 1000, Side::Buy))
            .unwrap();
        assert_eq!(wal.pending_count(), 2);

        // Add 3rd entry - should trigger auto-flush
        wal.append(&InternalOrder::new(3, 100, 0, 10000, 1000, Side::Buy))
            .unwrap();
        assert_eq!(wal.pending_count(), 0); // Flushed!

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn test_wal_entry_format() {
        let order = InternalOrder::new(42, 100, 0, 50000, 1000, Side::Buy);
        let entry = WalEntry {
            seq_id: 1,
            timestamp_ns: 1234567890,
            order,
        };

        let csv = entry.to_csv_line();
        // Format: seq_id,timestamp,order_id,user_id,symbol_id,price,qty,side,type,ingested_at
        assert!(csv.contains("1,1234567890,42,100,0,50000,1000,Buy,Limit,0"));
    }
}
