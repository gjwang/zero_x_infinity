# 0x08-e 撤单优化与性能分析

> **本章目标**：
> 1. 实现 Order Index 优化撤单查找
> 2. 建立正确的架构级 Profiling
> 3. 精确定位性能瓶颈

---

## 1. Order Index 优化

### 1.1 问题

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

### 1.2 解决方案

引入 `order_index: FxHashMap<OrderId, (Price, Side)>` 实现 O(1) 查找：

```rust
pub struct OrderBook {
    asks: BTreeMap<u64, VecDeque<InternalOrder>>,
    bids: BTreeMap<u64, VecDeque<InternalOrder>>,
    order_index: FxHashMap<u64, (u64, Side)>,  // 新增
    trade_id_counter: u64,
}
```

### 1.3 索引维护

| 操作 | 索引动作 |
|------|----------|
| `rest_order()` | 插入 |
| `cancel_order()` | 移除 |
| `remove_order_by_id()` | 移除 |
| 撮合成交 | 移除 |

### 1.4 优化后实现

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

---

## 2. 架构级 Profiling

### 2.1 正确的 Profiling 方法

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

### 2.2 PerfMetrics 设计

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
    pub trade_count: u64,
    
    // 子级分析
    pub total_cancel_lookup_ns: u64,
}
```

---

## 3. 性能测试结果

### 3.1 测试环境
- 数据集：130万订单（100万 Place + 30万 Cancel）
- 机器：MacBook Pro M1

### 3.2 架构级 Breakdown

```
=== Performance Breakdown ===
Orders: 1300000 (Place: 1000000, Cancel: 300000), Trades: 538487

1. Pre-Trade:        745.96ms (  0.9%)  [  0.57 µs/order]
2. Matching:       83530.33ms ( 96.4%)  [ 83.53 µs/order]
3. Settlement:        37.93ms (  0.0%)  [  0.07 µs/trade]
4. Event Log:       2362.76ms (  2.7%)  [  1.82 µs/order]

Total Tracked:     86676.97ms

--- Sub-Breakdown ---
  Cancel Lookup:      96.82ms  [0.32 µs/cancel]
```

### 3.3 关键发现

| 阶段 | 时间 | 占比 | 每操作耗时 |
|------|------|------|-----------|
| **Matching** | **83.5s** | **96.4%** | **83.53 µs/order** |
| Event Log | 2.4s | 2.7% | 1.82 µs/order |
| Pre-Trade | 0.75s | 0.9% | 0.57 µs/order |
| Settlement | 0.04s | 0.0% | 0.07 µs/trade |
| Cancel Lookup | 0.10s | - | **0.32 µs/cancel** |

### 3.4 结论

1. **Order Index 优化成功** - 撤单查找从 O(N) 降到 O(1)，仅需 0.32 µs/次
2. **瓶颈是 Matching Engine** - 占用 96.4% 的时间
3. **UBSCore 开销很小** - Pre-Trade + Settlement 不到 1%
4. **Event Logging 可接受** - 仅占 2.7%

---

## 4. 执行性能对比

| 版本 | 执行时间 | 吞吐量 | 改进 |
|------|----------|--------|------|
| 优化前 (O(N) 撤单) | 7+ 分钟 | ~3k ops/s | - |
| Order Index + 错误 profiling | 102s | 12.7k ops/s | 4x |
| Order Index + 正确 profiling | **87s** | **15k ops/s** | **5x** |

---

## 5. 总结

### 5.1 已完成

- [x] Order Index 实现 - O(1) 撤单查找
- [x] 架构级 Profiling - 正确定位瓶颈
- [x] 性能提升 5x (3k → 15k ops/s)

### 5.2 下一步

**优化 Matching Engine** - 当前瓶颈 (96.4% 时间)

可能的优化方向：
- 分析 `MatchingEngine::process_order` 内部细节
- 考虑数据结构优化
- 减少 clone 操作

---

## 6. 设计模式

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
│  └─────────────────┘    └─────────────────────────────┘ │
└─────────────────────────────────────────────────────────┘
```

---

**Order Index 优化完成，真正瓶颈已定位：Matching Engine (96.4%)** 🔍
