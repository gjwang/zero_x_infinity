# Performance Report

**Generated**: 2025-12-18 22:09
**Branch**: `0x08-h-performance-monitoring`
**Dataset**: 1.3M orders (30% cancels, high-balance mode)

## Summary

| Metric | Single-Thread | Multi-Thread | Notes |
|--------|---------------|--------------|-------|
| Orders | 1,300,000 | 1,300,000 | - |
| Trades | 667,567 | 667,567 | ✅ Exact match |
| Exec Time | 14.18s | 20.17s | - |
| Throughput | 91,710/s | 64,450/s | MT has queue overhead |
| P50 Latency | 2.5 µs | 113 ms | E2E vs per-order |

## Multi-Thread Timing Breakdown

| Component | Time | Latency/op | % of Total | Throughput |
|-----------|------|------------|------------|------------|
| Pre-Trade (Lock) | 0.00s | - | 0.0% | N/A |
| Matching Engine | 19.23s | 19.23 µs | 76.6% | 52.0k ops/s |
| Settlement (Upd) | 0.51s | 0.76 µs | 2.0% | 1.31M ops/s |
| Persistence | 5.35s | 4.12 µs | 21.3% | 242.9k ops/s |

## Latency Percentiles (Multi-Thread)

| Percentile | Value |
|------------|-------|
| MIN | 81 µs |
| AVG | 111 ms |
| P50 | 113 ms |
| P99 | 188 ms |
| P99.9 | 206 ms |
| MAX | 210 ms |

## Verdict

✅ **Correctness Verified**: ST and MT produce identical results (667,567 trades, 0 balance differences)

📊 **Bottleneck**: Matching Engine (76.6% of tracked time, 52k ops/s theoretical max)
