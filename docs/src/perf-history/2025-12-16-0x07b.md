# Performance Report

**Generated**: 2025-12-16 18:16:36

## Summary

| Metric | Baseline | Current | Change |
|--------|----------|---------|--------|
| Orders | 100,000 | 100,000 | - |
| Trades | 47,886 | 47,886 | - |
| Exec Time | 3753.87ms | 3956.64ms | +5.4% |
| Throughput | 26,639/s | 25,274/s | -5.1% |

## Timing Breakdown

| Component | Time | OPS | % of Total |
|-----------|------|-----|------------|
| Balance Check | 17.64ms | 5.7M | 0.4% |
| Matching Engine | 36.37ms | 2.7M | 0.9% |
| Settlement | 4.71ms | 21.2M | 0.1% |
| Ledger I/O | 3.88s | 26K | 98.5% |

## Latency Percentiles

| Percentile | Value |
|------------|-------|
| MIN | 125ns |
| AVG | 38.6µs |
| P50 | 625ns |
| P99 | 429.7µs |
| P99.9 | 1.37ms |
| MAX | 7.25ms |

## Verdict

❌ **2 regression(s) detected**

- Exec Time: +5.4%
- Throughput: -5.1%
