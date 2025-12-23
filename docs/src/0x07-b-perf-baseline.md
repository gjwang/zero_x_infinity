# 0x07-b Performance Baseline - Initial Setup

<h3>
  <a href="#-english">🇺🇸 English</a>
  &nbsp;&nbsp;&nbsp;|&nbsp;&nbsp;&nbsp;
  <a href="#-chinese">🇨🇳 中文</a>
</h3>

<div id="-english"></div>

## 🇺🇸 English

> **📦 Code Changes**: [View Diff](https://github.com/gjwang/zero_x_infinity/compare/v0.7-a-testing-framework...v0.7-b-perf-baseline)

> **Core Objective**: To establish a quantifiable, traceable, and comparable performance baseline.

Building on the testing framework from 0x07-a, this chapter adds detailed performance metric collection and analysis capabilities.

### 1. Why a Performance Baseline?

#### 1.1 The Performance Trap

Optimization without a baseline is blind:

*   **Premature Optimization**: Optimizing code that accounts for 1% of runtime.
*   **Delayed Regression Detection**: A refactor drops performance by 50%, but it's only discovered 3 months later.
*   **Unquantifiable Improvement**: Promoting "it's much faster," but exactly how much?

#### 1.2 Value of a Baseline

With a baseline, you can:

1.  **Verify before Commit**: Ensure performance hasn't degraded.
2.  **Pinpoint Bottlenecks**: Identify which component consumes the most time.
3.  **Quantify Optimization**: "Throughput increased from 30K ops/s to 100K ops/s."

### 2. Metric Design

#### 2.1 Throughput Metrics

| Metric | Explanation | Calculation |
|--------|-------------|-------------|
| `throughput_ops` | Order Throughput | orders / exec_time |
| `throughput_tps` | Trade Throughput | trades / exec_time |

#### 2.2 Time Breakdown

We decompose execution time into four components:

```
┌─────────────────────────────────────────────────────────────┐
│ Order Processing (per order)                                │
├─────────────────────────────────────────────────────────────┤
│ 1. Balance Check     │ Account lookup + balance validation  │
│    - Account lookup  │ FxHashMap O(1)                       │
│    - Fund locking    │ Check avail >= required, then lock   │
├─────────────────────────────────────────────────────────────┤
│ 2. Matching Engine   │ book.add_order()                     │
│    - Price lookup    │ BTreeMap O(log n)                    │
│    - Order matching  │ iterate + partial fill               │
├─────────────────────────────────────────────────────────────┤
│ 3. Settlement        │ settle_as_buyer/seller               │
│    - Balance update  │ HashMap O(1)                         │
├─────────────────────────────────────────────────────────────┤
│ 4. Ledger I/O        │ write_entry()                        │
│    - File write      │ Disk I/O                             │
└─────────────────────────────────────────────────────────────┘
```

#### 2.3 Latency Percentiles

Sample total processing latency every N orders:

| Percentile | Meaning |
|------------|---------|
| P50 | Median, typical case |
| P99 | 99% of requests are faster than this |
| P99.9 | Tail latency, worst cases |
| Max | Maximum latency |

### 3. Initial Baseline Data

#### 3.1 Test Environment

*   **Hardware**: MacBook Pro M Series
*   **Data**: 100,000 Orders, 47,886 Trades
*   **Mode**: Release build (`--release`)

#### 3.2 Throughput

```
Throughput: ~29,000 orders/sec | ~14,000 trades/sec
Execution Time: ~3.5s
```

#### 3.3 Time Breakdown 🔥

```
=== Performance Breakdown ===
Balance Check:       17.68ms (  0.5%)  ← FxHashMap O(1)
Matching Engine:     36.04ms (  1.0%)  ← Extremely Fast!
Settlement:           4.77ms (  0.1%)  ← Negligible
Ledger I/O:        3678.68ms ( 98.4%) ← Bottleneck!
```

**Key Findings**:
*   **Ledger I/O consumes 98.4% of time.**
*   Balance Check + Matching + Settlement total only ~58ms.
*   Theoretical Limit: ~1.7 Million orders/sec (without I/O).

#### 3.4 Order Lifecycle Timeline 📊

```
                           Order Lifecycle
    ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

    ┌─────────────┐    ┌─────────────┐    ┌─────────────┐    ┌─────────────┐
    │   Balance   │    │  Matching   │    │ Settlement  │    │  Ledger     │
    │   Check     │───▶│   Engine    │───▶│  (Balance)  │───▶│   I/O       │
    └─────────────┘    └─────────────┘    └─────────────┘    └─────────────┘
          │                  │                  │                  │
          ▼                  ▼                  ▼                  ▼
    ┌─────────────┐    ┌─────────────┐    ┌─────────────┐    ┌─────────────┐
    │ FxHashMap   │    │  BTreeMap   │    │Vec<Balance> │    │  File::     │
    │   +Vec O(1) │    │  O(log n)   │    │    O(1)     │    │  write()    │
    └─────────────┘    └─────────────┘    └─────────────┘    └─────────────┘

    ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    Total Time:   17.68ms            36.04ms            4.77ms          3678.68ms
    Percentage:    0.5%               1.0%              0.1%             98.4%
    ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    Per-Order:    0.18µs             0.36µs            0.05µs           36.79µs
    Potential:   5.6M ops/s         2.8M ops/s       20M ops/s         27K ops/s
    ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

                        Business Logic ~58ms (1.6%)        I/O ~3679ms (98.4%)
                    ◀─────────────────────────▶      ◀───────────────────────▶
                             Fast ✅                        Bottleneck 🔴
```

**Analysis**:

| Phase | Latency/Order | Theoretical OPS | Note |
|-------|---------------|-----------------|------|
| Balance Check | 0.18µs | 5.6M/s | FxHashMap Lookup + Vec O(1) |
| Matching Engine | 0.36µs | 2.8M/s | BTreeMap Price Matching |
| Settlement | 0.05µs | 20M/s | Vec\<Balance\> O(1) Indexing |
| **Ledger I/O** | **36.79µs** | **27K/s** | **Unbuffered File Write = Bottleneck!** |

**E2E Result**:
*   Actual Throughput: **~29K orders/sec** (I/O Bound)
*   Theoretical Limit (No I/O): **~1.7M orders/sec** (60x room for improvement!)

#### 3.5 Latency Percentiles

```
=== Latency Percentiles (sampled) ===
  Min:        125 ns
  Avg:      34022 ns
  P50:        583 ns   ← Typical order < 1µs
  P99:     391750 ns   ← 99% of orders < 0.4ms
  P99.9:  1243833 ns   ← Tail latency ~1.2ms
  Max:    3207875 ns   ← Worst case ~3ms
```

### 4. Output Files

#### 4.1 t2_perf.txt (Machine Readable)

```
# Performance Baseline - 0xInfinity
# Generated: 2025-12-16
orders=100000
trades=47886
exec_time_ms=3451.78
throughput_ops=28971
throughput_tps=13873
matching_ns=32739014
settlement_ns=3085409
ledger_ns=3388134698
latency_min_ns=125
latency_avg_ns=34022
latency_p50_ns=583
latency_p99_ns=391750
latency_p999_ns=1243833
latency_max_ns=3207875
```

#### 4.2 t2_summary.txt (Human Readable)

Contains full execution summary and performance breakdown.

### 5. PerfMetrics Implementation

```rust
/// Performance metrics for execution analysis
#[derive(Default)]
struct PerfMetrics {
    // Timing breakdown (nanoseconds)
    total_balance_check_ns: u64,  // Account lookup + balance check + lock
    total_matching_ns: u64,       // OrderBook.add_order()
    total_settlement_ns: u64,     // Balance updates after trade
    total_ledger_ns: u64,         // Ledger file I/O
    
    // Per-order latency samples
    latency_samples: Vec<u64>,
    sample_rate: usize,
}

impl PerfMetrics {
    fn new(sample_rate: usize) -> Self { ... }
    
    fn add_order_latency(&mut self, latency_ns: u64) { ... }
    fn add_balance_check_time(&mut self, ns: u64) { ... }
    fn add_matching_time(&mut self, ns: u64) { ... }
    fn add_settlement_time(&mut self, ns: u64) { ... }
    fn add_ledger_time(&mut self, ns: u64) { ... }
    
    fn percentile(&self, p: f64) -> Option<u64> { ... }
    fn min_latency(&self) -> Option<u64> { ... }
    fn max_latency(&self) -> Option<u64> { ... }
    fn avg_latency(&self) -> Option<u64> { ... }
}
```

### 6. Optimization Roadmap

Based on baseline data, future directions:

#### 6.1 Short Term (0x07-c)

| Optimization | Expected Gain | Difficulty |
|--------------|---------------|------------|
| Use BufWriter | 10-50x I/O | Low |
| Batch Write | 2-5x | Low |

#### 6.2 Mid Term (0x08+)

| Optimization | Expected Gain | Difficulty |
|--------------|---------------|------------|
| Async I/O | Decouple Matching & Persistence | Medium |
| Memory Pool | Reduce Allocation | Medium |

#### 6.3 Long Term

| Optimization | Expected Gain | Difficulty |
|--------------|---------------|------------|
| DPDK/io_uring | 10x+ | High |
| FPGA | 100x+ | Extreme |

### 7. Commands Reference

```bash
# Run and generate performance data
cargo run --release

# Update baseline (when code changes)
cargo run --release -- --baseline

# View performance data
cat output/t2_perf.txt

# Compare performance changes
python3 scripts/compare_perf.py
```

#### compare_perf.py Output Example

```
╔════════════════════════════════════════════════════════════════════════╗
║                    Performance Comparison Report                       ║
╚════════════════════════════════════════════════════════════════════════╝

Metric                           Baseline         Current       Change
───────────────────────────────────────────────────────────────────────────
Orders                             100000          100000            -
Trades                              47886           47886            -

Exec Time                       3753.87ms       3484.37ms        -7.2%
Throughput (orders)               26639/s         28700/s        +7.7%
Throughput (trades)               12756/s         13743/s        +7.7%

───────────────────────────────────────────────────────────────────────────
Timing Breakdown (lower is better):

Metric                           Baseline         Current     Change        OPS
Balance Check                     17.68ms         16.51ms      -6.6%       6.1M
Matching Engine                   36.04ms         35.01ms      -2.8%       2.9M
Settlement                         4.77ms          5.22ms      +9.4%      19.2M
Ledger I/O                      3678.68ms       3411.49ms      -7.3%        29K

───────────────────────────────────────────────────────────────────────────
Latency Percentiles (lower is better):

Metric                           Baseline         Current       Change
Latency MIN                         125ns           125ns        +0.0%
Latency AVG                        37.9µs          34.8µs        -8.2%
Latency P50                         584ns           541ns        -7.4%
Latency P99                       420.2µs         398.9µs        -5.1%
Latency P99.9                      1.63ms          1.24ms       -24.3%
Latency MAX                        9.76ms          3.53ms       -63.9%

───────────────────────────────────────────────────────────────────────────
✅ No significant regressions detected
```

### Summary

This chapter accomplished:

1.  **PerfMetrics Structure**: Collecting time breakdown & latency samples.
2.  **Time Breakdown**: Balance Check / Matching / Settlement / Ledger I/O.
3.  **Latency Percentiles**: P50 / P99 / P99.9 / Max.
4.  **t2_perf.txt**: Machine-readable baseline file.
5.  **compare_perf.py**: Tool to detect regression.
6.  **Key Finding**: Ledger I/O takes 98.4%, major bottleneck.

<br>
<div align="right"><a href="#-english">↑ Back to Top</a></div>
<br>

---

<div id="-chinese"></div>

## 🇨🇳 中文

> **📦 代码变更**: [查看 Diff](https://github.com/gjwang/zero_x_infinity/compare/v0.7-a-testing-framework...v0.7-b-perf-baseline)

> **核心目的**：建立可量化、可追踪、可比较的性能基线。

本章在 0x07-a 测试框架基础上，添加详细的性能指标收集和分析能力。

### 1. 为什么需要性能基线？

#### 1.1 性能陷阱

没有基线的优化是盲目的：

- **过早优化**：优化了占 1% 时间的代码
- **回归发现延迟**：某次重构导致性能下降 50%，但 3 个月后才发现
- **无法量化改进**：说"快了很多"，但具体快了多少？

#### 1.2 基线的价值

有了基线，你可以：

1. **每次提交前验证**：性能没有下降
2. **精确定位瓶颈**：哪个组件消耗最多时间
3. **量化优化效果**：从 30K ops/s 提升到 100K ops/s

### 2. 性能指标设计

#### 2.1 吞吐量指标

| 指标 | 说明 | 计算方式 |
|------|------|----------|
| `throughput_ops` | 订单吞吐量 | orders / exec_time |
| `throughput_tps` | 成交吞吐量 | trades / exec_time |

#### 2.2 时间分解

我们将执行时间分解为四个组件：

```
┌─────────────────────────────────────────────────────────────┐
│ Order Processing (per order)                                │
├─────────────────────────────────────────────────────────────┤
│ 1. Balance Check     │ Account lookup + balance validation  │
│    - Account lookup  │ FxHashMap O(1)                       │
│    - Fund locking    │ Check avail >= required, then lock   │
├─────────────────────────────────────────────────────────────┤
│ 2. Matching Engine   │ book.add_order()                     │
│    - Price lookup    │ BTreeMap O(log n)                    │
│    - Order matching  │ iterate + partial fill               │
├─────────────────────────────────────────────────────────────┤
│ 3. Settlement        │ settle_as_buyer/seller               │
│    - Balance update  │ HashMap O(1)                         │
├─────────────────────────────────────────────────────────────┤
│ 4. Ledger I/O        │ write_entry()                        │
│    - File write      │ Disk I/O                             │
└─────────────────────────────────────────────────────────────┘
```

#### 2.3 延迟百分位数

采样每 N 个订单的总处理延迟，计算：

| 百分位数 | 含义 |
|----------|------|
| P50 | 中位数，典型情况 |
| P99 | 99% 的请求低于此值 |
| P99.9 | 尾延迟，最坏情况 |
| Max | 最大延迟 |

### 3. 初始基线数据

#### 3.1 测试环境

- **硬件**：MacBook Pro M 系列
- **数据**：100,000 订单，47,886 成交
- **模式**：Release build (`--release`)

#### 3.2 吞吐量

```
Throughput: ~29,000 orders/sec | ~14,000 trades/sec
Execution Time: ~3.5s
```

#### 3.3 时间分解 🔥

```
=== Performance Breakdown ===
Balance Check:       17.68ms (  0.5%)  ← FxHashMap O(1)
Matching Engine:     36.04ms (  1.0%)  ← 极快！
Settlement:           4.77ms (  0.1%)  ← 几乎可忽略
Ledger I/O:        3678.68ms ( 98.4%) ← 瓶颈！
```

**关键发现**：
- **Ledger I/O 占用 98.4% 的时间**
- Balance Check + Matching + Settlement 总共只需 ~58ms
- 理论上限：~170 万 orders/sec（如果没有 I/O）

#### 3.4 订单生命周期性能时间线 📊

```
                           订单生命周期 (Order Lifecycle)
    ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

    ┌─────────────┐    ┌─────────────┐    ┌─────────────┐    ┌─────────────┐
    │   Balance   │    │  Matching   │    │ Settlement  │    │  Ledger     │
    │   Check     │───▶│   Engine    │───▶│  (Balance)  │───▶│   I/O       │
    └─────────────┘    └─────────────┘    └─────────────┘    └─────────────┘
          │                  │                  │                  │
          ▼                  ▼                  ▼                  ▼
    ┌─────────────┐    ┌─────────────┐    ┌─────────────┐    ┌─────────────┐
    │ FxHashMap   │    │  BTreeMap   │    │Vec<Balance> │    │  File::     │
    │   +Vec O(1) │    │  O(log n)   │    │    O(1)     │    │  write()    │
    └─────────────┘    └─────────────┘    └─────────────┘    └─────────────┘

    ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    Total Time:   17.68ms            36.04ms            4.77ms          3678.68ms
    Percentage:    0.5%               1.0%              0.1%             98.4%
    ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    Per-Order:    0.18µs             0.36µs            0.05µs           36.79µs
    Potential:   5.6M ops/s         2.8M ops/s       20M ops/s         27K ops/s
    ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

                        业务逻辑 ~58ms (1.6%)              I/O ~3679ms (98.4%)
                    ◀─────────────────────────▶      ◀───────────────────────▶
                             极快 ✅                        瓶颈 🔴
```

**性能分析**:

| 阶段 | 每订单延迟 | 理论 OPS | 说明 |
|------|-----------|----------|------|
| Balance Check | 0.18µs | 5.6M/s | FxHashMap 账户查找 + Vec O(1) 余额索引 |
| Matching Engine | 0.36µs | 2.8M/s | BTreeMap 价格匹配 |
| Settlement | 0.05µs | 20M/s | Vec\<Balance\> O(1) 直接索引 |
| **Ledger I/O** | **36.79µs** | **27K/s** | **unbuffered 文件写入 = 瓶颈！** |

**E2E 结果**:
- 实际吞吐量: **~29K orders/sec** (受限于 Ledger I/O)
- 理论上限 (无 I/O): **~1.7M orders/sec** (60x 提升空间!)

#### 3.5 延迟百分位数

```
=== Latency Percentiles (sampled) ===
  Min:        125 ns
  Avg:      34022 ns
  P50:        583 ns   ← 典型订单 < 1µs
  P99:     391750 ns   ← 99% 的订单 < 0.4ms
  P99.9:  1243833 ns   ← 尾延迟 ~1.2ms
  Max:    3207875 ns   ← 最坏 ~3ms
```

### 4. 输出文件

#### 4.1 t2_perf.txt（机器可读）

```
# Performance Baseline - 0xInfinity
# Generated: 2025-12-16
orders=100000
trades=47886
exec_time_ms=3451.78
throughput_ops=28971
throughput_tps=13873
matching_ns=32739014
settlement_ns=3085409
ledger_ns=3388134698
latency_min_ns=125
latency_avg_ns=34022
latency_p50_ns=583
latency_p99_ns=391750
latency_p999_ns=1243833
latency_max_ns=3207875
```

#### 4.2 t2_summary.txt（人类可读）

包含完整的执行摘要和性能分解。

### 5. PerfMetrics 实现

```rust
/// Performance metrics for execution analysis
#[derive(Default)]
struct PerfMetrics {
    // Timing breakdown (nanoseconds)
    total_balance_check_ns: u64,  // Account lookup + balance check + lock
    total_matching_ns: u64,       // OrderBook.add_order()
    total_settlement_ns: u64,     // Balance updates after trade
    total_ledger_ns: u64,         // Ledger file I/O
    
    // Per-order latency samples
    latency_samples: Vec<u64>,
    sample_rate: usize,
}

impl PerfMetrics {
    fn new(sample_rate: usize) -> Self { ... }
    
    fn add_order_latency(&mut self, latency_ns: u64) { ... }
    fn add_balance_check_time(&mut self, ns: u64) { ... }
    fn add_matching_time(&mut self, ns: u64) { ... }
    fn add_settlement_time(&mut self, ns: u64) { ... }
    fn add_ledger_time(&mut self, ns: u64) { ... }
    
    fn percentile(&self, p: f64) -> Option<u64> { ... }
    fn min_latency(&self) -> Option<u64> { ... }
    fn max_latency(&self) -> Option<u64> { ... }
    fn avg_latency(&self) -> Option<u64> { ... }
}
```

### 6. 优化路线图

基于基线数据，后续优化方向：

#### 6.1 短期（0x07-c）

| 优化点 | 预期提升 | 难度 |
|--------|----------|------|
| 使用 BufWriter | 10-50x I/O | 低 |
| 批量写入 | 2-5x | 低 |

#### 6.2 中期（0x08+）

| 优化点 | 预期提升 | 难度 |
|--------|----------|------|
| 异步 I/O | 解耦撮合和持久化 | 中 |
| 内存池 | 减少分配 | 中 |

#### 6.3 长期

| 优化点 | 预期提升 | 难度 |
|--------|----------|------|
| DPDK/io_uring | 10x+ | 高 |
| FPGA | 100x+ | 极高 |

### 7. 命令参考

```bash
# 运行并生成性能数据
cargo run --release

# 更新基线（当代码变化时）
cargo run --release -- --baseline

# 查看性能数据
cat output/t2_perf.txt

# 对比性能变化
python3 scripts/compare_perf.py
```

#### compare_perf.py 输出示例

```
╔════════════════════════════════════════════════════════════════════════╗
║                    Performance Comparison Report                       ║
╚════════════════════════════════════════════════════════════════════════╝

Metric                           Baseline         Current       Change
───────────────────────────────────────────────────────────────────────────
Orders                             100000          100000            -
Trades                              47886           47886            -

Exec Time                       3753.87ms       3484.37ms        -7.2%
Throughput (orders)               26639/s         28700/s        +7.7%
Throughput (trades)               12756/s         13743/s        +7.7%

───────────────────────────────────────────────────────────────────────────
Timing Breakdown (lower is better):

Metric                           Baseline         Current     Change        OPS
Balance Check                     17.68ms         16.51ms      -6.6%       6.1M
Matching Engine                   36.04ms         35.01ms      -2.8%       2.9M
Settlement                         4.77ms          5.22ms      +9.4%      19.2M
Ledger I/O                      3678.68ms       3411.49ms      -7.3%        29K

───────────────────────────────────────────────────────────────────────────
Latency Percentiles (lower is better):

Metric                           Baseline         Current       Change
Latency MIN                         125ns           125ns        +0.0%
Latency AVG                        37.9µs          34.8µs        -8.2%
Latency P50                         584ns           541ns        -7.4%
Latency P99                       420.2µs         398.9µs        -5.1%
Latency P99.9                      1.63ms          1.24ms       -24.3%
Latency MAX                        9.76ms          3.53ms       -63.9%

───────────────────────────────────────────────────────────────────────────
✅ No significant regressions detected
```

### Summary

本章完成了以下工作：

1. **PerfMetrics 结构**：收集时间分解和延迟样本
2. **时间分解**：Balance Check / Matching / Settlement / Ledger I/O
3. **延迟百分位数**：P50 / P99 / P99.9 / Max
4. **t2_perf.txt**：机器可读的性能基线文件
5. **compare_perf.py**：对比工具，检测性能回归
6. **关键发现**：Ledger I/O 占 98.4%，是主要瓶颈
