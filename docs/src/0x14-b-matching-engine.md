# 0x14-b Matching Engine: Feature Parity (Spot)

<h3>
  <a href="#-english">🇺🇸 English</a>
  &nbsp;&nbsp;&nbsp;|&nbsp;&nbsp;&nbsp;
  <a href="#-chinese">🇨🇳 中文</a>
</h3>

<div id="-english"></div>

## 🇺🇸 English

| Status | 🚧 **DESIGN PHASE** |
| :--- | :--- |
| **Context** | Phase V: Extreme Optimization (Step 2) |
| **Goal** | Achieve feature parity with Exchange-Core's Spot Matching Engine to support the Benchmark harness. |
| **Scope** | **Spot Only**. Margin/Futures deferred to 0x14-c. |

---

### 1. Gap Analysis

Based on code review of `src/engine.rs`, `src/models.rs`, `src/orderbook.rs`:

#### ✅ Already Implemented

| Feature | Location | Notes |
| :--- | :--- | :--- |
| **MatchingEngine** | `src/engine.rs` | `process_order()`, `match_buy()`, `match_sell()` |
| **Price-Time Priority** | `engine.rs:80-165` | Lowest ask first (buy), highest bid first (sell), FIFO |
| **Limit Orders** | `engine.rs:61-68` | Unfilled remainder rests in book |
| **Market Orders** | `engine.rs:90-94` | `u64::MAX` price for buy, matches all |
| **Order Status** | `models.rs:57-68` | NEW, PARTIALLY_FILLED, FILLED, CANCELED, REJECTED, EXPIRED |
| **OrderBook** | `orderbook.rs` | BTreeMap storage, `cancel_order()` by ID+price+side |

#### ❌ Missing (Required for 0x14-b)

| Feature | Generator Requirement | Current Status | Action |
| :--- | :--- | :--- | :--- |
| **TimeInForce** | `Gtc`, `Ioc` | **Not Implemented** | Add `TimeInForce` enum to `models.rs` |
| **IOC Logic** | Remainder expires, never rests | **Not Implemented** | Modify `process_order()` to check TIF |
| **CancelOrder Command** | `CommandType::CancelOrder` | `OrderBook::cancel_order()` exists but no Engine API | Expose via `Engine::cancel()` |
| **ReduceOrder Command** | `CommandType::ReduceOrder` | **Not Implemented** | Add `Engine::reduce_order()` |
| **MoveOrder Command** | `CommandType::MoveOrder` | **Not Implemented** | Add `Engine::move_order()` (cancel+place) |
| **FOKBudget** | Low usage in Spot | Not needed for MVP | Defer |

---

### 2. Architectural Requirements

#### 2.1 Data Model Extensions (Schema)

We must extend `InternalOrder` to support varied execution strategies without polluting the core `OrderType`.

**New Enum: `TimeInForce`**
```rust
pub enum TimeInForce {
    GTC, // Good Till Cancel (Default)
    IOC, // Immediate or Cancel (Taker only, cancel remainder)
    FOK, // Fill or Kill (All or Nothing) - Optional for now
}
```

**Updated `InternalOrder`**:
- Add `pub time_in_force: TimeInForce`
- Add `pub post_only: bool` (Future proofing, Generator doesn't strictly use it yet but good practice)

#### 2.2 Matching Engine Logic

The Matching Engine must process orders **sequentially** based on `seq_id`.

**Execution Flow**:
1.  **Incoming Order**: Parse `TimeInForce` and `OrderType`.
2.  **Matching**:
    *   **Limit GTC**: Match against opposite book. Remaining -> Add to Book.
    *   **Limit IOC**: Match against opposite book. Remaining -> **Expire** (do not add to book).
    *   **Market**: Match against opposite book at any price. Remaining -> Expire (or defined slippage protection).
3.  **Command Handling**:
    *   `MoveOrder`: Atomic "Cancel old ID + Place new ID". **Priority Loss** is acceptable (and expected).
    *   `ReduceOrder`: Reduce qty in-place. **Priority Preservation** required if implemented efficiently, else re-insert. Exchange-Core typically preserves priority on reduce.

#### 2.3 `FokBudget` Handling (Spot)
*   Generator produces `FokBudget`? -> Checks show mostly `Gtc`/`Ioc`.
*   *Correction*: `CommandType::FokBudget` exists in Generator enum but usage is rare in the Spot Benchmark. We prioritize **IOC** and **GTC**.

---

### 3. Developer Specification

#### 3.1 Task List
1.  **Model Update**:
    *   Modify `src/models.rs`: Add `TimeInForce` enum.
    *   Update `InternalOrder` struct.
2.  **Engine Implementation** (`src/engine/matching.rs`):
    *   Implement `process_order(&mut self, order: InternalOrder) -> OrderResult`.
    *   Implement `match_market_order`.
    *   Implement `match_limit_order`.
3.  **Command Logic**:
    *   Implement `reduce_order(price, old_qty, new_qty)`.
    *   Implement `move_order` (atomic cancel + place).

#### 3.2 Acceptance Criteria
*   **Unit Tests**:
    *   `test_ioc_partial_fill`: 100 qty order vs 60 qty book -> 60 filled, 40 expired.
    *   `test_gtc_maker`: 100 qty order vs empty book -> 100 rests in book.
    *   `test_market_sweep`: Market order consumes multiple price levels.

---

### 4. QA Verification Plan
*   **Property**: `Ioc` orders must **never** appear in `all_orders()` (the book) after processing.
*   **Property**: `Gtc` orders must appear in book if not fully matched.
*   **Latency**: Measure `process_order` time (target < 5µs for simple inserts).

<br>
<div align="right"><a href="#-english">↑ Back to Top</a></div>
<br>

---

<div id="-chinese"></div>

## 🇨🇳 中文

| 状态 | 🚧 **设计阶段** |
| :--- | :--- |
| **上下文** | Phase V: 极致优化 (Step 2) |
| **目标** | 实现与 Exchange-Core 现货撮合引擎的功能对齐，以支持基准测试工具。 |
| **范围** | **仅现货**。杠杆/期货推迟至 0x14-c。 |

---

### 1. 差距分析 (Gap Analysis)

当前的 Rust `models.rs` 和 `orderbook.rs` 不足以支持 `TestOrdersGenerator` 的输出要求。

| 特性 | 生成器输出 | 当前 Rust 模型 | 差距 |
| :--- | :--- | :--- | :--- |
| **生效时间 (TIF)** | `Gtc`, `Ioc` | 隐式 `GTC` | 缺少 `TimeInForce` 枚举 (必须支持 IOC)。 |
| **订单类型** | `PlaceOrder` (Limit), `FokBudget` | `Limit`, `Market` | `FokBudget` (按金额市价买入) 未定义。 |
| **修改操作** | `Cancel`, `Move`, `Reduce` | 仅 `Cancel` | `Move` (取消并替换) 和 `Reduce` (减仓) 逻辑缺失。 |
| **撮合逻辑** | 价格-时间优先 | 仅存储 (无引擎) | **无撮合引擎**。OrderBook 目前仅作为容器。 |

---

### 2. 架构需求

#### 2.1 数据模型扩展 (Schema)

必须扩展 `InternalOrder` 以支持多种执行策略。

**新枚举: `TimeInForce`**
```rust
pub enum TimeInForce {
    GTC, // Good Till Cancel (默认: 一直有效直到取消)
    IOC, // Immediate or Cancel (Taker 专用: 剩余未成交部分立即过期)
    FOK, // Fill or Kill (全部成交或全部取消) - 暂可选
}
```

**更新 `InternalOrder`**:
- 新增 `pub time_in_force: TimeInForce`
- 新增 `pub post_only: bool` (为未来准备，虽然生成器暂时未严格使用)

#### 2.2 撮合引擎逻辑

撮合引擎必须基于 `seq_id` **顺序处理** 订单。

**执行流**:
1.  **新订单接入**: 解析 `TimeInForce` 和 `OrderType`。
2.  **撮合过程**:
    *   **Limit GTC**: 与对手盘撮合。剩余部分 -> **加入订单簿**。
    *   **Limit IOC**: 与对手盘撮合。剩余部分 -> **立即过期 (Expire)** (不入簿)。
    *   **Market**: 与对手盘在任意价格撮合。剩余部分 -> 过期 (或滑点保护)。
3.  **指令处理**:
    *   `MoveOrder`: 原子化 "取消旧ID + 下单新ID"。**优先级丢失** 是可接受的 (且预期的)。
    *   `ReduceOrder`: 原地减少数量。如果实现得当，应**保留优先级**。Exchange-Core 通常在减量时保留优先级。

#### 2.3 `FokBudget` 处理 (现货)
*   生成器会产生 `FokBudget` 吗？ -> 代码显示主要是 `Gtc`/`Ioc`。
*   *修正*: `CommandType::FokBudget` 存在于枚举中，但在现货 Benchmark 中极少使用。我们优先保证 **IOC** 和 **GTC** 的正确性。

---

### 3. 开发规范 (Developer Specification)

#### 3.1 任务清单
1.  **模型更新**:
    *   修改 `src/models.rs`: 增加 `TimeInForce` 枚举。
    *   更新 `InternalOrder` 结构体。
2.  **引擎实现** (`src/engine/matching.rs`):
    *   实现 `process_order(&mut self, order: InternalOrder) -> OrderResult`。
    *   实现 `match_market_order` (市价撮合)。
    *   实现 `match_limit_order` (限价撮合)。
3.  **指令逻辑**:
    *   实现 `reduce_order(price, old_qty, new_qty)`。
    *   实现 `move_order` (atomic cancel + place)。

#### 3.2 验收标准
*   **单元测试**:
    *   `test_ioc_partial_fill`: 100 qty 订单 vs 60 qty 深度 -> 成交 60, 过期 40。
    *   `test_gtc_maker`: 100 qty 订单 vs 空订单簿 -> 100 进入 OrderBook。
    *   `test_market_sweep`: 市价单吃掉多个价格档位。

---

### 4. QA 验证计划
*   **属性**: `Ioc` 订单处理后，**绝不** 应出现在 `all_orders()` (订单簿) 中。
*   **属性**: `Gtc` 订单若未完全成交，**必须** 出现在订单簿中。
*   **延迟**: 测量 `process_order` 处理时间 (目标: 单次插入 < 5µs)。

<br>
<div align="right"><a href="#-chinese">↑ 回到顶部</a></div>
<br>
