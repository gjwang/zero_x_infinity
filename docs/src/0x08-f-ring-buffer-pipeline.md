# 0x08-f Ring Buffer Pipeline Implementation

<h3>
  <a href="#-english">🇺🇸 English</a>
  &nbsp;&nbsp;&nbsp;|&nbsp;&nbsp;&nbsp;
  <a href="#-chinese">🇨🇳 中文</a>
</h3>

<div id="-english"></div>

## 🇺🇸 English

> **📦 Code Changes**: [View Diff](https://github.com/gjwang/zero_x_infinity/compare/v0.8-e-perf-bottleneck-profiling...v0.8-f-ring-buffer-pipeline)

> **Goal**: Connect services using Ring Buffers to implement a true Pipeline architecture.

---

## Part 1: Single-Thread Pipeline

### 1.1 Background

**Legacy Execution (Synchronous Serial)**:

```
for order in orders:
    1. ubscore.process_order(order)     # WAL + Lock
    2. engine.process_order(order)       # Match
    3. ubscore.settle_trade(trade)       # Settle
    4. ledger.write(event)               # Persist
```

**Problem**: No pipeline parallelism, latency accumulates.

### 1.2 Single-Thread Pipeline Architecture

Decouple services using Ring Buffers, but polling within a single thread loop:

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    Single-Thread Pipeline (Round-Robin)                  │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│   Stage 1: Ingestion          →  order_queue                            │
│   Stage 2: UBSCore Pre-Trade  →  valid_order_queue                      │
│   Stage 3: Matching Engine    →  trade_queue                            │
│   Stage 4: Settlement         →  (Ledger)                               │
│                                                                          │
│   All Stages executed in a round-robin loop                              │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

**Core Data Structures**:

```rust
pub struct PipelineQueues {
    pub order_queue: Arc<ArrayQueue<SequencedOrder>>,
    pub valid_order_queue: Arc<ArrayQueue<ValidOrder>>,
    pub trade_queue: Arc<ArrayQueue<TradeEvent>>,
}
```

**Execution Loop**:

```rust
loop {
    // UBSCore: order_queue → valid_order_queue
    if let Some(order) = queues.order_queue.pop() {
        // ...
    }
    
    // ME: valid_order_queue → trade_queue
    if let Some(valid_order) = queues.valid_order_queue.pop() {
        // ...
    }
    
    // Settlement: trade_queue → persist
    if let Some(trade) = queues.trade_queue.pop() {
        // ...
    }
}
```

---

## Part 2: Multi-Thread Pipeline

### 2.1 Architecture

Full Multi-Threaded Pipeline based on 0x08-a design:

```
┌───────────────────────────────────────────────────────────────────────────────────────┐
│                          Multi-Thread Pipeline (Full)                                  │
├───────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                        │
│  Thread 1: Ingestion       Thread 2: UBSCore              Thread 3: ME                │
│  ┌─────────────────┐       ┌──────────────────────┐       ┌─────────────────┐         │
│  │ Read orders     │       │  PRE-TRADE:          │       │ Match Order     │         │
│  │ Assign SeqNum   │──────▶│  - Write WAL         │──────▶│ in OrderBook    │         │
│  │                 │   ①   │  - process_order()   │  ③    │                 │         │
│  └─────────────────┘       │  - lock_balance()    │       │ Generate        │         │
│                            │                      │       │ TradeEvents     │         │
│                            └──────────┬───────────┘       └────────┬────────┘         │
│                                       ▲                            │                  │
│                                       │                            │                  │
│                                       │ ⑤ balance_update_queue     │ ④ trade_queue   │
│                                       └────────────────────────────┤                  │
│                                                                    │                  │
│                            ┌──────────────────────┐                ▼                  │
│                            │  POST-TRADE:         │       ┌─────────────────┐         │
│                            │  - settle_trade()    │       │ Thread 4:       │         │
│                            │  - spend_frozen()    │──────▶│ Settlement      │         │
│                            │  - deposit()         │  ⑥    │                 │         │
│                            │  - Generate Balance  │       │ Persist:        │         │
│                            │    Update Events     │       │ - Trade Events  │         │
│                            └──────────────────────┘       │ - Balance Events│         │
│                                                           │ - Ledger        │         │
│                                                           └─────────────────┘         │
│                                                                                        │
└───────────────────────────────────────────────────────────────────────────────────────┘
```

### 2.2 Key Design Points

1.  **ME Fan-out**: ME sends `TradeEvent` in parallel to:
    *   `trade_queue` → Settlement (Persist)
    *   `balance_update_queue` → UBSCore (Balance Settle)
2.  **UBSCore as Single Balance Entry**: Handles Pre-Trade Lock, Post-Trade Settle, and Refunds.
3.  **Settlement Consolidation**: Consumes both Trade Events and Balance Events.

### 2.3 Data Types

**BalanceUpdateRequest (ME → UBSCore)**:
Contains Trade Event and optional Price Improvement data.

**BalanceEvent (UBSCore → Settlement)**:
The unified channel for ALL balance changes (Lock, Settle, Credit, Refund).

```rust
pub enum BalanceEventType {
    Lock,           // Pre-Trade
    SpendFrozen,    // Post-Trade
    Credit,         // Post-Trade
    RefundFrozen,   // Price Improvement
    // ...
}
```

### 2.4 Implementation Status

| Component | Status |
|-----------|--------|
| All Queues | ✅ Implemented |
| UBSCore BalanceEvent Gen | ✅ Implemented |
| Settlement Persistence | ✅ Implemented |

---

## Verification & Performance (2025-12-17)

### Correctness
E2E tests pass for both pipeline modes.

### Performance Comparison

**1.3M Orders (with 300k Cancel)**:

| Mode | Time | Throughput | Trades |
|------|------|------------|--------|
| UBSCore (Baseline) | 23.5s | 55k ops/s | 538,487 |
| Single-Thread Pipeline | 22.1s | 59k ops/s | 538,487 |
| Multi-Thread Pipeline | **29.1s** | **45k ops/s** | 489,804 |

*   **Issue**: Multi-Thread mode is currently **slower** (-30%) on large datasets and skips cancel orders.

**100k Orders (Place only)**:

| Mode | Time | Throughput | vs Baseline |
|------|------|------------|-------------|
| UBSCore | 755ms | 132k ops/s | - |
| Single-Thread | 519ms | 193k ops/s | +46% |
| **Multi-Thread** | **391ms** | **256k ops/s** | **+93%** |

*   **Observation**: Multi-threading shines on smaller, simpler datasets (**+93%**).

### Analysis
Multi-threaded pipeline overhead (context switching, queue contention, event generation) outweighs benefits when per-order processing time is very low (due to optimizations). Also, missing Cancel logic reduces correctness.

---

## Key Design Decisions

*   **Backpressure**: Spin Wait (prioritize low latency).
*   **Shutdown**: Graceful drain using Atomic Signals.
*   **Error Handling**: Logging and metric counting; critical paths must succeed.

<br>
<div align="right"><a href="#-english">↑ Back to Top</a></div>
<br>

---

<div id="-chinese"></div>

## 🇨🇳 中文

> **📦 代码变更**: [查看 Diff](https://github.com/gjwang/zero_x_infinity/compare/v0.8-e-perf-bottleneck-profiling...v0.8-f-ring-buffer-pipeline)

> **目标**：使用 Ring Buffer 串接不同服务，实现真正的 Pipeline 架构

---

## Part 1: 单线程 Pipeline

### 1.1 背景

**原始执行模式 (同步串行)**:

```
for order in orders:
    1. ubscore.process_order(order)     # WAL + Lock
    2. engine.process_order(order)       # Match
    3. ubscore.settle_trade(trade)       # Settle
    4. ledger.write(event)               # Persist
```

**问题**：没有 Pipeline 并行，延迟累加

### 1.2 单线程 Pipeline 架构

使用 Ring Buffer 解耦各服务，但仍在单线程中轮询执行：

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    Single-Thread Pipeline (Round-Robin)                  │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│   Stage 1: Ingestion          →  order_queue                            │
│   Stage 2: UBSCore Pre-Trade  →  valid_order_queue                      │
│   Stage 3: Matching Engine    →  trade_queue                            │
│   Stage 4: Settlement         →  (Ledger)                               │
│                                                                          │
│   所有 Stage 在同一个 while 循环中轮询执行                               │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

**核心数据结构**:

```rust
pub struct PipelineQueues {
    pub order_queue: Arc<ArrayQueue<SequencedOrder>>,
    pub valid_order_queue: Arc<ArrayQueue<ValidOrder>>,
    pub trade_queue: Arc<ArrayQueue<TradeEvent>>,
}
```

**执行流程**:

```rust
loop {
    // UBSCore: order_queue → valid_order_queue
    if let Some(order) = queues.order_queue.pop() {
        // ...
    }
    
    // ME: valid_order_queue → trade_queue
    if let Some(valid_order) = queues.valid_order_queue.pop() {
        // ...
    }
    
    // Settlement: trade_queue → persist
    if let Some(trade) = queues.trade_queue.pop() {
        // ...
    }
}
```

---

## Part 2: 多线程 Pipeline

### 2.1 架构

根据 0x08-a 原始设计，完整的多线程 Pipeline 数据流如下：

```
┌───────────────────────────────────────────────────────────────────────────────────────┐
│                          Multi-Thread Pipeline (完整版)                                │
├───────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                        │
│  Thread 1: Ingestion       Thread 2: UBSCore              Thread 3: ME                │
│  ┌─────────────────┐       ┌──────────────────────┐       ┌─────────────────┐         │
│  │ Read orders     │       │  PRE-TRADE:          │       │ Match Order     │         │
│  │ Assign SeqNum   │──────▶│  - Write WAL         │──────▶│ in OrderBook    │         │
│  │                 │   ①   │  - process_order()   │  ③    │                 │         │
│  └─────────────────┘       │  - lock_balance()    │       │ Generate        │         │
│                            │                      │       │ TradeEvents     │         │
│                            └──────────┬───────────┘       └────────┬────────┘         │
│                                       ▲                            │                  │
│                                       │                            │                  │
│                                       │ ⑤ balance_update_queue     │ ④ trade_queue   │
│                                       └────────────────────────────┤                  │
│                                                                    │                  │
│                            ┌──────────────────────┐                ▼                  │
│                            │  POST-TRADE:         │       ┌─────────────────┐         │
│                            │  - settle_trade()    │       │ Thread 4:       │         │
│                            │  - spend_frozen()    │──────▶│ Settlement      │         │
│                            │  - deposit()         │  ⑥    │                 │         │
│                            │  - Generate Balance  │       │ Persist:        │         │
│                            │    Update Events     │       │ - Trade Events  │         │
│                            └──────────────────────┘       │ - Balance Events│         │
│                                                           │ - Ledger        │         │
│                                                           └─────────────────┘         │
│                                                                                        │
└───────────────────────────────────────────────────────────────────────────────────────┘
```

### 2.2 关键设计点

1. **ME Fan-out**: ME 将 `TradeEvent` **并行**发送到：
   - `trade_queue` → Settlement (持久化交易记录)
   - `balance_update_queue` → UBSCore (余额结算)
2. **UBSCore 是余额操作的唯一入口**: 处理 Pre-Trade 锁定、Post-Trade 结算和退款。
3. **Settlement 聚合**: 同时消费交易事件和余额事件。

### 2.3 数据类型

**BalanceUpdateRequest (ME → UBSCore)**:
包含成交事件和可能的价格改善(Price Improvement)数据。

**BalanceEvent (UBSCore → Settlement)**:
所有余额变更的统一通道 (Lock, Settle, Credit, Refund)。

```rust
pub enum BalanceEventType {
    Lock,           // Pre-Trade
    SpendFrozen,    // Post-Trade
    Credit,         // Post-Trade
    RefundFrozen,   // Price Improvement
    // ...
}
```

### 2.4 实现状态

| 组件 | 状态 |
|------|------|
| 所有队列 | ✅ 已实现 |
| UBSCore BalanceEvent 生成 | ✅ 已实现 |
| Settlement 持久化 | ✅ 已实现 |

---

## 验证与性能 (2025-12-17)

### 正确性
E2E 测试在两种模式下均通过。

### 性能对比

**1.3M 订单 (含 30 万撤单)**:

| 模式 | 执行时间 | 吞吐量 | 成交数 |
|------|----------|--------|--------|
| UBSCore (Baseline) | 23.5s | 55k ops/s | 538,487 |
| 单线程 Pipeline | 22.1s | 59k ops/s | 538,487 |
| 多线程 Pipeline | **29.1s** | **45k ops/s** | 489,804 |

*   **问题**: 多线程模式在大数据集上反而**更慢** (-30%)，且目前跳过了撤单处理。

**100k 订单 (仅 Place)**:

| 模式 | 时间 | 吞吐量 | 提升 |
|------|------|--------|------|
| UBSCore | 755ms | 132k ops/s | - |
| 单线程 | 519ms | 193k ops/s | +46% |
| **多线程** | **391ms** | **256k ops/s** | **+93%** |

*   **观察**: 多线程在简单的小数据集上表现出色 (**+93%**)。

### 分析
在单笔处理极快的情况下，多线程带来的开销（上下文切换、队列竞争、事件生成）超过了并行的收益。此外，缺失撤单逻辑降低了正确性。

---

## 关键设计决策

*   **背压**: 自旋等待 (Spin Wait)，优先低延迟。
*   **关闭**: 使用原子信号优雅退出。
*   **错误处理**: 日志记录，核心路径必须成功。
