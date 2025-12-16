//! Ledger - Settlement audit log
//!
//! Records every balance change for complete auditability.

use std::fs::File;
use std::io::Write;

/// Ledger entry for settlement audit
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

/// Writes ledger entries to CSV file
pub struct LedgerWriter {
    file: File,
    entry_count: u64,
}

impl LedgerWriter {
    /// Create a new ledger writer at the given path
    pub fn new(path: &str) -> Self {
        let mut file = File::create(path).expect("Failed to create ledger file");
        // Header: trade_id,user_id,asset_id,op,delta,balance_after
        writeln!(file, "trade_id,user_id,asset_id,op,delta,balance_after").unwrap();

        LedgerWriter {
            file,
            entry_count: 0,
        }
    }

    /// Write a single ledger entry
    pub fn write_entry(&mut self, entry: &LedgerEntry) {
        writeln!(
            self.file,
            "{},{},{},{},{},{}",
            entry.trade_id,
            entry.user_id,
            entry.asset_id,
            entry.op,
            entry.delta,
            entry.balance_after
        )
        .unwrap();
        self.entry_count += 1;
    }

    /// Get total number of entries written
    pub fn entry_count(&self) -> u64 {
        self.entry_count
    }
}
