# 0x08-e Performance Profiling & Optimization

<h3>
  <a href="#-english">🇺🇸 English</a>
  &nbsp;&nbsp;&nbsp;|&nbsp;&nbsp;&nbsp;
  <a href="#-chinese">🇨🇳 中文</a>
</h3>

<div id="-english"></div>

## 🇺🇸 English

> **📦 Code Changes**: [View Diff](https://github.com/gjwang/zero_x_infinity/compare/v0.8-d-complete-order-lifecycle...v0.8-e-perf-bottleneck-profiling)

> **Background**: After introducing Cancel, execution time exploded from ~30s to 7+ minutes. We need to identify and fix the issue.
>
> **Goal**:
> 1. Establish architecture-level profiling to pinpoint bottlenecks.
> 2. Fix the identified O(N) issues.
> 3. Verify improvements with data.

---

## 1. Symptoms

Performance collapsed after adding Cancel:
- Execution Time: ~30s → 7+ minutes
- Throughput: ~34k ops/s → ~3k ops/s

**Hypothesis**:
- Is it the O(N) Cancel scan?
- `VecDeque` removal overhead?
- Something else?

**Hypothesis implies guessing. Profiling provides facts.**

---

## 2. Optimization 1: Order Index

### 2.1 The Problem

Cancelling requires looking up an order. The naive `remove_order_by_id` iterates the entire book:

```rust
// Before: O(N) full scan
pub fn remove_order_by_id(&mut self, order_id: u64) -> Option<InternalOrder> {
    for (key, orders) in self.bids.iter_mut() {
        if let Some(pos) = orders.iter().position(|o| o.order_id == order_id) {
            // ...
        }
    }
    // Scan asks...
}
```

### 2.2 The Solution

Introduce `order_index: FxHashMap<OrderId, (Price, Side)>` for O(1) lookup.

```rust
pub struct OrderBook {
    asks: BTreeMap<u64, VecDeque<InternalOrder>>,
    bids: BTreeMap<u64, VecDeque<InternalOrder>>,
    order_index: FxHashMap<u64, (u64, Side)>,  // New
    trade_id_counter: u64,
}
```

### 2.3 Index Maintenance

| Operation | Action |
|-----------|--------|
| `rest_order()` | Insert |
| `cancel_order()` | Remove |
| `remove_order_by_id()` | Remove |
| Match Fill | Remove |

### 2.4 Optimized Implementation

```rust
pub fn remove_order_by_id(&mut self, order_id: u64) -> Option<InternalOrder> {
    // O(1) Lookup
    let (price, side) = self.order_index.remove(&order_id)?;
    
    // O(log n) Find level
    let (book, key) = match side {
        Side::Buy => (&mut self.bids, u64::MAX - price),
        Side::Sell => (&mut self.asks, price),
    };
    
    // O(k) Find in level (k is small)
    let orders = book.get_mut(&key)?;
    let pos = orders.iter().position(|o| o.order_id == order_id)?;
    let order = orders.remove(pos)?;
    
    if orders.is_empty() {
        book.remove(&key);
    }
    
    Some(order)
}
```

### 2.5 Result 1

| Metric | Before | After |
|--------|--------|-------|
| Time | 7+ min | **87s** |
| Throughput | ~3k ops/s | **15k ops/s** |
| Boost | - | **5x** |

**Huge improvement!** But 87s for 1.3M orders is still slow (15k ops/s). Further analysis is needed.

---

## 3. Architecture Profiling

### 3.1 Design

Measure time at architectural stages:

```
Order Input
    │
    ▼
┌─────────────────┐
│  1. Pre-Trade   │  ← UBSCore: WAL + Balance Lock
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  2. Matching    │  ← Pure ME: process_order
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  3. Settlement  │  ← UBSCore: settle_trade
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  4. Event Log   │  ← Ledger writes
└─────────────────┘
```

### 3.2 PerfMetrics

```rust
pub struct PerfMetrics {
    pub total_pretrade_ns: u64,    // UBSCore WAL + Lock
    pub total_matching_ns: u64,    // Match processing
    pub total_settlement_ns: u64,  // Balance updates
    pub total_event_log_ns: u64,   // Ledger I/O
    
    pub place_count: u64,
    pub cancel_count: u64,
}
```

---

## 4. Optimization 2: Matching Engine

### 4.1 Bottleneck Identification

Profiling revealed `Matching Engine` used **96%** of time.
Deep dive found:

```rust
// Problem: Copy ALL price keys on every match
let prices: Vec<u64> = book.asks().keys().copied().collect();
```

With 250k+ price levels in the Cancel test, copying keys O(P) + Alloc every match is disastrous.

### 4.2 Solution

Use `BTreeMap::range()` to iterate only relevant prices.

```rust
// Solution: Iterate only valid price range
let max_price = if buy_order.order_type == OrderType::Limit {
    buy_order.price
} else {
    u64::MAX
};
let prices: Vec<u64> = book.asks().range(..=max_price).map(|(&k, _)| k).collect();
```

---

## 5. Final Results

### 5.1 Environment
*   Dataset: 1.3M Orders (1M Place + 300k Cancel)
*   HW: MacBook Pro M1

### 5.2 Breakdown

```
=== Performance Breakdown ===
Orders: 1300000, Trades: 538487

1. Pre-Trade:        621.97ms (  3.5%)  [  0.48 µs/order]
2. Matching:       15014.08ms ( 84.0%)  [ 15.01 µs/order]
3. Settlement:        21.57ms (  0.1%)  [  0.04 µs/trade]
4. Event Log:       2206.71ms ( 12.4%)  [  1.70 µs/order]

Total Tracked:     17864.33ms
```

### 5.3 Improvements

| Stage | Latency Before | Latency After | Gain |
|-------|----------------|---------------|------|
| Matching | 83.53 µs/order | **15.01 µs/order** | **5.6x** |
| Cancel Lookup | O(N) | **0.29 µs** | - |

---

## 6. Comparison Table

| Version | Time | Throughput | Gain |
|---------|------|------------|------|
| Before optimization | 7+ min | ~3k ops/s | - |
| Order Index | 87s | 15k ops/s | 5x |
| **+ BTreeMap range** | **18s** | **72k ops/s** | **24x** |

---

## 7. Summary

### 7.1 Achievements

| Optimization | Problem | Solution | Result |
|--------------|---------|----------|--------|
| Order Index | O(N) Cancel | `FxHashMap` | 0.29 µs |
| Range Query | Full key copy | `range()` | 83→15 µs |

### 7.2 Final Design Pattern

```
┌─────────────────────────────────────────────────────────┐
│                     OrderBook                           │
│  ┌─────────────────┐    ┌─────────────────────────────┐ │
│  │   order_index   │◄───│  Sync on: rest, cancel,     │ │
│  │ FxHashMap<id,   │    │           match, remove     │ │
│  │   (price,side)> │    └─────────────────────────────┘ │
│  └────────┬────────┘                                    │
│           │ O(1) lookup                                 │
│           ▼                                             │
│  ┌─────────────────┐    ┌─────────────────────────────┐ │
│  │      bids       │    │          asks               │ │
│  │ BTreeMap<price, │    │  BTreeMap<price,            │ │
│  │   VecDeque>     │    │    VecDeque>                │ │
│  │  + range()      │    │    + range()                │ │
│  └─────────────────┘    └─────────────────────────────┘ │
└─────────────────────────────────────────────────────────┘
```

**Optimization Conclusion**: From 7 minutes to 18 seconds. **24x boost**. 🚀

<br>
<div align="right"><a href="#-english">↑ Back to Top</a></div>
<br>

---

<div id="-chinese"></div>

## 🇨🇳 中文

> **📦 代码变更**: [查看 Diff](https://github.com/gjwang/zero_x_infinity/compare/v0.8-d-complete-order-lifecycle...v0.8-e-perf-bottleneck-profiling)

> **背景**：引入 Cancel 功能后，执行时间从 ~30s 暴涨到 7+ 分钟，需要定位并解决问题。
>
> **本章目的**：
> 1. 建立正确的架构级 Profiling 方法
> 2. 通过 Profiling 精确定位性能瓶颈
> 3. 针对性修复发现的问题
>
> **关键点**：直觉可以指导方向，但必须用 Profiling 数据验证。

---

## 1. 问题现象

引入 Cancel 后性能急剧下降：
- 执行时间：~30s → 7+ 分钟
- 吞吐量：~34k ops/s → ~3k ops/s

**初始假设可能的原因：**
- Cancel 的 O(N) 查找？
- VecDeque 删除开销？
- 其他未知问题？

**但在 Profile 之前，这些都只是猜测。**

---

## 2. Order Index 优化（第一次优化）

### 2.1 问题

撤单操作需要在 OrderBook 中查找订单。原始实现 `remove_order_by_id` 需要遍历整个订单簿：

```rust
// 优化前：O(N) 全表扫描
pub fn remove_order_by_id(&mut self, order_id: u64) -> Option<InternalOrder> {
    for (key, orders) in self.bids.iter_mut() {
        if let Some(pos) = orders.iter().position(|o| o.order_id == order_id) {
            // ...
        }
    }
    // 再遍历 asks...
}
```

### 2.2 解决方案

引入 `order_index: FxHashMap<OrderId, (Price, Side)>` 实现 O(1) 查找：

```rust
pub struct OrderBook {
    asks: BTreeMap<u64, VecDeque<InternalOrder>>,
    bids: BTreeMap<u64, VecDeque<InternalOrder>>,
    order_index: FxHashMap<u64, (u64, Side)>,  // 新增
    trade_id_counter: u64,
}
```

### 2.3 索引维护

| 操作 | 索引动作 |
|------|----------|
| `rest_order()` | 插入 |
| `cancel_order()` | 移除 |
| `remove_order_by_id()` | 移除 |
| 撮合成交 | 移除 |

### 2.4 优化后实现

```rust
pub fn remove_order_by_id(&mut self, order_id: u64) -> Option<InternalOrder> {
    // O(1) 查找
    let (price, side) = self.order_index.remove(&order_id)?;
    
    // O(log n) 定位价格层级
    let (book, key) = match side {
        Side::Buy => (&mut self.bids, u64::MAX - price),
        Side::Sell => (&mut self.asks, price),
    };
    
    // O(k) 在价格层级内查找 (k 通常很小)
    let orders = book.get_mut(&key)?;
    let pos = orders.iter().position(|o| o.order_id == order_id)?;
    let order = orders.remove(pos)?;
    
    if orders.is_empty() {
        book.remove(&key);
    }
    
    Some(order)
}
```

### 2.5 第一次优化结果

| 指标 | 优化前 | 优化后 |
|------|--------|--------|
| 执行时间 | 7+ 分钟 | **87s** |
| 吞吐量 | ~3k ops/s | **15k ops/s** |
| 提升 | - | **5x** |

**提升巨大！** 但 87s 处理 130万订单仍然很慢。需要继续分析。

---

## 3. 架构级 Profiling（定位真正瓶颈）

### 3.1 Profiling 设计

按照订单生命周期的顶层架构分阶段计时：

```
Order Input
    │
    ▼
┌─────────────────┐
│  1. Pre-Trade   │  ← UBSCore: WAL + Balance Lock
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  2. Matching    │  ← Pure ME: process_order
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  3. Settlement  │  ← UBSCore: settle_trade
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  4. Event Log   │  ← Ledger writes
└─────────────────┘
```

### 3.2 PerfMetrics 设计

```rust
pub struct PerfMetrics {
    // 顶层架构计时
    pub total_pretrade_ns: u64,    // UBSCore WAL + Lock
    pub total_matching_ns: u64,    // Pure ME
    pub total_settlement_ns: u64,  // Balance updates
    pub total_event_log_ns: u64,   // Ledger writes
    
    // 操作计数
    pub place_count: u64,
    pub cancel_count: u64,
}
```

---

## 4. Matching Engine 优化（第二次优化）

### 4.1 问题定位

通过架构级 Profiling 发现 Matching Engine 占用 96% 时间。深入分析发现：

```rust
// 问题代码：每次 match 都复制所有价格 keys
let prices: Vec<u64> = book.asks().keys().copied().collect();
```

当订单簿有 25万+ 价格层级时，每次 match 都要：
1. 遍历整个 BTreeMap 收集 keys - O(P)
2. 分配 Vec 存储 - 内存分配开销
3. 再遍历 Vec 进行匹配

### 4.2 优化方案

使用 `BTreeMap::range()` 只收集匹配范围内的 keys：

```rust
// 优化后：只收集匹配价格范围内的 keys
let max_price = if buy_order.order_type == OrderType::Limit {
    buy_order.price
} else {
    u64::MAX
};
let prices: Vec<u64> = book.asks().range(..=max_price).map(|(&k, _)| k).collect();
```

---

## 5. 性能测试结果

### 5.1 测试环境
- 数据集：130万订单（100万 Place + 30万 Cancel）
- 机器：MacBook Pro M1

### 5.2 最终 Breakdown

```
=== Performance Breakdown ===
Orders: 1300000 (Place: 1000000, Cancel: 300000), Trades: 538487

1. Pre-Trade:        621.97ms (  3.5%)  [  0.48 µs/order]
2. Matching:       15014.08ms ( 84.0%)  [ 15.01 µs/order]
3. Settlement:        21.57ms (  0.1%)  [  0.04 µs/trade]
4. Event Log:       2206.71ms ( 12.4%)  [  1.70 µs/order]

Total Tracked:     17864.33ms
```

### 5.3 优化效果

| 阶段 | 优化前 | 优化后 | 提升 |
|------|--------|--------|------|
| Matching | 83.53 µs/order | **15.01 µs/order** | **5.6x** |
| Cancel Lookup | O(N) | **0.29 µs** | - |

---

## 6. 执行性能对比

| 版本 | 执行时间 | 吞吐量 | 改进 |
|------|----------|--------|------|
| 优化前 (O(N) 撤单 + 全量 keys) | 7+ 分钟 | ~3k ops/s | - |
| Order Index 优化 | 87s | 15k ops/s | 5x |
| **+ BTreeMap range query** | **18s** | **72k ops/s** | **24x** |

---

## 7. 总结

### 7.1 优化成果

| 优化 | 问题 | 解决方案 | 效果 |
|------|------|----------|------|
| Order Index | O(N) 撤单查找 | FxHashMap 索引 | 0.29 µs/cancel |
| BTreeMap range | 全量 keys 复制 | range() 范围查询 | 83→15 µs/order |

### 7.2 最终设计模式

```
┌─────────────────────────────────────────────────────────┐
│                     OrderBook                           │
│  ┌─────────────────┐    ┌─────────────────────────────┐ │
│  │   order_index   │◄───│  Sync on: rest, cancel,     │ │
│  │ FxHashMap<id,   │    │           match, remove     │ │
│  │   (price,side)> │    └─────────────────────────────┘ │
│  └────────┬────────┘                                    │
│           │ O(1) lookup                                 │
│           ▼                                             │
│  ┌─────────────────┐    ┌─────────────────────────────┐ │
│  │      bids       │    │          asks               │ │
│  │ BTreeMap<price, │    │  BTreeMap<price,            │ │
│  │   VecDeque>     │    │    VecDeque>                │ │
│  │  + range()      │    │    + range()                │ │
│  └─────────────────┘    └─────────────────────────────┘ │
└─────────────────────────────────────────────────────────┘
```

**本次优化先到此为止！从 7 分钟到 18 秒，吞吐量提升 24 倍！** 🚀
