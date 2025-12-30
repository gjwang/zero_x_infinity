# 0x14 Extreme Optimization: Methodology

<h3>
  <a href="#-english">🇺🇸 English</a>
  &nbsp;&nbsp;&nbsp;|&nbsp;&nbsp;&nbsp;
  <a href="#-chinese">🇨🇳 中文</a>
</h3>

<div id="-english"></div>

## 🇺🇸 English

> **Phase V Keynote**
> **Codename**: "Metal Mode"
> **Philosophy**: "If you can't measure it, you can't improve it."

### 1. The Performance Ceiling

In the previous chapters, we built a highly reliable exchange core (Phase I-IV). We achieved **1.3M TPS** on a single thread using the Ring Buffer architecture. This is "fast enough" for 99% of crypto exchanges.

But for top-tier HFT engines, "Fast Enough" is not enough. We want to hit the physical limits of the CPU and Memory.

#### 1.1 Why "Extreme Optimization"?

| Phase | Focus | Goal |
|-------|-------|------|
| I-III | Correctness | "Does it work?" |
| IV | Integration | "Does it work end-to-end?" |
| **V** | **Speed** | **"How fast can it go?"** |

In Phase V, we assume correctness is already proven. Our sole focus is **performance**.

#### 1.2 Why "Metal Mode"?

**"Metal Mode"** is our internal codename. It means:
*   **Close to the Metal**: We will bypass high-level abstractions and work directly with memory layouts, CPU caches, and SIMD instructions.
*   **Bare Metal Rust**: No unnecessary `clone()`, no hidden `malloc()`, no runtime surprises.

---

### 2. The Benchmarking Methodology (Tier 2)

To optimize, we must first measure. But **what** we measure matters.

#### 2.1 The Problem with Naive Benchmarks

| Benchmark Type | What it Measures | Problem for Optimization |
|----------------|------------------|--------------------------|
| `wrk` / `curl` | HTTP round-trip | Includes OS, Network, Kernel noise |
| Unit tests | Function correctness | No performance data |

These are useful for **validation** (Phase IV), but not for **isolation** (Phase V).

#### 2.2 Tier 2: Pipeline Benchmarks

We introduce **Tier 2 Pipeline Benchmarks**:

| Feature | Description |
|---------|-------------|
| **No Network I/O** | Data is pre-loaded in memory. |
| **No Disk I/O** | WAL is mocked or in-memory. |
| **Pure CPU/Memory** | Measures only the "Hot Path": RingBuffer → UBSCore → ME → Settlement. |
| **Deterministic** | Same input → Same output → Same timing. |

**Goal**: Establish the **"Red Line"** – the current baseline performance under ideal conditions. All future optimizations will be measured against this.

---

### 3. The Golden Data Strategy

To ensure our benchmarks are reproducible and comparable to industry standards, we adopt the **Exchange-Core Verification Kit**.

#### 3.1 Reference: Exchange-Core

[exchange-core](https://github.com/exchange-core/exchange-core) is a well-known open-source Java matching engine. Its test suite provides:

*   **Deterministic Data Generation**: Using a Java-compatible LCG (Linear Congruential Generator) PRNG.
*   **Standard Datasets**: From `SinglePair` (1K orders) to `Huge` (30M orders).
*   **Performance Baselines**: Documented latency percentiles on reference hardware.

#### 3.2 Golden Data Files

We have pre-generated "golden" CSV files using the exact Java algorithm (Seed = 1):

| File | Records | Use Case |
|------|---------|----------|
| `golden_single_pair_margin.csv` | 1,100 | Futures margin contract verification |
| `golden_single_pair_exchange.csv` | 1,100 | Spot exchange verification |

**CSV Format**: `phase,command,order_id,symbol,price,size,action,order_type,uid`

These files serve as the **ground truth** for verifying that our Rust LCG PRNG and order generator match the Java implementation byte-for-byte.

#### 3.3 Performance Targets (Reference Hardware)

From the original Java benchmarks (Intel Xeon X5690 @ 3.47GHz):

| Operation | Mean Latency |
|-----------|--------------|
| Move Order | ~0.5 µs |
| Cancel Order | ~0.7 µs |
| Place Order | ~1.0 µs |

| Rate (ops/sec) | P50 | P99 | Worst |
|----------------|-----|-----|-------|
| 1 M | 0.5 µs | 4.0 µs | 45 µs |
| 3 M | 0.7 µs | 15.0 µs | 60 µs |

> **Target**: Rust implementation on modern hardware (i9-13900K) should achieve **< 200ns** core latency.

---

### 4. The "Metal Harness"

We will build a dedicated benchmark harness:

```
benches/metal_pipeline.rs
├── LCG PRNG (Java-compatible)
├── Load orders from golden_data/*.csv
├── Mock RingBuffer (no crossbeam overhead)
├── Mock WAL (no fsync)
└── Measures: Latency (P50, P99), Throughput (TPS)
```

This harness is the foundation of Phase V. Without it, any optimization is just guesswork.

---

<div id="-chinese"></div>

## 🇨🇳 中文

> **Phase V 基调**
> **内部代号**: "Metal Mode"
> **核心哲学**: "无法测量，就无法优化。"

### 1. 性能天花板

在前几个阶段（Phase I-IV），我们构建了一个高可靠的交易所核心。利用 Ring Buffer 架构，我们在单线程上实现了 **130万 TPS**。对于 99% 的加密货币交易所来说，这已经"足够快"了。

但对于顶级的 HFT 引擎，"足够快"是不够的。我们要触达 CPU 和内存的物理极限。

#### 1.1 为什么叫 "Extreme Optimization"？

| 阶段 | 关注点 | 目标 |
|------|--------|------|
| I-III | 正确性 | "能跑吗？" |
| IV | 集成 | "端到端能跑通吗？" |
| **V** | **速度** | **"能跑多快？"** |

在 Phase V，我们假设正确性已经被验证。唯一的焦点是**性能**。

#### 1.2 为什么叫 "Metal Mode"？

**"Metal Mode"** 是我们的内部代号，意为：
*   **贴近金属 (Close to the Metal)**：我们将绕过高层抽象，直接操作内存布局、CPU 缓存和 SIMD 指令。
*   **Bare Metal Rust**：没有不必要的 `clone()`，没有隐藏的 `malloc()`，没有运行时惊喜。

---

### 2. 基准测试方法论 (Tier 2)

要优化，必须先测量。但**测什么**至关重要。

#### 2.1 朴素基准测试的问题

| 基准测试类型 | 测量内容 | 优化的问题 |
|--------------|----------|------------|
| `wrk` / `curl` | HTTP 往返 | 包含操作系统、网络、内核噪声 |
| 单元测试 | 函数正确性 | 没有性能数据 |

这些对于**验证 (Phase IV)** 有用，但不适合**隔离测试 (Phase V)**。

#### 2.2 Tier 2: 流水线基准测试 (Pipeline Benchmarks)

我们引入 **Tier 2 流水线基准测试**：

| 特性 | 描述 |
|------|------|
| **无网络 I/O** | 数据预加载在内存中。 |
| **无磁盘 I/O** | WAL 被 Mock 或在内存中。 |
| **纯 CPU/内存** | 只测量"热路径"：RingBuffer → UBSCore → ME → Settlement。 |
| **确定性** | 相同输入 → 相同输出 → 相同耗时。 |

**目标**：建立 **"Red Line (红线)"** – 理想条件下的当前基线性能。所有后续优化都将以此为基准进行衡量。

---

### 3. 黄金数据策略 (Golden Data Strategy)

为了确保我们的基准测试可重现且与业界标准可比，我们采用 **Exchange-Core Verification Kit**。

#### 3.1 参考项目: Exchange-Core

[exchange-core](https://github.com/exchange-core/exchange-core) 是一个知名的开源 Java 撮合引擎。其测试套件提供了：

*   **确定性数据生成**: 使用 Java 兼容的 LCG (线性同余发生器) PRNG。
*   **标准数据集**: 从 `SinglePair` (1K 订单) 到 `Huge` (3000万订单)。
*   **性能基线**: 在参考硬件上记录的延迟百分位数据。

#### 3.2 黄金数据文件

我们使用精确的 Java 算法 (Seed = 1) 预生成了"黄金" CSV 文件：

| 文件 | 记录数 | 用途 |
|------|--------|------|
| `golden_single_pair_margin.csv` | 1,100 | 期货保证金合约验证 |
| `golden_single_pair_exchange.csv` | 1,100 | 现货交易验证 |

**CSV 格式**: `phase,command,order_id,symbol,price,size,action,order_type,uid`

这些文件作为**真相来源 (Ground Truth)**，用于验证我们的 Rust LCG PRNG 和订单生成器与 Java 实现完全一致。

#### 3.3 性能目标 (参考硬件)

来自原始 Java 基准测试 (Intel Xeon X5690 @ 3.47GHz)：

| 操作 | 平均延迟 |
|------|----------|
| Move Order | ~0.5 µs |
| Cancel Order | ~0.7 µs |
| Place Order | ~1.0 µs |

| 速率 (ops/sec) | P50 | P99 | 最差 |
|----------------|-----|-----|------|
| 1 M | 0.5 µs | 4.0 µs | 45 µs |
| 3 M | 0.7 µs | 15.0 µs | 60 µs |

> **目标**: Rust 实现在现代硬件 (i9-13900K) 上应达到 **< 200ns** 核心延迟。

---

### 4. "Metal Harness" (金属测试脚手架)

我们将构建一个专用的基准测试脚手架：

```
benches/metal_pipeline.rs
├── LCG PRNG (Java 兼容)
├── 从 golden_data/*.csv 加载订单
├── Mock RingBuffer (无 crossbeam 开销)
├── Mock WAL (无 fsync)
└── 测量指标: 延迟 (P50, P99), 吞吐量 (TPS)
```

这个脚手架是 Phase V 的基础。没有它，任何优化都只是猜测。
