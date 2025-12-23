# 0x08-g Multi-Thread Pipeline Design

<h3>
  <a href="#-english">🇺🇸 English</a>
  &nbsp;&nbsp;&nbsp;|&nbsp;&nbsp;&nbsp;
  <a href="#-chinese">🇨🇳 中文</a>
</h3>

<div id="-english"></div>

## 🇺🇸 English

> **📦 Code Changes**: [View Diff](https://github.com/gjwang/zero_x_infinity/compare/v0.8-f-ring-buffer-pipeline...v0.8-h-performance-monitoring) | [Key File: pipeline_mt.rs](https://github.com/gjwang/zero_x_infinity/blob/main/src/pipeline_mt.rs)

## Overview

The Multi-Thread Pipeline distributes processing logic across 4 independent threads, communicating via lock-free queues to achieve high throughput order processing.

## Architecture

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│  Ingestion  │────▶│   UBSCore   │────▶│     ME      │────▶│ Settlement  │
│  (Thread 1) │     │  (Thread 2) │     │  (Thread 3) │     │  (Thread 4) │
└─────────────┘     └─────────────┘     └─────────────┘     └─────────────┘
      │                   │ ▲                 │                   │
      │                   │ │                 │                   │
      ▼                   ▼ │                 ▼                   ▼
  order_queue ────▶ action_queue      balance_update_queue   trade_queue
                           │                                balance_event_queue
                           └──────────────────────────────────────┘
```

### Thread Responsibilities

| Thread | Responsibility | Input Queue | Output |
|--------|----------------|-------------|--------|
| **Ingestion** | Parse orders, assign SeqNum | orders (iterator) | order_queue |
| **UBSCore** | Pre-Trade (WAL + Lock) + Post-Trade (Settle) | order_queue, balance_update_queue | action_queue, balance_event_queue |
| **ME** | Match, Cancel handling | action_queue | trade_queue, balance_update_queue |
| **Settlement** | Persist Events (Trade, Balance) | trade_queue, balance_event_queue | ledgers |

## Queue Design

Using `crossbeam-queue::ArrayQueue` for lock-free MPSC queues:

```rust
pub struct MultiThreadQueues {
    pub order_queue: Arc<ArrayQueue<OrderAction>>,     // 64K
    pub action_queue: Arc<ArrayQueue<ValidAction>>,    // 64K
    pub trade_queue: Arc<ArrayQueue<TradeEvent>>,      // 64K
    pub balance_update_queue: Arc<ArrayQueue<BalanceUpdateRequest>>,  // 64K
    pub balance_event_queue: Arc<ArrayQueue<BalanceEvent>>,           // 64K
}
```

## Cancel Handling

1.  **Ingestion**: Create `OrderAction::Cancel`.
2.  **UBSCore**: Pass to `action_queue` (No lock needed).
3.  **ME**: Remove from OrderBook, send `BalanceUpdateRequest::Cancel`.
4.  **UBSCore**: Process unlock, generate `BalanceEvent::Unlock`.
5.  **Settlement**: Persist `BalanceEvent`.

## Consistency Verification

### Test Script

```bash
# Run full comparison test
./scripts/test_pipeline_compare.sh highbal

# Supported Datasets:
#   100k    - 100k orders without cancel
#   cancel  - 1.3M orders with 30% cancel
#   highbal - 1.3M orders with 30% cancel, high balance (Recommended)
```

### Verification Results (1.3M orders, 30% cancel, high balance)

```
╔════════════════════════════════════════════════════════════════╗
║                    ✅ ALL TESTS PASSED                         ║
║  Multi-thread pipeline matches single-thread exactly!          ║
╚════════════════════════════════════════════════════════════════╝
```

### Key Metrics

| Dataset | Total | Place | Cancel | Trades | Result |
|---------|-------|-------|--------|--------|--------|
| 100k | 100,000 | 100,000 | 0 | 47,886 | ✅ Match |
| 1.3M HighBal | 1,300,000 | 1,000,000 | 300,000 | 667,567 | ✅ Match |

## Important Considerations

### Balance Sufficiency
Insufficient balance may cause rejections. In concurrent environments, rejection timing can vary due to settlement latency, leading to non-deterministic results.
**Solution**: Use `highbal` dataset (1000 BTC + 100M USDT per user).

### Shutdown Synchronization
Wait for queues to drain before signaling shutdown:

```rust
while !queues.all_empty() {
    std::hint::spin_loop();
}
shutdown.request_shutdown();
```

## Performance

| Mode | 100k orders | 1.3M orders |
|------|-------------|-------------|
| Single-Thread | 350ms | 15.5s |
| Multi-Thread | 330ms | 15.6s |

**Note**: Multi-thread version includes overhead for BalanceEvent generation/persistence, matching Single-Thread performance. Future optimizations: Batch I/O, reduce contention.

## Queue Priority Strategy (Future)

**Current Implementation**:
Prioritize draining `balance_update_queue` completely before processing `order_queue`.

**Future: Weighted Round-Robin**:
Allow alternating processing to improve responsiveness.

```rust
const SETTLE_WEIGHT: u32 = 3;  // settle : order = 3 : 1
```

## File Structure

```
src/
├── pipeline.rs       # Shared types
├── pipeline_mt.rs    # Multi-thread impl
├── pipeline_runner.rs # Single-thread impl
└── main.rs
```

<br>
<div align="right"><a href="#-english">↑ Back to Top</a></div>
<br>

---

<div id="-chinese"></div>

## 🇨🇳 中文

> **📦 代码变更**: [查看 Diff](https://github.com/gjwang/zero_x_infinity/compare/v0.8-f-ring-buffer-pipeline...v0.8-h-performance-monitoring) | [关键文件: pipeline_mt.rs](https://github.com/gjwang/zero_x_infinity/blob/main/src/pipeline_mt.rs)

## 概述

Multi-Thread Pipeline 将处理逻辑分布在 4 个独立线程中，通过无锁队列通信，实现高吞吐量的订单处理。

## 架构

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│  Ingestion  │────▶│   UBSCore   │────▶│     ME      │────▶│ Settlement  │
│  (Thread 1) │     │  (Thread 2) │     │  (Thread 3) │     │  (Thread 4) │
└─────────────┘     └─────────────┘     └─────────────┘     └─────────────┘
      │                   │ ▲                 │                   │
      │                   │ │                 │                   │
      ▼                   ▼ │                 ▼                   ▼
  order_queue ────▶ action_queue      balance_update_queue   trade_queue
                           │                                balance_event_queue
                           └──────────────────────────────────────┘
```

### 线程职责

| 线程 | 职责 | 输入队列 | 输出 |
|------|------|----------|------|
| **Ingestion** | 订单解析、序列号分配 | orders (iterator) | order_queue |
| **UBSCore** | Pre-Trade (WAL + Lock) + Post-Trade (Settle) | order_queue, balance_update_queue | action_queue, balance_event_queue |
| **ME** | 订单撮合、取消处理 | action_queue | trade_queue, balance_update_queue |
| **Settlement** | 事件持久化 (TradeEvent, BalanceEvent) | trade_queue, balance_event_queue | ledger files |

## 队列设计

使用 `crossbeam-queue::ArrayQueue` 实现无锁 MPSC 队列：

```rust
pub struct MultiThreadQueues {
    pub order_queue: Arc<ArrayQueue<OrderAction>>,     // 64K capacity
    pub action_queue: Arc<ArrayQueue<ValidAction>>,    // 64K capacity
    pub trade_queue: Arc<ArrayQueue<TradeEvent>>,      // 64K capacity
    pub balance_update_queue: Arc<ArrayQueue<BalanceUpdateRequest>>,  // 64K
    pub balance_event_queue: Arc<ArrayQueue<BalanceEvent>>,           // 64K
}
```

## Cancel 订单处理

Cancel 订单流程：

1. **Ingestion**: 创建 `OrderAction::Cancel { order_id, user_id }`
2. **UBSCore**: 直接传递到 `action_queue`（无需 balance lock）
3. **ME**: 从 OrderBook 移除订单，发送 `BalanceUpdateRequest::Cancel`
4. **UBSCore** (Post-Trade): 处理 unlock，生成 `BalanceEvent::Unlock`
5. **Settlement**: 持久化 `BalanceEvent`

## 一致性验证

### 测试脚本

```bash
# 运行完整对比测试
./scripts/test_pipeline_compare.sh highbal

# 支持的数据集:
#   100k    - 100k orders without cancel
#   cancel  - 1.3M orders with 30% cancel
#   highbal - 1.3M orders with 30% cancel, high balance (推荐)
```

### 验证结果 (1.3M orders, 30% cancel, high balance)

```
╔════════════════════════════════════════════════════════════════╗
║                    ✅ ALL TESTS PASSED                         ║
║  Multi-thread pipeline matches single-thread exactly!          ║
╚════════════════════════════════════════════════════════════════╝
```

### 关键指标

| 数据集 | 总订单 | Place | Cancel | Trades | 结果 |
|--------|--------|-------|--------|--------|------|
| 100k (无 cancel) | 100,000 | 100,000 | 0 | 47,886 | ✅ 完全一致 |
| 1.3M + 30% cancel (高余额) | 1,300,000 | 1,000,000 | 300,000 | 667,567 | ✅ 完全一致 |

## 注意事项

### 余额充足性

如果测试数据中用户余额不足，可能导致部分订单被 reject。在并发环境中，由于 settle 时序不同，这些 reject 可能与单线程结果不同。

**解决方案**: 使用 `highbal` 数据集，确保每个用户有充足余额（1000 BTC + 100M USDT）。

### Shutdown 同步

Multi-thread pipeline 在 shutdown 时需要确保所有队列都已 drain：

```rust
while !queues.all_empty() {
    std::hint::spin_loop();
}
shutdown.request_shutdown();
```

## 性能

| 模式 | 100k orders | 1.3M orders |
|------|-------------|-------------|
| Single-Thread | 350ms | 15.5s |
| Multi-Thread | 330ms | 15.6s |

注：Multi-thread 当前版本包含 BalanceEvent 生成和持久化开销，性能与 Single-Thread 相当。未来优化方向包括批量 I/O 和减少队列竞争。

## 队列优先级策略 (未来)

**当前实现**:
完全优先 drain `balance_update_queue`，然后才处理新订单。

**未来优化: 加权轮询 (Weighted Round-Robin)**:
允许交替处理，提高响应性。

```rust
const SETTLE_WEIGHT: u32 = 3;  // settle : order = 3 : 1
```

## 文件结构

```
src/
├── pipeline.rs       # 共享类型: PipelineStats, MultiThreadQueues, ShutdownSignal
├── pipeline_mt.rs    # Multi-thread 实现: run_pipeline_multi_thread()
├── pipeline_runner.rs # Single-thread 实现: run_pipeline()
└── main.rs           # --pipeline / --pipeline-mt 模式选择
```
