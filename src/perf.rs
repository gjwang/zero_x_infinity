//! Performance Metrics - Timing breakdown and latency sampling
//!
//! Collects detailed performance data for analysis and regression detection.

/// Performance metrics for execution analysis
/// Collects timing breakdown and latency samples for percentile calculation
#[derive(Default)]
pub struct PerfMetrics {
    // Timing breakdown (nanoseconds)
    pub total_balance_check_ns: u64, // Account lookup + balance check + lock
    pub total_matching_ns: u64,      // OrderBook.add_order()
    pub total_settlement_ns: u64,    // Balance updates after trade
    pub total_ledger_ns: u64,        // Ledger file I/O

    // Per-order latency samples (nanoseconds)
    // We sample every Nth order to keep memory bounded
    pub latency_samples: Vec<u64>,
    sample_rate: usize,
    sample_counter: usize,
}

impl PerfMetrics {
    /// Create new metrics collector with given sample rate
    ///
    /// # Arguments
    /// * `sample_rate` - Sample every Nth order for latency percentiles
    pub fn new(sample_rate: usize) -> Self {
        PerfMetrics {
            sample_rate,
            latency_samples: Vec::with_capacity(10_000),
            ..Default::default()
        }
    }

    /// Record per-order latency (sampled)
    #[inline]
    pub fn add_order_latency(&mut self, latency_ns: u64) {
        self.sample_counter += 1;
        if self.sample_counter >= self.sample_rate {
            self.latency_samples.push(latency_ns);
            self.sample_counter = 0;
        }
    }

    /// Add time spent on balance check
    #[inline]
    pub fn add_balance_check_time(&mut self, ns: u64) {
        self.total_balance_check_ns += ns;
    }

    /// Add time spent on matching
    #[inline]
    pub fn add_matching_time(&mut self, ns: u64) {
        self.total_matching_ns += ns;
    }

    /// Add time spent on settlement
    #[inline]
    pub fn add_settlement_time(&mut self, ns: u64) {
        self.total_settlement_ns += ns;
    }

    /// Add time spent on ledger I/O
    #[inline]
    pub fn add_ledger_time(&mut self, ns: u64) {
        self.total_ledger_ns += ns;
    }

    /// Calculate percentile from samples
    ///
    /// # Arguments
    /// * `p` - Percentile (0-100), e.g., 50.0 for median, 99.0 for P99
    pub fn percentile(&self, p: f64) -> Option<u64> {
        if self.latency_samples.is_empty() {
            return None;
        }
        let mut sorted = self.latency_samples.clone();
        sorted.sort_unstable();
        let idx = ((p / 100.0) * (sorted.len() - 1) as f64).round() as usize;
        Some(sorted[idx.min(sorted.len() - 1)])
    }

    /// Get minimum latency
    pub fn min_latency(&self) -> Option<u64> {
        self.latency_samples.iter().copied().min()
    }

    /// Get maximum latency
    pub fn max_latency(&self) -> Option<u64> {
        self.latency_samples.iter().copied().max()
    }

    /// Get average latency
    pub fn avg_latency(&self) -> Option<u64> {
        if self.latency_samples.is_empty() {
            return None;
        }
        Some(self.latency_samples.iter().sum::<u64>() / self.latency_samples.len() as u64)
    }

    /// Get total tracked time (sum of all components)
    pub fn total_tracked_ns(&self) -> u64 {
        self.total_balance_check_ns
            + self.total_matching_ns
            + self.total_settlement_ns
            + self.total_ledger_ns
    }

    /// Get percentage breakdown
    pub fn breakdown_pct(&self) -> (f64, f64, f64, f64) {
        let total = self.total_tracked_ns() as f64;
        if total == 0.0 {
            return (0.0, 0.0, 0.0, 0.0);
        }
        (
            self.total_balance_check_ns as f64 / total * 100.0,
            self.total_matching_ns as f64 / total * 100.0,
            self.total_settlement_ns as f64 / total * 100.0,
            self.total_ledger_ns as f64 / total * 100.0,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_percentile() {
        let mut perf = PerfMetrics::new(1); // Sample every order
        for i in 1..=100 {
            perf.add_order_latency(i);
        }

        assert_eq!(perf.min_latency(), Some(1));
        assert_eq!(perf.max_latency(), Some(100));
        // P50 of 1..100 with this formula rounds to 51 (50.5 rounded)
        let p50 = perf.percentile(50.0).unwrap();
        assert!(p50 == 50 || p50 == 51, "P50 should be ~50, got {}", p50);
        assert_eq!(perf.percentile(99.0), Some(99));
    }

    #[test]
    fn test_breakdown() {
        let mut perf = PerfMetrics::new(1);
        perf.add_balance_check_time(100);
        perf.add_matching_time(200);
        perf.add_settlement_time(100);
        perf.add_ledger_time(600);

        assert_eq!(perf.total_tracked_ns(), 1000);

        let (b, m, s, l) = perf.breakdown_pct();
        assert!((b - 10.0).abs() < 0.1);
        assert!((m - 20.0).abs() < 0.1);
        assert!((s - 10.0).abs() < 0.1);
        assert!((l - 60.0).abs() < 0.1);
    }
}
