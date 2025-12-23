# 0x08-d Complete Order Lifecycle & Cancel Optimization

<h3>
  <a href="#-english">🇺🇸 English</a>
  &nbsp;&nbsp;&nbsp;|&nbsp;&nbsp;&nbsp;
  <a href="#-chinese">🇨🇳 中文</a>
</h3>

<div id="-english"></div>

## 🇺🇸 English

> **📦 Code Changes**: [View Diff](https://github.com/gjwang/zero_x_infinity/compare/v0.8-c-complete-event-flow...v0.8-d-complete-order-lifecycle)

> **Core Objective**: Implement full order lifecycle management (including Cancel and Refund), design a dual-track testing framework, and analyze performance bottlenecks.

---

## 1. Feature Implementation Overview

In this chapter, we completed the following core features to equip the trading engine with full order processing capabilities:

### 1.1 Order Events & State Management

Implemented complete `OrderEvent` enum and CSV logging.

**OrderStatus (src/models.rs)**:
Follows Binance-style Screaming Snake Case.
```rust
pub enum OrderStatus {
    NEW,              // Booked
    PARTIALLY_FILLED, 
    FILLED,           
    CANCELED,         // User Cancelled
    REJECTED,         // Risk Check Failed
    EXPIRED,          // System Expired
}
```

**OrderEvent (src/messages.rs)**:
Used for Event Sourcing and Audit Logs.

| Event Type | Trigger | Fund Operation |
|------------|---------|----------------|
| `Accepted` | Passed risk check | `Lock` |
| `Rejected` | Insufficient balance/Bad params | None |
| `Filled` | Fully filled | `Settle` |
| `PartialFilled` | Partially filled | `Settle` |
| `Cancelled` | User cancel | `Unlock` (Refund remaining) |
| `Expired` | System expired | `Unlock` |

**CSV Log Format (output/t2_order_events.csv)**:
```csv
event_type,order_id,user_id,seq_id,filled_qty,remaining_qty,price,reason
accepted,1,100,101,,,,
rejected,3,102,103,,,,insufficient_balance
partial_filled,1,100,,5000,1000,,
filled,1,100,,0,,85000,
cancelled,5,100,,,2000,,
```

### 1.2 Cancel Workflow

1.  **Parsing**: `scripts/csv_io.rs` supports `action=cancel`.
2.  **Removal**: `MatchingEngine` calls `OrderBook::remove_order_by_id`.
3.  **Unlock**: `UBSCore` generates `Unlock` event to refund frozen funds.
4.  **Logging**: Record `Cancelled` event.

---

## 2. Dual-Track Testing Framework

To guarantee baseline stability while adding new features:

### 2.1 Regression Baseline
*   **Dataset**: `fixtures/orders.csv` (100k orders, Place only).
*   **Script**: `scripts/test_e2e.sh`
*   **Goal**: Ensure no performance regression for legacy flows.

### 2.2 Feature Testing
*   **Dataset**: `fixtures/test_with_cancel/orders.csv` (1M orders, 30% Cancel).
*   **Script**: `scripts/test_cancel.sh`
*   **Goal**: Verify lifecycle closure (Lock = Settle + Unlock).

---

## 3. Major Performance Issue

When scaling Cancel tests from 1,000 to **1,000,000** orders, we hit a severe performance wall.

### 3.1 Symptoms
*   **Baseline (100k Place)**: ~3 seconds.
*   **Cancel Test (1M Place+Cancel)**: **> 7 minutes (430s)**.
*   **Bottleneck**: `Matching Engine` consumes 98% CPU.

### 3.2 Root Cause Analysis

The culprit is `OrderBook::remove_order_by_id`:

```rust
// src/orderbook.rs
pub fn remove_order_by_id(&mut self, order_id: u64) -> Option<InternalOrder> {
    // Scan ALL price levels -> Scan ALL orders in level
    for (key, orders) in self.bids.iter_mut() {
        if let Some(pos) = orders.iter().position(|o| o.order_id == order_id) {
            // ...
        }
    }
    // Scan asks...
}
```

*   **Complexity**: **O(N)**.
*   **Worst Case**: With 500k orders piled up in the book, executing 300k cancels means **150 billion comparisons**.

### 3.3 Solution (Next Step)
Introduce **Order Index**:
*   **Structure**: `HashMap<OrderId, (Price, Side)>`.
*   **Complexity**: Reduces Cancel from O(N) to **O(1)**.

---

## 4. Verification Scripts

1.  `verify_balance_events.py`:
    *   Added `Check 8`: Verify Frozen Balance history consistency.
    *   Verify `Unlock` events correctly release funds.

2.  `verify_order_events.py`:
    *   Verify every `Accepted` order has a final state.
    *   Verify `Cancelled` orders correspond to existing `Accepted` orders.

---

## 5. Summary

We implemented full order lifecycle management and established a rigorous testing framework. Crucially, mass stress testing exposed a **Big O algorithm defect** in the cancel logic, setting the stage for the next optimization iteration.

<br>
<div align="right"><a href="#-english">↑ Back to Top</a></div>
<br>

---

<div id="-chinese"></div>

## 🇨🇳 中文

> **📦 代码变更**: [查看 Diff](https://github.com/gjwang/zero_x_infinity/compare/v0.8-c-complete-event-flow...v0.8-d-complete-order-lifecycle)

> **核心目标**：实现订单全生命周期管理（含撤单、退款），设计双轨制测试框架，并深入分析引入的性能瓶颈。

---

## 1. 功能实现概览

在本章中，我们完成了以下核心功能，使交易引擎具备了完整的订单处理能力：

### 1.1 订单事件与状态管理
实现了完整的 `OrderEvent` 枚举与 CSV 日志记录。

**OrderStatus (src/models.rs)**:
注意遵循 Binance 风格的 Screaming Snake Case。
```rust
pub enum OrderStatus {
    NEW,              // 挂单中
    PARTIALLY_FILLED, // 部分成交
    FILLED,           // 完全成交
    CANCELED,         // 用户撤单 (注意拼写 CANCELED)
    REJECTED,         // 风控拒绝
    EXPIRED,          // 系统过期
}
```

**OrderEvent (src/messages.rs)**:
用于 Event Sourcing 和审计日志。
| 事件类型 | 触发场景 | 资金操作 |
|---|---|---|
| `Accepted` | 订单通过风控并进入撮合 | `Lock` (冻结) |
| `Rejected` | 余额不足或参数错误 | 无 |
| `Filled` | 完全成交 | `Settle` (结算) |
| `PartialFilled` | 部分成交 | `Settle` (结算) |
| `Cancelled` | 用户撤单 (注意拼写 Cancelled) | `Unlock` (解冻剩余资金) |
| `Expired` | 系统过期 | `Unlock` (解冻) |

**CSV 日志格式 (output/t2_order_events.csv)**:
实际代码实现的列顺序如下：
```csv
event_type,order_id,user_id,seq_id,filled_qty,remaining_qty,price,reason
accepted,1,100,101,,,,
rejected,3,102,103,,,,insufficient_balance
partial_filled,1,100,,5000,1000,,
filled,1,100,,0,,85000,
cancelled,5,100,,,2000,,
```

### 1.2 撤单流程 (Cancel Workflow)
实现了 `cancel` 动作的处理流程：
1.  **输入解析**: `scripts/csv_io.rs` 支持新旧两种 CSV 格式。
    *   新格式: `order_id,user_id,action,side,price,qty` (支持 `action=cancel`)。
2.  **撮合移除**: `MatchingEngine` 调用 `OrderBook::remove_order_by_id` 移除订单。
3.  **资金解锁**: `UBSCore` 生成 `Unlock` 事件，返还冻结资金。
4.  **事件记录**: 记录 `Cancelled` 事件。

---

## 2. 双轨制测试框架

为了在引入新功能的同时保证原有基准不被破坏，我们设计了**双轨制测试策略**：

### 2.1 原始基准 (Regression Baseline)
*   **数据集**: `fixtures/orders.csv` (10万订单，仅 Place)。
*   **脚本**: `scripts/test_e2e.sh`
*   **目的**: 确保传统撮合性能不回退，验证核心正确性。
*   **原则**: **保持基准稳定** (非必要不修改，除非格式升级或重大调整)。

### 2.2 新功能测试 (Feature Testing)
*   **数据集**: `fixtures/test_with_cancel/orders.csv` (100万订单，含30% Cancel)。
*   **脚本**: `scripts/test_cancel.sh`
*   **验证**:
    *   `verify_balance_events.py`: 验证资金守恒 (Lock = Settle + Unlock)。
    *   `verify_order_events.py`: 验证订单生命周期闭环。

---

## 3. 重大性能问题分析 (Major Issue)

在将撤单测试规模从 1000 扩大到 100万 时，我们发现了一个严重的性能崩塌现象。

### 3.1 现象
*   **基准测试 (10万 Place)**: 耗时 ~3秒。
*   **撤单测试 (100万 Place+Cancel)**: 耗时 **超过 7分钟 (430秒)**。
*   **瓶颈定位**: `Matching Engine` 耗时占比 98%。

### 3.2 原因深入分析
通过代码审查，我们发现瓶颈在于 `OrderBook::remove_order_by_id` 的实现：

```rust
// src/orderbook.rs
pub fn remove_order_by_id(&mut self, order_id: u64) -> Option<InternalOrder> {
    // 遍历卖单簿的所有价格层级 --> 遍历每个层级的所有订单
    for (key, orders) in self.bids.iter_mut() {
        if let Some(pos) = orders.iter().position(|o| o.order_id == order_id) {
            // ...
        }
    }
    // 遍历买单簿...
}
```

*   **复杂度**: **O(N)**，其中 N 是当前 OrderBook 中的订单总数。
*   **数据分布恶化**: 在 `test_with_cancel` 数据集中，由于缺乏激进的“吃单”逻辑，大量订单堆积在撮合簿中（未成交）。假设盘口堆积了 50万 订单。
*   **计算量**: 执行 30万 次撤单，每次遍历 50万 数据 = **1500亿次 CPU 比较操作**。

这解释了为什么系统在处理大规模撤单时速度极慢。

### 3.3 解决方案 (Next Step)
为了解决此问题，必须引入**订单索引 (Order Index)**：
*   **结构**: `HashMap<OrderId, (Price, Side)>`。
*   **优化后复杂度**: 撤单查找从 O(N) 降为 **O(1)**。

---

## 4. 验证脚本

我们提供了两个 Python脚本用于验证逻辑正确性：

1.  `verify_balance_events.py`:
    *   新增 `Check 8`: 验证 Frozen Balance 的历史一致性。
    *   验证 `Unlock` 事件是否正确释放了资金。

2.  `verify_order_events.py`:
    *   验证所有 `Accepted` 订单最终都有终态 (Filled/Cancelled/Rejected)。
    *   验证 `Cancelled` 订单真的对应了相应的 `Accepted` 事件。

## 5. 总结

本章不仅完成了功能的开发，更重要的是建立了**数据隔离的测试体系**，并通过大规模压测暴露了**算法复杂度缺陷**。这为下一步的持续迭代奠定了坚实基础。
