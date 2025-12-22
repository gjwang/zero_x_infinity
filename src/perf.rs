//! Performance Metrics - Timing breakdown and latency sampling
//!
//! Profiling follows the order lifecycle architecture:
//! 1. UBSCore Pre-Trade (WAL + Lock)
//! 2. Matching Engine (pure matching)
//! 3. Settlement (balance updates)
//! 4. Event Logging (all writes)

/// Performance metrics for execution analysis
/// Collects timing breakdown and latency samples for percentile calculation
#[derive(Clone, Default, Debug)]
pub struct PerfMetrics {
    // ============================================
    // TOP-LEVEL ARCHITECTURE TIMING (nanoseconds)
    // ============================================
    /// 1. UBSCore Pre-Trade: WAL write + Balance Lock
    ///    (ubscore.process_order for place, book.remove for cancel)
    pub total_pretrade_ns: u64,

    /// 2. Matching Engine: Pure order matching
    ///    (MatchingEngine::process_order only)
    pub total_matching_ns: u64,

    /// 3. Settlement: Post-trade balance updates
    ///    (ubscore.settle_trade, ubscore.unlock for cancel)
    pub total_settlement_ns: u64,

    /// 4. Event Logging: All writes to ledger/events
    ///    (ledger.write_* calls)
    pub total_event_log_ns: u64,

    // ============================================
    // OPERATION COUNTS
    // ============================================
    pub place_count: u64,
    pub cancel_count: u64,
    pub trade_count: u64,

    // ============================================
    // SUB-BREAKDOWN (for deeper analysis)
    // ============================================
    /// WAL write time (part of pretrade)
    pub total_wal_ns: u64,

    /// Balance lock time (part of pretrade)
    pub total_lock_ns: u64,

    /// OrderBook lookup time (for cancel)
    pub total_cancel_lookup_ns: u64,

    // ============================================
    // LATENCY SAMPLING
    // ============================================
    pub latency_samples: Vec<u64>,
    sample_rate: usize,
    sample_counter: usize,
}

impl PerfMetrics {
    /// Create new metrics collector with given sample rate
    pub fn new(sample_rate: usize) -> Self {
        PerfMetrics {
            sample_rate,
            latency_samples: Vec::with_capacity(10_000),
            ..Default::default()
        }
    }

    // ============================================
    // TOP-LEVEL TIMING
    // ============================================

    #[inline]
    pub fn add_pretrade_time(&mut self, ns: u64) {
        self.total_pretrade_ns += ns;
    }

    #[inline]
    pub fn add_matching_time(&mut self, ns: u64) {
        self.total_matching_ns += ns;
    }

    #[inline]
    pub fn add_settlement_time(&mut self, ns: u64) {
        self.total_settlement_ns += ns;
    }

    #[inline]
    pub fn add_event_log_time(&mut self, ns: u64) {
        self.total_event_log_ns += ns;
    }

    // ============================================
    // OPERATION COUNTS
    // ============================================

    #[inline]
    pub fn inc_place(&mut self) {
        self.place_count += 1;
    }

    #[inline]
    pub fn inc_cancel(&mut self) {
        self.cancel_count += 1;
    }

    #[inline]
    pub fn add_trades(&mut self, count: u64) {
        self.trade_count += count;
    }

    // ============================================
    // SUB-BREAKDOWN
    // ============================================

    #[inline]
    pub fn add_wal_time(&mut self, ns: u64) {
        self.total_wal_ns += ns;
    }

    #[inline]
    pub fn add_lock_time(&mut self, ns: u64) {
        self.total_lock_ns += ns;
    }

    #[inline]
    pub fn add_cancel_lookup_time(&mut self, ns: u64) {
        self.total_cancel_lookup_ns += ns;
    }

    // ============================================
    // LATENCY SAMPLING
    // ============================================

    #[inline]
    pub fn add_order_latency(&mut self, latency_ns: u64) {
        self.sample_counter += 1;
        if self.sample_counter >= self.sample_rate {
            self.latency_samples.push(latency_ns);
            self.sample_counter = 0;
        }
    }

    /// Calculate percentile from samples
    pub fn percentile(&self, p: f64) -> Option<u64> {
        if self.latency_samples.is_empty() {
            return None;
        }
        let mut sorted = self.latency_samples.clone();
        sorted.sort_unstable();
        let idx = ((p / 100.0) * (sorted.len() - 1) as f64).round() as usize;
        Some(sorted[idx.min(sorted.len() - 1)])
    }

    pub fn min_latency(&self) -> Option<u64> {
        self.latency_samples.iter().copied().min()
    }

    pub fn max_latency(&self) -> Option<u64> {
        self.latency_samples.iter().copied().max()
    }

    pub fn avg_latency(&self) -> Option<u64> {
        if self.latency_samples.is_empty() {
            return None;
        }
        Some(self.latency_samples.iter().sum::<u64>() / self.latency_samples.len() as u64)
    }

    // ============================================
    // REPORTING
    // ============================================

    /// Get total tracked time
    pub fn total_tracked_ns(&self) -> u64 {
        self.total_pretrade_ns
            + self.total_matching_ns
            + self.total_settlement_ns
            + self.total_event_log_ns
    }

    /// Get architectural breakdown as formatted string
    pub fn breakdown(&self) -> String {
        let mut s = self.legacy_report();
        s.push_str("\n--- Markdown Analysis Table ---\n");
        s.push_str(&self.markdown_report());
        s
    }

    /// Legacy text-based report
    pub fn legacy_report(&self) -> String {
        let total = self.total_tracked_ns() as f64;
        if total == 0.0 {
            return "No timing data collected".to_string();
        }

        let pct = |v: u64| -> f64 { v as f64 / total * 100.0 };
        let ms = |v: u64| -> f64 { v as f64 / 1_000_000.0 };
        let us_per = |v: u64, count: u64| -> f64 {
            if count == 0 {
                0.0
            } else {
                v as f64 / 1000.0 / count as f64
            }
        };

        let total_orders = self.place_count + self.cancel_count;

        let mut s = String::new();

        // Header
        s.push_str(&format!(
            "Orders: {} (Place: {}, Cancel: {}), Trades: {}\n\n",
            total_orders, self.place_count, self.cancel_count, self.trade_count
        ));

        // Top-level breakdown
        s.push_str(&format!(
            "1. Pre-Trade:    {:>10.2}ms ({:>5.1}%)  [{:>6.2} µs/order]\n",
            ms(self.total_pretrade_ns),
            pct(self.total_pretrade_ns),
            us_per(self.total_pretrade_ns, total_orders)
        ));
        s.push_str(&format!(
            "2. Matching:     {:>10.2}ms ({:>5.1}%)  [{:>6.2} µs/order]\n",
            ms(self.total_matching_ns),
            pct(self.total_matching_ns),
            us_per(self.total_matching_ns, self.place_count)
        ));
        s.push_str(&format!(
            "3. Settlement:   {:>10.2}ms ({:>5.1}%)  [{:>6.2} µs/trade]\n",
            ms(self.total_settlement_ns),
            pct(self.total_settlement_ns),
            us_per(self.total_settlement_ns, self.trade_count)
        ));
        s.push_str(&format!(
            "4. Event Log:    {:>10.2}ms ({:>5.1}%)  [{:>6.2} µs/order]\n",
            ms(self.total_event_log_ns),
            pct(self.total_event_log_ns),
            us_per(self.total_event_log_ns, total_orders)
        ));

        s.push_str(&format!(
            "\nTotal Tracked:   {:>10.2}ms\n",
            ms(self.total_tracked_ns())
        ));

        // Latency Profile
        if !self.latency_samples.is_empty() {
            s.push_str("\n--- Latency Profile (sampled) ---\n");
            let p50 = self.percentile(50.0).unwrap_or(0);
            let p99 = self.percentile(99.0).unwrap_or(0);
            let max = self.max_latency().unwrap_or(0);

            s.push_str(&format!("  P50:    {:>10} ns\n", p50));
            s.push_str(&format!("  P99:    {:>10} ns\n", p99));
            s.push_str(&format!("  Max:    {:>10} ns\n", max));
        }

        s
    }

    /// Generate Markdown table for documentation
    pub fn markdown_report(&self) -> String {
        let total_ns = self.total_tracked_ns() as f64;
        if total_ns == 0.0 {
            return "| Service | Module | Task Time | Ratio | Latency | Throughput |\n| :--- | :--- | :--- | :--- | :--- | :--- |\n| N/A | N/A | 0 | 0% | 0 | 0 |".to_string();
        }

        let total_orders = self.place_count + self.cancel_count;

        let mut s = String::new();
        s.push_str("| Service | Module / Key Span | Total Time | Ratio | Latency | Throughput |\n");
        s.push_str("| :--- | :--- | :--- | :--- | :--- | :--- |\n");

        let mut add_row = |service: &str, module: &str, ns: u64, count: u64| {
            let sec = ns as f64 / 1_000_000_000.0;
            let ratio = ns as f64 / total_ns * 100.0;
            let latency_us = if count == 0 {
                0.0
            } else {
                ns as f64 / 1000.0 / count as f64
            };
            let throughput = if latency_us == 0.0 {
                0.0
            } else {
                1_000_000.0 / latency_us
            };

            let tp_str = if throughput > 1_000_000.0 {
                format!("{:.2}M ops/s", throughput / 1_000_000.0)
            } else {
                format!("{:.1}k ops/s", throughput / 1000.0)
            };

            s.push_str(&format!(
                "| **{}** | `{}` | {:.2}s | {:.1}% | {:.2} µs | {} |\n",
                service, module, sec, ratio, latency_us, tp_str
            ));
        };

        add_row(
            "UBSCore",
            "ORDER (Lock)",
            self.total_pretrade_ns,
            total_orders,
        );
        add_row(
            "UBSCore",
            "SETTLE (Upd)",
            self.total_settlement_ns,
            self.trade_count,
        );
        add_row(
            "Matching Engine",
            "ORDER (Match)",
            self.total_matching_ns,
            self.place_count,
        );
        add_row(
            "Persistence",
            "TRADE (Ledger)",
            self.total_event_log_ns,
            total_orders,
        );

        s
    }

    // Legacy compatibility
    pub fn add_balance_check_time(&mut self, ns: u64) {
        self.add_pretrade_time(ns);
    }

    pub fn add_ledger_time(&mut self, ns: u64) {
        self.add_event_log_time(ns);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_percentile() {
        let mut perf = PerfMetrics::new(1);
        for i in 1..=100 {
            perf.add_order_latency(i);
        }

        assert_eq!(perf.min_latency(), Some(1));
        assert_eq!(perf.max_latency(), Some(100));
        let p50 = perf.percentile(50.0).unwrap();
        assert!(p50 == 50 || p50 == 51, "P50 should be ~50, got {}", p50);
    }

    #[test]
    fn test_breakdown() {
        let mut perf = PerfMetrics::new(1);
        perf.add_pretrade_time(1_000_000); // 1ms
        perf.add_matching_time(2_000_000); // 2ms
        perf.add_settlement_time(1_000_000); // 1ms
        perf.add_event_log_time(6_000_000); // 6ms
        perf.place_count = 100;

        assert_eq!(perf.total_tracked_ns(), 10_000_000);

        let breakdown = perf.breakdown();
        assert!(breakdown.contains("Pre-Trade"));
        assert!(breakdown.contains("Matching"));
    }
}
