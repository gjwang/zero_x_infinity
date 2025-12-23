# Performance Report

<h3>
  <a href="#-english">🇺🇸 English</a>
  &nbsp;&nbsp;&nbsp;|&nbsp;&nbsp;&nbsp;
  <a href="#-chinese">🇨🇳 中文</a>
</h3>

<div id="-english"></div>

## 🇺🇸 English

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

## Verdict

✅ **Correctness Verified**: ST and MT produce identical results.

📊 **Bottleneck**: Matching Engine (76.6% time).

<br>
<div align="right"><a href="#-english">↑ Back to Top</a></div>
<br>

---

<div id="-chinese"></div>

## 🇨🇳 中文

**生成时间**: 2025-12-18 22:09
**分支**: `0x08-h-performance-monitoring`
**数据集**: 1.3M 订单 (30% 撤单)

## 总结

| 指标 | 单线程 (Single-Thread) | 多线程 (Multi-Thread) | 备注 |
|------|------------------------|-----------------------|------|
| 订单数 | 1,300,000 | 1,300,000 | - |
| 成交数 | 667,567 | 667,567 | ✅ 完全匹配 |
| 执行时间 | 14.18s | 20.17s | - |
| 吞吐量 | 91,710/s | 64,450/s | 多线程有队列开销 |

## 结论

✅ **正确性验证**: 单线程与多线程结果一致。

📊 **瓶颈**: 撮合引擎 (Matching Engine) 占用 76.6% 时间。
