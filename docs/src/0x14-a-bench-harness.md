# 0x14-a Benchmark Harness: The Metal Foundation

<h3>
  <a href="#-english">🇺🇸 English</a>
  &nbsp;&nbsp;&nbsp;|&nbsp;&nbsp;&nbsp;
  <a href="#-chinese">🇨🇳 中文</a>
</h3>

<div id="-english"></div>

## 🇺🇸 English

> **Phase V, Step 1**
> **Objective**: Build the Tier 2 Pipeline Benchmark infrastructure using the Exchange-Core Verification Kit.

---

### 1. Chapter Overview

This chapter establishes the **"Metal Harness"** – a dedicated benchmark environment that isolates the matching engine from external noise (Network, Disk I/O) and measures pure CPU/Memory performance.

**Prerequisites**:
*   Chapter [0x14: Extreme Optimization](./0x14-extreme-optimization.md) (Methodology)
*   `docs/exchange_core_verification_kit/` (Golden Data)

---

### 2. Golden Data Integration

We use pre-generated CSV files from the Exchange-Core project to ensure bit-accurate parity with the Java reference implementation.

#### 2.1 Data Files

| File | Records | Description |
|------|---------|-------------|
| `golden_single_pair_margin.csv` | 1,100 | Futures (margin) contract test data |
| `golden_single_pair_exchange.csv` | 1,100 | Spot exchange test data |

**CSV Format**:
```csv
phase,command,order_id,symbol,price,size,action,order_type,uid
PREFILL,PLACE_ORDER,1,0,12345,100,BID,GTC,42
BENCHMARK,PLACE_ORDER,2,0,12340,50,ASK,IOC,17
...
```

#### 2.2 LCG PRNG Implementation

To generate larger datasets deterministically, we implement the Java-compatible Linear Congruential Generator:

```rust
/// Java-compatible LCG PRNG
pub struct JavaRandom {
    seed: u64,
}

impl JavaRandom {
    const MULTIPLIER: u64 = 0x5DEECE66D;
    const ADDEND: u64 = 0xB;
    const MASK: u64 = (1 << 48) - 1;

    pub fn new(seed: i64) -> Self {
        Self {
            seed: (seed as u64 ^ Self::MULTIPLIER) & Self::MASK,
        }
    }

    fn next(&mut self, bits: u32) -> i32 {
        self.seed = (self.seed.wrapping_mul(Self::MULTIPLIER).wrapping_add(Self::ADDEND)) & Self::MASK;
        (self.seed >> (48 - bits)) as i32
    }

    pub fn next_int(&mut self, bound: i32) -> i32 {
        // ... Java Random.nextInt(bound) logic
    }
}
```

---

### 3. Metal Harness Architecture

```
benches/metal_pipeline.rs
├── Criterion Benchmark Group
│   ├── "baseline_serde" - Current bincode/serde pipeline
│   └── "baseline_raw"   - Pre-parsed order vector
├── Mock Components
│   ├── MockRingBuffer   - In-memory queue (no crossbeam)
│   └── MockWAL          - No-op persistence
├── Data Loaders
│   ├── load_golden_csv  - Load from CSV files
│   └── generate_orders  - Use LCG to generate N orders
└── Metrics
    ├── Throughput (TPS)
    └── Latency (P50, P99, Worst)
```

---

### 4. Implementation Checklist

- [ ] **Step 1**: Implement `JavaRandom` LCG PRNG
    - [ ] Pass unit tests against golden data
- [ ] **Step 2**: Create `benches/metal_pipeline.rs`
    - [ ] Setup Criterion benchmark group
    - [ ] Add CSV loader
- [ ] **Step 3**: Mock Components
    - [ ] `MockRingBuffer` (simple `VecDeque`)
    - [ ] `MockWAL` (no-op)
- [ ] **Step 4**: Establish Baseline
    - [ ] Run benchmarks
    - [ ] Document "Red Line" metrics

---

<div id="-chinese"></div>

## 🇨🇳 中文

> **Phase V, 步骤 1**
> **目标**: 使用 Exchange-Core Verification Kit 构建 Tier 2 流水线基准测试基础设施。

---

### 1. 章节概述

本章建立 **"Metal Harness (金属测试脚手架)"** – 一个专用的基准测试环境，将撮合引擎与外部噪声（网络、磁盘 I/O）隔离，测量纯 CPU/内存性能。

**前置条件**:
*   章节 [0x14: Extreme Optimization](./0x14-extreme-optimization.md) (方法论)
*   `docs/exchange_core_verification_kit/` (黄金数据)

---

### 2. 黄金数据集成

我们使用从 Exchange-Core 项目预生成的 CSV 文件，确保与 Java 参考实现完全一致。

#### 2.1 数据文件

| 文件 | 记录数 | 描述 |
|------|--------|------|
| `golden_single_pair_margin.csv` | 1,100 | 期货（保证金）合约测试数据 |
| `golden_single_pair_exchange.csv` | 1,100 | 现货交易测试数据 |

#### 2.2 LCG PRNG 实现

为了确定性地生成更大规模的数据集，我们实现 Java 兼容的线性同余发生器 (LCG)。

---

### 3. Metal Harness 架构

```
benches/metal_pipeline.rs
├── Criterion 基准测试组
│   ├── "baseline_serde" - 当前 bincode/serde 流水线
│   └── "baseline_raw"   - 预解析订单向量
├── Mock 组件
│   ├── MockRingBuffer   - 内存队列 (无 crossbeam)
│   └── MockWAL          - 空操作持久化
├── 数据加载器
│   ├── load_golden_csv  - 从 CSV 文件加载
│   └── generate_orders  - 使用 LCG 生成 N 个订单
└── 指标
    ├── 吞吐量 (TPS)
    └── 延迟 (P50, P99, 最差)
```

---

### 4. 实施清单

- [ ] **步骤 1**: 实现 `JavaRandom` LCG PRNG
    - [ ] 通过黄金数据单元测试
- [ ] **步骤 2**: 创建 `benches/metal_pipeline.rs`
    - [ ] 设置 Criterion 基准测试组
    - [ ] 添加 CSV 加载器
- [ ] **步骤 3**: Mock 组件
    - [ ] `MockRingBuffer` (简单的 `VecDeque`)
    - [ ] `MockWAL` (空操作)
- [ ] **步骤 4**: 建立基线
    - [ ] 运行基准测试
    - [ ] 记录 "Red Line" 指标
