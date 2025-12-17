# 0x08-g 多线程 Pipeline 设计 (Multi-Thread Pipeline Design)

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
║        Pipeline Comparison Test                                ║
╠════════════════════════════════════════════════════════════════╣
║  Dataset: 1.3M orders with 30% cancel (high balance)
╚════════════════════════════════════════════════════════════════╝

════════════════════════════════════════════════════════════════
Metric            Single-Thread    Multi-Thread     Status
────────────────────────────────────────────────────────────────
Ingested               1300000         1300000   ✅ PASS
Place                  1000000         1000000   ✅ PASS
Cancel                  300000          300000   ✅ PASS
Accepted               1000000         1000000   ✅ PASS
Rejected                     0               0   ✅ PASS
Trades                  667567          667567   ✅ PASS
════════════════════════════════════════════════════════════════

Final balances: ✅ MATCH (0 differences)

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
// Wait for all processing queues to drain before signaling shutdown
loop {
    if queues.all_empty() {
        break;
    }
    std::hint::spin_loop();
}

// Now signal shutdown
shutdown.request_shutdown();
```

## 性能

| 模式 | 100k orders | 1.3M orders |
|------|-------------|-------------|
| Single-Thread | 350ms | 15.5s |
| Multi-Thread | 330ms | 15.6s |

注：Multi-thread 当前版本包含 BalanceEvent 生成和持久化开销，性能与 Single-Thread 相当。未来优化方向包括批量 I/O 和减少队列竞争。

## 队列优先级策略

### 当前实现

UBSCore 同时消费两个队列：
- `balance_update_queue` - 来自 ME 的 settle/unlock 请求
- `order_queue` - 新订单

**当前策略**: 完全 drain `balance_update_queue` 后才处理 `order_queue`。

```rust
// PRIORITY 1: 先处理所有 balance updates
while let Some(update) = balance_update_queue.pop() {
    process_settlement(update);
}

// PRIORITY 2: 再处理一个新订单
if let Some(order) = order_queue.pop() {
    process_order(order);
}
```

### 未来优化: 加权优先级

生产环境建议实现更灵活的优先级策略，允许交替处理但保持 settle 优先：

#### 方案 1: 加权轮询 (Weighted Round-Robin)

```rust
const SETTLE_WEIGHT: u32 = 3;  // settle : order = 3 : 1

let mut settle_count = 0;
loop {
    if settle_count < SETTLE_WEIGHT {
        if let Some(update) = balance_update_queue.pop() {
            process_settlement(update);
            settle_count += 1;
            continue;
        }
    }
    
    if let Some(order) = order_queue.pop() {
        process_order(order);
        settle_count = 0;  // 重置
    }
}
```

#### 方案 2: 批量处理 (Batch Processing)

```rust
const MAX_SETTLE_BATCH: usize = 10;
const MAX_ORDER_BATCH: usize = 3;

loop {
    // Phase 1: 最多 10 个 settlements
    for _ in 0..MAX_SETTLE_BATCH {
        if balance_update_queue.pop().map(process_settlement).is_none() {
            break;
        }
    }
    
    // Phase 2: 最多 3 个 orders
    for _ in 0..MAX_ORDER_BATCH {
        if order_queue.pop().map(process_order).is_none() {
            break;
        }
    }
}
```

#### 方案 3: 动态自适应 (Adaptive)

```rust
loop {
    let settle_depth = balance_update_queue.len();
    
    // 根据积压深度动态调整比例
    let ratio = match settle_depth {
        0..=10 => 2,    // 空闲: 2:1
        11..=100 => 5,  // 正常: 5:1
        _ => 10,        // 积压: 10:1
    };
    
    // 按比例处理...
}
```

### 配置结构 (未来)

```rust
pub struct QueuePriority {
    /// Settle:Order ratio (e.g., 3 = process 3 settlements per 1 order)
    pub settle_weight: u32,
    /// Max batch size per round
    pub max_batch: usize,
    /// Enable adaptive mode based on queue depth
    pub adaptive: bool,
}
```

## 文件结构

```
src/
├── pipeline.rs       # 共享类型: PipelineStats, MultiThreadQueues, ShutdownSignal
├── pipeline_mt.rs    # Multi-thread 实现: run_pipeline_multi_thread()
├── pipeline_runner.rs # Single-thread 实现: run_pipeline()
└── main.rs           # --pipeline / --pipeline-mt 模式选择

scripts/
├── test_pipeline_compare.sh        # 统一测试脚本
├── test_pipeline_baseline.sh       # 生成 baseline
├── test_pipeline_verify.sh         # 验证 multi-thread
└── generate_orders_with_cancel_highbal.py  # 生成高余额测试数据
```
