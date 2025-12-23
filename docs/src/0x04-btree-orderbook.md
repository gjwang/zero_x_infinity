# 0x04 OrderBook Refactoring (BTreeMap)

<h3>
  <a href="#-english">🇺🇸 English</a>
  &nbsp;&nbsp;&nbsp;|&nbsp;&nbsp;&nbsp;
  <a href="#-chinese">🇨🇳 中文</a>
</h3>

<div id="-english"></div>

## 🇺🇸 English

> **📦 Code Changes**: [View Diff](https://github.com/gjwang/zero_x_infinity/compare/v0.3-decimal-world...v0.4-btree-orderbook)

In the previous chapters, we completed the transition from Float to Integer and established a precision configuration system. However, our OrderBook data structure was still a "toy" implementation—re-sorting on every match! This chapter upgrades it to a truly production-ready data structure.

### 1. The Problem with the Naive Implementation

Let's review the original `engine.rs`:

```rust
pub struct OrderBook {
    bids: Vec<PriceLevel>,  // Was 'buys'
    asks: Vec<PriceLevel>,  // Was 'sells'
}
```

> 💡 **Naming Convention**: We renamed `buys/sells` to `bids/asks`. These are standard options industry terms:
> - **Bid**: Price buyers are willing to pay.
> - **Ask**: Price sellers are demanding.
>
> Using professional terminology aligns the code with industry docs and APIs.

```rust
fn match_buy(&mut self, buy_order: &mut Order) {
    // Problem 1: Re-sort every time! O(n log n)
    self.asks.sort_by_key(|l| l.price);
    
    for level in self.asks.iter_mut() {
        // ...matching logic...
    }
    
    // Problem 2: Removing empty levels shifts the whole array! O(n)
    self.asks.retain(|l| !l.orders.is_empty());
}

fn rest_order(&mut self, order: Order) {
    // Problem 3: Finding price level is a linear scan! O(n)
    let level = self.asks.iter_mut().find(|l| l.price == order.price);
    // ...
}
```

#### Time Complexity Analysis

| Operation | Vec Impl | Issue |
|-----------|----------|-------|
| Insert Order | O(n) | Linear scan for price level |
| Pre-match Sort | O(n log n) | Sort required before every match |
| Remove Empty Level | O(n) | Array element shifting |

In an active exchange with tens of thousands of orders per second, O(n) operations quickly become a performance bottleneck.

### 2. The BTreeMap Solution

Rust's standard library provides `BTreeMap`, a **Self-Balancing Binary Search Tree**:

```rust
use std::collections::BTreeMap;

pub struct OrderBook {
    /// Asks: price -> orders (Ascending, Lowest Price = Best Ask)
    asks: BTreeMap<u64, VecDeque<Order>>,
    
    /// Bids: (u64::MAX - price) -> orders (Trick: Highest Price First)
    bids: BTreeMap<u64, VecDeque<Order>>,
}
```

#### Key Trick: Key Design for Bids

`BTreeMap` sorts keys in ascending order by default. This works perfectly for **Asks** (lowest price first). But for **Bids**, we need highest price first.

Solution: Use `u64::MAX - price` as the key.

```rust
// Insert Bid
let key = u64::MAX - order.price;
self.bids.entry(key).or_insert_with(VecDeque::new).push_back(order);

// Read Real Price
let price = u64::MAX - key;
```

Thus, Price 100 becomes Key `u64::MAX - 100`, and Price 99 becomes `u64::MAX - 99`. Since `(u64::MAX - 100) < (u64::MAX - 99)`, Price 100 comes before Price 99!

#### Why not `Reverse` or Custom Comparator?

You might ask: Why not `BTreeMap<Reverse<u64>, ...>`?

**Comparison**:

| Approach | Issue |
|----------|-------|
| `BTreeMap<Reverse<u64>>` | `Reverse` is a wrapper; unwrapping on every access adds complexity. |
| Custom `Ord` | Requires a newtype wrapper, increasing boilerplate. |
| `u64::MAX - price` | **Zero-Cost Abstraction**: Two subtraction ops, easily inlined by compiler. |

**Key Advantages**:
*   **Simple**: Just two lines of code.
*   **Zero Overhead**: Subtraction is a single-cycle CPU instruction.
*   **Type Safe**: Key remains `u64`.
*   **No Overflow**: Price is always < `u64::MAX`.

#### Time Complexity Comparison

| Operation | Vec Impl | BTreeMap Impl |
|-----------|----------|---------------|
| Insert Order | O(n) | **O(log n)** |
| Match (No Sort) | - | **O(log n)** |
| Cancel Order | O(n) | O(n)* |
| Remove Empty Level | O(n) | **O(log n)** |
| Query Best Price | O(n) / O(n log n) | **O(1)**xx |

> *Note: Cancelling requires linear scan in `VecDeque` (O(n)). O(1) cancel requires an auxiliary HashMap index.
> **Note: `BTreeMap::first_key_value()` is amortized O(1).

### 3. New Data Models

#### Order

```rust
#[derive(Debug, Clone)]
pub struct Order {
    pub id: u64,
    pub price: u64,          // Internal Integer Price
    pub qty: u64,            // Original Qty
    pub filled_qty: u64,     // Filled Qty
    pub side: Side,
    pub order_type: OrderType,
    pub status: OrderStatus,
}
```

#### Trade

```rust
#[derive(Debug, Clone)]
pub struct Trade {
    pub id: u64,
    pub buyer_order_id: u64,
    pub seller_order_id: u64,
    pub price: u64,
    pub qty: u64,
}
```

#### OrderResult

```rust
pub struct OrderResult {
    pub order: Order,       // Updated Order
    pub trades: Vec<Trade>, // Generated Trades
}
```

### 4. Core API

```rust
impl OrderBook {
    /// Add order, return match result
    pub fn add_order(&mut self, order: Order) -> OrderResult;
    
    /// Cancel order
    pub fn cancel_order(&mut self, order_id: u64, price: u64, side: Side) -> bool;
    
    /// Get Best Bid
    pub fn best_bid(&self) -> Option<u64>;
    
    /// Get Best Ask
    pub fn best_ask(&self) -> Option<u64>;
    
    /// Get Spread
    pub fn spread(&self) -> Option<u64>;
}
```

### 5. Execution Results

```text
=== 0xInfinity: Stage 4 (BTree OrderBook) ===
Symbol: BTC_USDT (ID: 0)
Price Decimals: 2, Qty Display Decimals: 3

[1] Makers coming in...
    Order 1: Sell 10.000 BTC @ $100.00 -> New
    Order 2: Sell 5.000 BTC @ $102.00 -> New
    Order 3: Sell 5.000 BTC @ $101.00 -> New

    Book State: Best Bid=None, Best Ask=Some("100.00"), Spread=None

[2] Taker eats liquidity...
    Order 4: Buy 12.000 BTC @ $101.50
    Trades:
      - Trade #1: 10.000 @ $100.00
      - Trade #2: 2.000 @ $101.00
    Order Status: Filled, Filled: 12.000/12.000

    Book State: Best Bid=None, Best Ask=Some("101.00")

[3] More makers...
    Order 5: Buy 10.000 BTC @ $99.00 -> New

    Final Book State: Best Bid=Some("99.00"), Best Ask=Some("101.00"), Spread=Some("2.00")

=== End of Simulation ===
```

Observations:
*   Orders matched correctly by price priority (First $100, then $101).
*   Every trade recorded in `Trades`.
*   Real-time tracking of Best Bid/Ask and Spread.

### 6. Unit Tests

We added 8 unit tests covering core scenarios:

```bash
$ cargo test

running 8 tests
test engine::tests::test_add_resting_order ... ok
test engine::tests::test_cancel_order ... ok
test engine::tests::test_fifo_at_same_price ... ok
test engine::tests::test_full_match ... ok
test engine::tests::test_multiple_trades_single_order ... ok
test engine::tests::test_partial_match ... ok
test engine::tests::test_price_priority ... ok
test engine::tests::test_spread ... ok

test result: ok. 8 passed; 0 failed
```

### 7. Is BTreeMap Enough?

For an exchange **not chasing extreme performance**, BTreeMap is perfectly adequate:

| Scenario | BTreeMap Performance |
|----------|----------------------|
| 1,000 TPS | Easy |
| 10,000 TPS | Manageable |
| 100,000+ TPS | Need specialized structures |

If you want to build a **Ferrari-level** matching engine (nanosecond latency, millions of TPS), you need:
*   Lock-free data structures
*   Memory pools (avoid heap allocation)
*   CPU Cache optimization
*   FPGA acceleration

But that's for later. For now, we have a **Correct and Efficient** baseline implementation.

### Summary

This chapter accomplished:
1.  ✅ **Analyzed Problem**: O(n) bottleneck in Vec implementation.
2.  ✅ **Refactored to BTreeMap**: O(log n) insert/search/delete.
3.  ✅ **Defined Types**: Standard Order/Trade/OrderResult models.
4.  ✅ **Refined API**: best_bid/ask, spread, cancel_order.
5.  ✅ **Added Tests**: 8 tests covering core logic.

<br>
<div align="right"><a href="#-english">↑ Back to Top</a></div>
<br>

---

<div id="-chinese"></div>

## 🇨🇳 中文

> **📦 代码变更**: [查看 Diff](https://github.com/gjwang/zero_x_infinity/compare/v0.3-decimal-world...v0.4-btree-orderbook)

在前三章中，我们完成了从浮点数到整数的转换，并建立了精度配置系统。但我们的 OrderBook 数据结构还是一个"玩具"实现——每次撮合都需要重新排序！本章我们将把它升级为一个真正生产可用的数据结构。

### 1. 原有实现的问题

让我们回顾一下原来的 `engine.rs`：

```rust
pub struct OrderBook {
    bids: Vec<PriceLevel>,  // 原来叫 buys
    asks: Vec<PriceLevel>,  // 原来叫 sells
}
```

> 💡 **命名规范**：我们把 `buys/sells` 改名为 `bids/asks`。这是金融行业的标准术语：
> - **Bid**（买盘）：买方愿意出的价格
> - **Ask**（卖盘）：卖方要求的价格
>
> 使用专业术语可以让代码更易于与行业文档、API 对接。

```rust
fn match_buy(&mut self, buy_order: &mut Order) {
    // 问题 1: 每次都要重新排序！O(n log n)
    self.asks.sort_by_key(|l| l.price);
    
    for level in self.asks.iter_mut() {
        // ...matching logic...
    }
    
    // 问题 2: 删除空档位需要移动整个数组！O(n)
    self.asks.retain(|l| !l.orders.is_empty());
}

fn rest_order(&mut self, order: Order) {
    // 问题 3: 查找价格档位是线性扫描！O(n)
    let level = self.asks.iter_mut().find(|l| l.price == order.price);
    // ...
}
```

#### 时间复杂度分析

| 操作 | Vec 实现 | 问题 |
|------|---------|------|
| 插入订单 | O(n) | 线性查找价格档位 |
| 撮合前排序 | O(n log n) | 每次撮合都要排序 |
| 删除空档位 | O(n) | 数组元素移动 |

在一个活跃的交易所，每秒可能有数万笔订单。如果每笔订单都要 O(n) 操作，这里很快就会成为性能瓶颈。

### 2. BTreeMap 解决方案

Rust 标准库提供了 `BTreeMap`，它是一个**自平衡二叉搜索树**：

```rust
use std::collections::BTreeMap;

pub struct OrderBook {
    /// 卖单: price -> orders (按价格升序，最低价 = 最优卖价)
    asks: BTreeMap<u64, VecDeque<Order>>,
    
    /// 买单: (u64::MAX - price) -> orders (技巧：让最高价排在前面)
    bids: BTreeMap<u64, VecDeque<Order>>,
}
```

#### 关键技巧：买单的 Key 设计

BTreeMap 默认按 key 升序排列。对于卖单，这正好是我们想要的（最低价优先）。但对于买单，我们需要最高价优先。

解决方案：使用 `u64::MAX - price` 作为 key：

```rust
// 插入买单
let key = u64::MAX - order.price;
self.bids.entry(key).or_insert_with(VecDeque::new).push_back(order);

// 读取真实价格
let price = u64::MAX - key;
```

这样，价格 100 对应 key `u64::MAX - 100`，价格 99 对应 key `u64::MAX - 99`。由于 `(u64::MAX - 100) < (u64::MAX - 99)`，价格 100 会排在价格 99 前面！

#### 为什么不用 `Reverse` 或自定义比较器？

你可能会问：为什么不用 `BTreeMap<Reverse<u64>, ...>` 或者自定义比较器？

**方案对比**：

| 方案 | 问题 |
|------|------|
| `BTreeMap<Reverse<u64>, ...>` | `Reverse` 是一个 wrapper 类型，每次访问 key 都需要解包，增加代码复杂度 |
| 自定义 `Ord` trait | 需要创建 newtype wrapper，代码量大增 |
| `u64::MAX - price` | 零成本抽象：两次减法操作，编译器可以内联优化 |

**关键优势**：
- **简单**：只需要两行代码（插入时 `u64::MAX - price`，读取时再减回来）
- **零开销**：减法操作在 CPU 上是单周期指令
- **类型安全**：key 仍然是 `u64`，不需要额外的 wrapper 类型
- **无溢出风险**：价格永远小于 `u64::MAX`，减法不会溢出

#### 时间复杂度对比

| 操作 | Vec 实现 | BTreeMap 实现 |
|------|---------|--------------|
| 插入订单 | O(n) | **O(log n)** |
| 撮合（不排序） | - | **O(log n)** |
| 取消订单 | O(n) | O(n)* |
| 删除空价格档 | O(n) | **O(log n)** |
| 查询最优价 | O(n) 或 O(n log n) | **O(1)**xx |

> *注: 取消订单需要在 VecDeque 中线性查找订单 ID，这是 O(n)。如果需要 O(1) 取消，需要额外的 HashMap 索引。
>
> **注: BTreeMap 的 `first_key_value()` 是 O(1) 摊销复杂度。

### 3. 新的数据模型

#### Order（订单）

```rust
#[derive(Debug, Clone)]
pub struct Order {
    pub id: u64,
    pub price: u64,          // 价格（内部单位）
    pub qty: u64,            // 原始数量
    pub filled_qty: u64,     // 已成交数量
    pub side: Side,
    pub order_type: OrderType,
    pub status: OrderStatus,
}
```

#### Trade（成交记录）

```rust
#[derive(Debug, Clone)]
pub struct Trade {
    pub id: u64,
    pub buyer_order_id: u64,
    pub seller_order_id: u64,
    pub price: u64,
    pub qty: u64,
}
```

#### OrderResult（下单结果）

```rust
pub struct OrderResult {
    pub order: Order,      // 更新后的订单
    pub trades: Vec<Trade>, // 产生的成交
}
```

### 4. 核心 API

```rust
impl OrderBook {
    /// 添加订单，返回成交结果
    pub fn add_order(&mut self, order: Order) -> OrderResult;
    
    /// 取消订单
    pub fn cancel_order(&mut self, order_id: u64, price: u64, side: Side) -> bool;
    
    /// 获取最优买价
    pub fn best_bid(&self) -> Option<u64>;
    
    /// 获取最优卖价
    pub fn best_ask(&self) -> Option<u64>;
    
    /// 获取买卖价差
    pub fn spread(&self) -> Option<u64>;
}
```

### 5. 运行结果

```text
=== 0xInfinity: Stage 4 (BTree OrderBook) ===
Symbol: BTC_USDT (ID: 0)
Price Decimals: 2, Qty Display Decimals: 3

[1] Makers coming in...
    Order 1: Sell 10.000 BTC @ $100.00 -> New
    Order 2: Sell 5.000 BTC @ $102.00 -> New
    Order 3: Sell 5.000 BTC @ $101.00 -> New

    Book State: Best Bid=None, Best Ask=Some("100.00"), Spread=None

[2] Taker eats liquidity...
    Order 4: Buy 12.000 BTC @ $101.50
    Trades:
      - Trade #1: 10.000 @ $100.00
      - Trade #2: 2.000 @ $101.00
    Order Status: Filled, Filled: 12.000/12.000

    Book State: Best Bid=None, Best Ask=Some("101.00")

[3] More makers...
    Order 5: Buy 10.000 BTC @ $99.00 -> New

    Final Book State: Best Bid=Some("99.00"), Best Ask=Some("101.00"), Spread=Some("2.00")

=== End of Simulation ===
```

可以看到：
- 订单按价格优先级正确匹配（先 $100，再 $101）
- 每笔成交都记录在 `Trade` 中
- 实时追踪 Best Bid/Ask 和 Spread

### 6. 单元测试

我们添加了 8 个单元测试来验证核心功能：

```bash
$ cargo test

running 8 tests
test engine::tests::test_add_resting_order ... ok
test engine::tests::test_cancel_order ... ok
test engine::tests::test_fifo_at_same_price ... ok
test engine::tests::test_full_match ... ok
test engine::tests::test_multiple_trades_single_order ... ok
test engine::tests::test_partial_match ... ok
test engine::tests::test_price_priority ... ok
test engine::tests::test_spread ... ok

test result: ok. 8 passed; 0 failed
```

覆盖的场景包括：
- ✅ 订单挂单（无匹配）
- ✅ 完全成交
- ✅ 部分成交
- ✅ 价格优先级（Price Priority）
- ✅ 同价格 FIFO
- ✅ 取消订单
- ✅ 价差计算
- ✅ 一个大单吃掉多个小单

### 7. BTreeMap 够用吗？

对于一个**不追求极致性能**的交易所，BTreeMap 完全够用：

| 场景 | BTreeMap 表现 |
|------|-------------|
| 每秒 1000 单 | 轻松应对 |
| 每秒 10000 单 | 可以应对 |
| 每秒 100000+ 单 | 需要更专业的数据结构 |

如果你要打造一个**法拉利级别**的撮合引擎（纳秒级延迟、每秒百万单），需要考虑：
- 无锁数据结构
- 内存池（避免动态分配）
- CPU Cache 优化
- FPGA 硬件加速

但那是后话了。现在，我们有了一个**正确且高效**的基础实现。

### Summary

本章完成了以下工作：

1. ✅ **分析原有问题**：Vec 实现的 O(n) 复杂度瓶颈
2. ✅ **重构为 BTreeMap**：O(log n) 的插入、查找、删除
3. ✅ **定义规范类型**：Order、Trade、OrderResult
4. ✅ **完善 API**：best_bid/ask、spread、cancel_order
5. ✅ **添加单元测试**：8 个测试覆盖核心场景
