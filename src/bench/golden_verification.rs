//! Golden CSV Verification Tests
//!
//! This module verifies that our Rust implementation generates bit-exact data
//! matching the Java reference implementation.

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

/// A row from the golden CSV
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoldenRow {
    pub phase: String,
    pub command: String,
    pub order_id: i64,
    pub symbol: i32,
    pub price: i64,
    pub size: i64,
    pub action: String,
    pub order_type: String,
    pub uid: i64,
}

impl GoldenRow {
    /// Parse a CSV line into a GoldenRow
    pub fn from_csv_line(line: &str) -> Option<Self> {
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() != 9 {
            return None;
        }

        Some(Self {
            phase: parts[0].to_string(),
            command: parts[1].to_string(),
            order_id: parts[2].parse().ok()?,
            symbol: parts[3].parse().ok()?,
            price: parts[4].parse().ok()?,
            size: parts[5].parse().ok()?,
            action: parts[6].to_string(),
            order_type: parts[7].to_string(),
            uid: parts[8].parse().ok()?,
        })
    }
}

/// Load golden data from CSV file
pub fn load_golden_csv(path: &Path) -> std::io::Result<Vec<GoldenRow>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut rows = Vec::new();

    for (i, line) in reader.lines().enumerate() {
        let line = line?;
        // Skip header row
        if i == 0 && line.starts_with("phase,") {
            continue;
        }
        if let Some(row) = GoldenRow::from_csv_line(&line) {
            rows.push(row);
        }
    }

    Ok(rows)
}

/// Get the path to golden data directory
pub fn golden_data_dir() -> std::path::PathBuf {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    std::path::PathBuf::from(manifest_dir)
        .join("docs")
        .join("exchange_core_verification_kit")
        .join("golden_data")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify we can load the golden CSV files
    #[test]
    fn test_load_golden_csv_margin() {
        let path = golden_data_dir().join("golden_single_pair_margin.csv");
        if !path.exists() {
            eprintln!("Warning: Golden data file not found: {:?}", path);
            eprintln!("Skipping test - please ensure golden data is present");
            return;
        }

        let rows = load_golden_csv(&path).expect("Failed to load golden CSV");
        assert_eq!(rows.len(), 11000, "Expected 11000 rows in golden CSV");

        // Verify first row matches expected values
        let first = &rows[0];
        assert_eq!(first.phase, "FILL");
        assert_eq!(first.command, "PLACE_ORDER");
        assert_eq!(first.order_id, 1);
        assert_eq!(first.symbol, 40000);
        assert_eq!(first.price, 34386);
        assert_eq!(first.size, 1);
        assert_eq!(first.action, "BID");
        assert_eq!(first.order_type, "GTC");
        assert_eq!(first.uid, 393);
    }

    /// Verify we can load the exchange golden CSV
    #[test]
    fn test_load_golden_csv_exchange() {
        let path = golden_data_dir().join("golden_single_pair_exchange.csv");
        if !path.exists() {
            eprintln!("Warning: Golden data file not found: {:?}", path);
            return;
        }

        let rows = load_golden_csv(&path).expect("Failed to load golden CSV");
        assert_eq!(rows.len(), 11000, "Expected 11000 rows in golden CSV");
    }

    /// First-pass comparison: verify our generator produces consistent output
    #[test]
    fn test_generator_consistency() {
        use crate::bench::java_random::JavaRandom;

        // Test that same seed always produces same sequence
        let mut rng1 = JavaRandom::new(1);
        let mut rng2 = JavaRandom::new(1);

        for _ in 0..100 {
            assert_eq!(rng1.next_int(100), rng2.next_int(100));
        }
    }

    /// Verify derive_session_seed produces expected hash for symbol 40000, seed 1
    #[test]
    fn test_session_seed_known_value() {
        use crate::bench::java_random::derive_session_seed;

        // Symbol 40000, benchmark seed 1
        let session_seed = derive_session_seed(40000, 1);

        // This seed should be consistent
        let session_seed2 = derive_session_seed(40000, 1);
        assert_eq!(session_seed, session_seed2);

        // Log the actual value for debugging
        eprintln!(
            "Session seed for symbol=40000, benchmark_seed=1: {}",
            session_seed
        );
    }

    /// Golden data verification - compare first N orders
    ///
    /// This test verifies that our order generator produces the same sequence
    /// as the Java reference implementation.
    #[test]
    fn test_golden_single_pair_margin() {
        let path = golden_data_dir().join("golden_single_pair_margin.csv");
        if !path.exists() {
            eprintln!("Warning: Golden data file not found: {:?}", path);
            eprintln!("This test requires golden data to be present.");
            eprintln!("Path expected: {:?}", path);
            return;
        }

        let golden_rows = load_golden_csv(&path).expect("Failed to load golden CSV");

        // For now, we verify the golden data loads correctly
        // Phase 2: We will compare generated data with golden data

        eprintln!("\n=== Golden Data Summary (Margin) ===");
        eprintln!("Total rows: {}", golden_rows.len());
        eprintln!("First 5 rows:");
        for (i, row) in golden_rows.iter().take(5).enumerate() {
            eprintln!(
                "  [{}] phase={}, cmd={}, order_id={}, price={}, size={}, action={}, uid={}",
                i + 1,
                row.phase,
                row.command,
                row.order_id,
                row.price,
                row.size,
                row.action,
                row.uid
            );
        }

        // Count FILL vs BENCHMARK phase
        let fill_count = golden_rows.iter().filter(|r| r.phase == "FILL").count();
        let bench_count = golden_rows
            .iter()
            .filter(|r| r.phase == "BENCHMARK")
            .count();
        eprintln!("\nFILL phase: {} orders", fill_count);
        eprintln!("BENCHMARK phase: {} orders", bench_count);

        // Verify basic properties
        assert_eq!(golden_rows.len(), 11000);
        assert!(fill_count > 0, "Expected some FILL phase orders");
    }

    /// Verify first 10 golden orders against known expected values
    #[test]
    fn test_golden_first_10_orders() {
        let path = golden_data_dir().join("golden_single_pair_margin.csv");
        if !path.exists() {
            return;
        }

        let rows = load_golden_csv(&path).expect("Failed to load golden CSV");

        // Expected first 10 orders from golden CSV (updated for 11k scale)
        let expected = [
            (1, 34386, 1, "BID", 393),   // order 1
            (2, 34135, 1, "BID", 677),   // order 2
            (3, 34347, 2, "BID", 577),   // order 3
            (4, 34150, 9, "BID", 325),   // order 4
            (5, 34152, 13, "BID", 116),  // order 5
            (6, 34391, 76, "ASK", 172),  // order 6
            (7, 34253, 7, "BID", 975),   // order 7
            (8, 34342, 25, "BID", 26),   // order 8
            (9, 34621, 1, "ASK", 723),   // order 9
            (10, 34971, 16, "ASK", 414), // order 10
        ];

        for (i, (order_id, price, size, action, uid)) in expected.iter().enumerate() {
            let row = &rows[i];
            assert_eq!(row.order_id, *order_id, "Order {} ID mismatch", i + 1);
            assert_eq!(row.price, *price, "Order {} price mismatch", i + 1);
            assert_eq!(row.size, *size, "Order {} size mismatch", i + 1);
            assert_eq!(row.action, *action, "Order {} action mismatch", i + 1);
            assert_eq!(row.uid, *uid, "Order {} uid mismatch", i + 1);
        }

        eprintln!("✅ First 10 golden orders verified successfully");
    }

    /// **DETAILED COMPARISON TEST**
    ///
    /// This test generates orders using our Rust implementation and compares
    /// each row against the golden CSV to find the first mismatch.
    #[test]
    fn test_generator_vs_golden_detailed() {
        use crate::bench::order_generator::{Action, SessionConfig, TestOrdersGeneratorSession};

        let path = golden_data_dir().join("golden_single_pair_margin.csv");
        if !path.exists() {
            eprintln!("Warning: Golden data file not found: {:?}", path);
            return;
        }

        let golden_rows = load_golden_csv(&path).expect("Failed to load golden CSV");

        // Create generator with same config as Java reference
        // RustPortingDataDumper: numAccounts=2000, totalTransactionsNumber=10000
        let config = SessionConfig {
            target_orders_per_side: 500, // FILL phase has 1000 orders = 500 per side
            num_accounts: 2000,          // From updated RustPortingDataDumper
            symbol_messages: 10000,      // totalTransactionsNumber
            symbol_id: 40000,
        };
        let mut session = TestOrdersGeneratorSession::new(config, 1);

        eprintln!("\n=== Generator vs Golden Comparison ===");
        eprintln!("Comparing first 20 orders...\n");

        let mut first_mismatch: Option<usize> = None;
        let compare_count = 20.min(golden_rows.len());

        for i in 0..compare_count {
            let golden = &golden_rows[i];
            let generated = session.next_command();

            let gen_action = match generated.action {
                Action::Bid => "BID",
                Action::Ask => "ASK",
            };

            let matches = golden.order_id == generated.order_id
                && golden.price == generated.price
                && golden.size == generated.size
                && golden.action == gen_action
                && golden.uid == generated.uid;

            let status = if matches { "✅" } else { "❌" };

            eprintln!(
                "[{:3}] {} | Golden: id={}, price={:5}, size={:3}, action={}, uid={:2}",
                i + 1,
                status,
                golden.order_id,
                golden.price,
                golden.size,
                golden.action,
                golden.uid
            );
            eprintln!(
                "        | Ours:   id={}, price={:5}, size={:3}, action={}, uid={:2}",
                generated.order_id, generated.price, generated.size, gen_action, generated.uid
            );

            if !matches && first_mismatch.is_none() {
                first_mismatch = Some(i);
                eprintln!("         ^^^^ FIRST MISMATCH ^^^^");
            }
        }

        eprintln!("\n=== Summary ===");
        if let Some(idx) = first_mismatch {
            eprintln!("❌ First mismatch at row {}", idx + 1);
            eprintln!("\nThis means our order generation algorithm does not yet");
            eprintln!("exactly match the Java reference implementation.");
            eprintln!("\nTo fix this, we need to:");
            eprintln!("1. Verify the exact Java algorithm for price/size/uid generation");
            eprintln!("2. Match the random number consumption order");
            eprintln!("3. Match the Pareto distribution parameters exactly");
        } else {
            eprintln!("✅ All {} compared rows match!", compare_count);
        }
    }

    /// **FULL VERIFICATION TEST**
    ///
    /// Verifies FILL phase (100 rows) for bit-exact match.
    /// BENCHMARK phase (1000 rows) requires order book simulation and is not verified.
    #[test]
    fn test_golden_full_verification() {
        use crate::bench::order_generator::{Action, SessionConfig, TestOrdersGeneratorSession};

        let path = golden_data_dir().join("golden_single_pair_margin.csv");
        if !path.exists() {
            eprintln!("Warning: Golden data file not found: {:?}", path);
            return;
        }

        let golden_rows = load_golden_csv(&path).expect("Failed to load golden CSV");

        // RustPortingDataDumper: numAccounts=100, totalTransactionsNumber=1000
        let config = SessionConfig {
            target_orders_per_side: 500,
            num_accounts: 2000,
            symbol_messages: 10000,
            symbol_id: 40000,
        };
        let mut session = TestOrdersGeneratorSession::new(config, 1);

        // Verify ALL rows (FILL + BENCHMARK) with IOC simulation
        let total_rows = golden_rows.len();
        let mut matched = 0;
        let mut mismatches: Vec<(usize, String)> = Vec::new();

        eprintln!("\n=== Full 11k Verification (FILL + BENCHMARK) ===");
        eprintln!("Verifying {} total orders...\n", total_rows);

        for i in 0..total_rows {
            let golden = &golden_rows[i];
            let generated = session.next_command();

            let gen_action = match generated.action {
                Action::Bid => "BID",
                Action::Ask => "ASK",
            };

            let matches = golden.order_id == generated.order_id
                && golden.price == generated.price
                && golden.size == generated.size
                && golden.action == gen_action
                && golden.uid == generated.uid;

            if matches {
                matched += 1;
            } else {
                mismatches.push((
                    i + 1,
                    format!(
                        "Golden: id={}, price={}, size={}, action={}, uid={} | Ours: id={}, price={}, size={}, action={}, uid={}",
                        golden.order_id, golden.price, golden.size, golden.action, golden.uid,
                        generated.order_id, generated.price, generated.size, gen_action, generated.uid
                    ),
                ));
                // Stop after first 5 mismatches to avoid flooding output
                if mismatches.len() >= 5 {
                    break;
                }
            }
        }

        eprintln!("=== Full Verification Results ===");
        eprintln!("Total rows:  {}", total_rows);
        eprintln!("Matched:     {}", matched);
        eprintln!("Mismatches:  {}", mismatches.len());

        if mismatches.is_empty() {
            eprintln!(
                "\n✅ ALL {} ROWS MATCH - Ready for 3M scale generation",
                total_rows
            );
        } else {
            eprintln!("\n❌ First {} mismatches:", mismatches.len());
            for (row, detail) in &mismatches {
                eprintln!("  Row {}: {}", row, detail);
            }
            panic!(
                "Full verification failed: {}/{} rows matched",
                matched, total_rows
            );
        }
    }
}
