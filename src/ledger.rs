//! Ledger - Complete audit log for all balance changes
//!
//! Records every balance change for complete auditability and event sourcing.
//!
//! # Event Types
//! - **Lock**: Funds locked for order placement
//! - **Unlock**: Funds unlocked on order cancel/partial fill
//! - **Settle**: Trade settlement (spend frozen + receive)
//! - **Deposit**: External deposit
//! - **Withdraw**: External withdrawal
//!
//! # Separated Version Spaces
//! Each event type is tracked in its own version space:
//! - Lock/Unlock events use `lock_version`
//! - Settle events use `settle_version`
//!
//! This enables deterministic verification even when events
//! from different queues are interleaved in a pipelined architecture.

use crate::messages::{BalanceEvent, OrderEvent};
use std::fs::File;
use std::io::{BufWriter, Write};

// ============================================================
// CONSTANTS
// ============================================================

pub const OP_CREDIT: &str = "credit";
pub const OP_DEBIT: &str = "debit";

// ============================================================
// LEGACY LEDGER ENTRY (for backward compatibility)
// ============================================================

/// Legacy ledger entry for settlement audit (backward compatible)
/// Each balance change is recorded as one entry
#[derive(Debug, Clone)]
pub struct LedgerEntry {
    pub trade_id: u64,
    pub user_id: u64,
    pub asset_id: u32,
    pub op: &'static str, // "credit" or "debit"
    pub delta: u64,
    pub balance_after: u64,
}

// ============================================================
// LEDGER WRITER
// ============================================================

/// Writes ledger entries to CSV file
///
/// Supports both legacy format (for backward compatibility) and
/// new BalanceEvent format (for complete event sourcing).
///
/// # Performance Optimization
/// Uses BufWriter to batch I/O operations.
pub struct LedgerWriter {
    /// Buffered writer for legacy format
    legacy_writer: BufWriter<File>,
    /// Entry count for legacy format
    legacy_count: u64,
    /// Optional: New event writer for complete event sourcing
    event_writer: Option<BufWriter<File>>,
    /// Event count for new format
    event_count: u64,
    /// Optional: Order event writer
    order_writer: Option<BufWriter<File>>,
    /// Order event count
    order_count: u64,
}

impl LedgerWriter {
    /// Create a new ledger writer at the given path
    ///
    /// This creates the legacy format file at `path`.
    /// To enable full event sourcing, call `enable_event_logging()`.
    /// To enable order logging, call `enable_order_logging()`.
    pub fn new(path: &str) -> Self {
        let file = File::create(path).expect("Failed to create ledger file");
        // Use 1MB buffer to reduce syscalls for large datasets (e.g., 1M orders)
        let mut writer = BufWriter::with_capacity(1024 * 1024, file);

        // Legacy header: trade_id,user_id,asset_id,op,delta,balance_after
        writeln!(writer, "trade_id,user_id,asset_id,op,delta,balance_after").unwrap();

        LedgerWriter {
            legacy_writer: writer,
            legacy_count: 0,
            event_writer: None,
            event_count: 0,
            order_writer: None,
            order_count: 0,
        }
    }

    /// Enable full event logging to a separate file
    ///
    /// This creates a new file for BalanceEvent records with all operation types.
    pub fn enable_event_logging(&mut self, path: &str) {
        let file = File::create(path).expect("Failed to create event ledger file");
        let mut writer = BufWriter::new(file);

        // Write header for BalanceEvent format
        writeln!(writer, "{}", BalanceEvent::csv_header()).unwrap();

        self.event_writer = Some(writer);
    }

    /// Enable order event logging to a separate file
    pub fn enable_order_logging(&mut self, path: &str) {
        let file = File::create(path).expect("Failed to create order event file");
        let mut writer = BufWriter::new(file);

        // Write header for OrderEvent format
        writeln!(writer, "{}", OrderEvent::csv_header()).unwrap();

        self.order_writer = Some(writer);
    }

    /// Write a single legacy ledger entry (backward compatible)
    pub fn write_entry(&mut self, entry: &LedgerEntry) {
        writeln!(
            self.legacy_writer,
            "{},{},{},{},{},{}",
            entry.trade_id,
            entry.user_id,
            entry.asset_id,
            entry.op,
            entry.delta,
            entry.balance_after
        )
        .unwrap();
        self.legacy_count += 1;
    }

    /// Write a BalanceEvent (new format with full event sourcing)
    ///
    /// This records the event to the event log file if enabled.
    /// Also writes to legacy format for backward compatibility.
    pub fn write_balance_event(&mut self, event: &BalanceEvent) {
        // Write to new event log if enabled
        if let Some(ref mut writer) = self.event_writer {
            writeln!(writer, "{}", event.to_csv()).unwrap();
            self.event_count += 1;
        }
    }

    /// Write an OrderEvent
    pub fn write_order_event(&mut self, event: &OrderEvent) {
        if let Some(ref mut writer) = self.order_writer {
            writeln!(writer, "{}", event.to_csv()).unwrap();
            self.order_count += 1;
        }
    }

    /// Flush all buffered data to disk
    pub fn flush(&mut self) {
        self.legacy_writer.flush().unwrap();
        if let Some(ref mut writer) = self.event_writer {
            writer.flush().unwrap();
        }
        if let Some(ref mut writer) = self.order_writer {
            writer.flush().unwrap();
        }
    }

    /// Get total number of legacy entries written
    pub fn entry_count(&self) -> u64 {
        self.legacy_count
    }

    /// Get total number of events written (new format)
    pub fn event_count(&self) -> u64 {
        self.event_count
    }

    /// Get total number of order events written
    pub fn order_count(&self) -> u64 {
        self.order_count
    }

    /// Check if event logging is enabled
    pub fn event_logging_enabled(&self) -> bool {
        self.event_writer.is_some()
    }
}

impl Drop for LedgerWriter {
    fn drop(&mut self) {
        self.flush();
    }
}

// ============================================================
// TESTS
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::messages::{BalanceEventType, SourceType};
    use std::fs;

    #[test]
    fn test_legacy_ledger_entry() {
        let path = "/tmp/test_legacy_ledger.csv";
        {
            let mut ledger = LedgerWriter::new(path);
            ledger.write_entry(&LedgerEntry {
                trade_id: 1,
                user_id: 100,
                asset_id: 1,
                op: "credit",
                delta: 1000,
                balance_after: 11000,
            });
            assert_eq!(ledger.entry_count(), 1);
        }

        let content = fs::read_to_string(path).unwrap();
        assert!(content.contains("trade_id,user_id,asset_id,op,delta,balance_after"));
        assert!(content.contains("1,100,1,credit,1000,11000"));
        fs::remove_file(path).ok();
    }

    #[test]
    fn test_balance_event_logging() {
        let legacy_path = "/tmp/test_event_legacy.csv";
        let event_path = "/tmp/test_event_log.csv";
        {
            let mut ledger = LedgerWriter::new(legacy_path);
            ledger.enable_event_logging(event_path);

            let event = BalanceEvent::new(
                100,                    // user_id
                1,                      // asset_id
                BalanceEventType::Lock, // event_type
                5,                      // version
                SourceType::Order,      // source_type
                42,                     // source_id
                -1000,                  // delta
                9000,                   // avail_after
                1000,                   // frozen_after
            );
            ledger.write_balance_event(&event);
            assert_eq!(ledger.event_count(), 1);
        }

        let content = fs::read_to_string(event_path).unwrap();
        assert!(content.contains(BalanceEvent::csv_header()));
        assert!(content.contains("100,1,lock,5,order,42,-1000,9000,1000"));
        fs::remove_file(legacy_path).ok();
        fs::remove_file(event_path).ok();
    }
}
