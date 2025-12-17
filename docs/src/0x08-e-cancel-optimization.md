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

### 5.2 结果对比

| 指标 | 优化前 | 优化后 | 改进 |
|------|--------|--------|------|
| **执行时间** | 7+ 分钟 | **102 秒** | **~4.2x** |
| **吞吐量** | ~3k ops/s | **12.7k ops/s** | **~4x** |

### 5.3 验证通过
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

## 6. 总结

### 6.1 关键收获

1. **算法复杂度至关重要**：O(N) vs O(1) 在大规模数据下差异巨大
2. **索引是空间换时间的经典策略**：额外 24 bytes/订单换取 4x 性能提升
3. **Rust 借用检查器**：强制我们写出更安全的代码，但需要理解其规则

### 6.2 设计模式

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

## 7. 下一步

- [ ] 考虑 `VecDeque` 内的 O(K) 查找优化（如使用 `IndexMap`）
- [ ] 添加索引健康检查和自动修复机制
- [ ] 性能监控：索引命中率、平均 K 值等

---

**优化完成，撤单性能提升 4 倍！** 🚀
