# 0x08-e 撤单性能优化：Order Index

> **核心目标**：通过引入订单索引，将撤单查找复杂度从 O(N) 优化到 O(1)。

---

## 1. 问题回顾

在 [0x08-d](./0x08-d-complete-order-lifecycle.md) 中，我们实现了完整的撤单流程。但在大规模压测时发现了严重的性能问题：

### 1.1 现象
- **基准测试 (10万 Place)**: 耗时 ~3秒
- **撤单测试 (100万 Place + 30% Cancel)**: 耗时 **超过 7 分钟**

### 1.2 原因分析

问题出在 `OrderBook::remove_order_by_id` 的实现：

```rust
// 优化前：O(N) 全表扫描
pub fn remove_order_by_id(&mut self, order_id: u64) -> Option<InternalOrder> {
    // 遍历所有 bids 价格层级
    for (key, orders) in self.bids.iter_mut() {
        // 遍历该价格层级的所有订单
        if let Some(pos) = orders.iter().position(|o| o.order_id == order_id) {
            // 找到了...
        }
    }
    // 再遍历所有 asks...
}
```

**复杂度**: O(P × K) ≈ **O(N)**
- P = 价格层级数
- K = 每个价格层级的平均订单数
- N = 订单总数

当盘口堆积了 50万 未成交订单时，执行 30万 次撤单，每次都要遍历整个订单簿！

---

## 2. 解决方案：Order Index

### 2.1 核心思想

引入一个 **HashMap 索引**，将 `OrderId` 映射到 `(Price, Side)`：

```rust
use rustc_hash::FxHashMap;

pub struct OrderBook {
    asks: BTreeMap<u64, VecDeque<InternalOrder>>,
    bids: BTreeMap<u64, VecDeque<InternalOrder>>,
    
    // 🆕 订单索引：OrderId -> (Price, Side)
    order_index: FxHashMap<u64, (u64, Side)>,
    
    trade_id_counter: u64,
}
```

### 2.2 选择 FxHashMap 的原因

| HashMap 类型 | 特点 |
|-------------|------|
| `std::HashMap` | 使用 SipHash，防 DoS 攻击，较慢 |
| `FxHashMap` | 使用 FxHash，速度极快，适合整数 key |

对于 `u64` 类型的 `order_id`，`FxHashMap` 是最佳选择。

---

## 3. 实现细节

### 3.1 索引维护点

需要在以下操作中维护索引的一致性：

| 操作 | 索引动作 |
|------|----------|
| `rest_order()` | **插入** 索引 |
| `cancel_order()` | **移除** 索引 |
| `remove_order_by_id()` | **移除** 索引 |
| 撮合成交 (`pop_front()`) | **移除** 索引 |

### 3.2 rest_order 实现

```rust
pub fn rest_order(&mut self, order: InternalOrder) {
    // 维护索引
    self.order_index.insert(order.order_id, (order.price, order.side));

    match order.side {
        Side::Buy => {
            let key = u64::MAX - order.price;
            self.bids.entry(key).or_default().push_back(order);
        }
        Side::Sell => {
            self.asks.entry(order.price).or_default().push_back(order);
        }
    }
}
```

### 3.3 remove_order_by_id 优化实现

```rust
pub fn remove_order_by_id(&mut self, order_id: u64) -> Option<InternalOrder> {
    // O(1) - 从索引获取 price 和 side
    let (price, side) = self.order_index.remove(&order_id)?;

    // O(log n) - 定位价格层级
    let (book, key) = match side {
        Side::Buy => (&mut self.bids, u64::MAX - price),
        Side::Sell => (&mut self.asks, price),
    };

    let orders = book.get_mut(&key)?;

    // O(k) - 在该价格层级内查找（k 通常很小）
    let pos = orders.iter().position(|o| o.order_id == order_id)?;
    let order = orders.remove(pos)?;

    // 清理空价格层级
    if orders.is_empty() {
        book.remove(&key);
    }

    Some(order)
}
```

### 3.4 撮合引擎同步

在 `engine.rs` 中，当订单被完全成交并移除时，需要同步更新索引：

```rust
// 收集成交订单的 ID
let mut filled_order_ids = Vec::new();

while let Some(sell_order) = orders.front_mut() {
    // ... 撮合逻辑 ...
    
    if sell_order.is_filled() {
        filled_order_ids.push(sell_order.order_id);
        orders.pop_front();
    }
}

// 批量从索引中移除（避免借用冲突）
for order_id in filled_order_ids {
    book.remove_from_index(order_id);
}
```

> ⚠️ **Rust 借用检查器**：不能在持有 `book.asks_mut()` 引用的循环内调用 `book.remove_from_index()`，
> 需要先收集 ID，循环结束后再批量移除。

---

## 4. 复杂度对比

| 操作 | 优化前 | 优化后 |
|------|--------|--------|
| `remove_order_by_id` | O(N) | **O(1)** + O(log P) + O(K) |
| `rest_order` | O(log P) | O(log P) + O(1) |
| 内存开销 | - | +24 bytes/订单 |

其中：
- N = 订单总数
- P = 价格层级数
- K = 单个价格层级的订单数（通常 < 100）

---

## 5. 性能验证

### 5.1 测试环境
- 数据集：130万订单（100万 Place + 30万 Cancel）
- 机器：MacBook Pro M1

### 5.2 详细测试结果

**Cancel 测试 (UBSCore Pipeline)**:
```
=== Execution Summary ===
Symbol: BTC_USDT
Total Orders: 1300000
  Accepted: 903107
  Rejected: 96893
Total Trades: 538487
Execution Time: 102.05s
Throughput: 12739 orders/sec | 5277 trades/sec

Final Orderbook:
  Best Bid: Some(8534925)
  Best Ask: Some(8538446)
  Bid Depth: 98178 levels
  Ask Depth: 16323 levels

=== Performance Breakdown ===
Balance Check:     1329.40ms (  1.3%)
Matching Engine:  98581.51ms ( 97.1%)
Settlement:         141.09ms (  0.1%)
Ledger I/O:        1472.63ms (  1.5%)

=== Latency Percentiles (sampled) ===
  Min:       1084 ns
  Avg:     114412 ns
  P50:      48084 ns
  P99:     710667 ns
  P99.9:  1375833 ns
  Max:   21157125 ns
```

**对比基准 (Baseline - 无 UBSCore)**:
```
Total Orders: 100000
Execution Time: 2.93s
Throughput: 34171 orders/sec

=== Performance Breakdown ===
Balance Check:       16.23ms (  0.6%)
Matching Engine:   2870.91ms ( 98.6%)
Settlement:           3.63ms (  0.1%)
Ledger I/O:          20.67ms (  0.7%)
```

### 5.3 性能对比表

| 测试场景 | 订单数 | ME 时间 | 总时间 | 吞吐量 |
|----------|--------|---------|--------|--------|
| Baseline (无 UBS, 无 Cancel) | 100k | 2.87s | 4.5s | **34k/s** |
| Order Index + UBS + Cancel | 1.3M | 98.6s | 102s | **12.7k/s** |
| 优化前 (O(N) 撤单) | 1M | 7+ min | 7+ min | ~3k/s |

### 5.4 验证通过
```
=== Step 2: Verify Balance Events ===
✅ Lock events (903107) = Accepted orders (903107)
✅ All trades have zero sum delta (538487 trades)
✅ Frozen balances match event history

=== Step 3: Verify Order Events ===
✅ Order lifecycle consistency checks passed (1000000 orders)
✅ SUCCESS: All order event checks passed
```

---

## 6. 剩余性能问题分析

### 6.1 VecDeque 不是瓶颈

经过分析，每个价格层级的订单数分布：
```bash
# 最多只有 13 个订单在同一价格
$ awk -F',' 'NR>1 {print $5}' output/t2_orderbook.csv | sort | uniq -c | sort -rn | head -5
  13 8471263
  11 8506461
  11 8502051
  ...
```

**K 值很小 (≤13)**，O(K) 的影响微乎其微。

### 6.2 真正的瓶颈：UBSCore Pipeline 开销

| 组件 | Baseline | UBSCore | 差异原因 |
|------|----------|---------|----------|
| 每订单延迟 | ~29µs | ~114µs | **~4x 额外开销** |
| 瓶颈占比 | ME 98.6% | ME 97.1% | 看似相同，但 ME 包含 UBS 逻辑 |

**额外开销来源**:

1. **WAL 写入** (每订单)
   ```rust
   ubscore.process_order() // 写入 WAL
   ```

2. **多次余额查询** (每笔交易)
   ```rust
   ubscore.get_balance(buyer, quote_id)  // 查询 1
   ubscore.get_balance(buyer, base_id)   // 查询 2
   ubscore.get_balance(seller, base_id)  // 查询 3
   ubscore.get_balance(seller, quote_id) // 查询 4
   // + 额外的 settle, refund 查询
   ```

3. **事件记录** (每笔交易)
   ```rust
   ledger.write_balance_event(&settle_event)  // 4次
   ledger.write_entry(&LedgerEntry)           // 4次
   // Cancel 还有 unlock_event + order_event
   ```

4. **Price Improvement Refund** (买单成交)
   ```rust
   if valid_order.order.price > trade.price {
       // 计算 refund + settle_unlock + 记录事件
   }
   ```

### 6.3 优化方向

| 优化项 | 预期收益 | 复杂度 |
|--------|----------|--------|
| 批量 WAL 写入 | ~10-20% | 低 |
| 减少余额查询 (缓存) | ~15-25% | 中 |
| 异步 Ledger I/O | ~5-10% | 中 |
| 移除重复事件 (Legacy + Event) | ~5% | 低 |

---

## 7. 总结

### 7.1 关键收获

1. **算法复杂度至关重要**：O(N) vs O(1) 在大规模数据下差异巨大
2. **索引是空间换时间的经典策略**：额外 24 bytes/订单换取 4x 性能提升
3. **Rust 借用检查器**：强制我们写出更安全的代码，但需要理解其规则
4. **性能分析要精确**：VecDeque 看似可疑，实际不是瓶颈

### 7.2 设计模式

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

## 8. 下一步优化计划

### 8.1 已完成 ✅
- [x] Order Index 实现 (O(1) 撤单查找)
- [x] 性能从 ~3k/s 提升到 ~12.7k/s

### 8.2 待优化 📋
- [ ] 减少 `get_balance` 调用次数 (缓存或合并)
- [ ] 批量 WAL flush (已有 100 条/批，可调优)
- [ ] 移除重复的 Legacy Ledger 写入
- [ ] 考虑异步事件记录

### 8.3 未来考虑 🔮
- [ ] SIMD 优化数值计算
- [ ] 内存池减少分配
- [ ] Lock-free 数据结构

---

**Order Index 优化完成，撤单性能提升 4 倍！**

**下一步重点：减少 UBSCore Pipeline 的中间开销。** 🚀

