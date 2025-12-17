# 0x08-f Ring Buffer Pipeline 实现

> **目标**：使用 Ring Buffer 串接不同服务，实现真正的 Pipeline 架构

---

## 1. 当前状态

### 1.1 已有组件

| 组件 | 文件 | 状态 |
|------|------|------|
| UBSCore | `src/ubscore.rs` | ✅ 实现 |
| WAL | `src/wal.rs` | ✅ 实现 |
| Messages | `src/messages.rs` | ✅ 实现 |
| OrderBook | `src/orderbook.rs` | ✅ 实现 |
| Engine | `src/engine.rs` | ✅ 实现 |
| crossbeam-queue | Cargo.toml | ✅ 依赖 |

### 1.2 当前执行模式

目前 `execute_orders_with_ubscore()` 是**同步串行**执行：

```
for order in orders:
    1. ubscore.process_order(order)     # WAL + Lock
    2. engine.process_order(order)       # Match
    3. ubscore.settle_trade(trade)       # Settle
    4. ledger.write(event)               # Persist
```

**问题**：没有 Pipeline 并行，延迟累加

---

## 2. Pipeline 架构目标

```
┌─────────────────────────────────────────────────────────────────────────┐
│                         Ring Buffer Pipeline                             │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│   Thread 1: Order Ingestion                                              │
│   ┌──────────────┐                                                       │
│   │   Input      │ ─────────────────────────────────────┐                │
│   │   Orders     │                                      │                │
│   └──────────────┘                                      ▼                │
│                                                 ┌──────────────┐         │
│                                                 │ order_queue  │         │
│                                                 │ ArrayQueue   │         │
│                                                 └──────┬───────┘         │
│                                                        │                 │
│   Thread 2: UBSCore (Pre-Trade)                        ▼                 │
│   ┌──────────────────────────────────────────────────────────┐          │
│   │  UBSCore.process_order():                                │          │
│   │    1. Write Order WAL                                    │          │
│   │    2. Lock Balance                                       │──┐       │
│   │    3. If OK → Push to valid_order_queue                  │  │       │
│   │       If Fail → Mark Rejected                            │  │       │
│   └──────────────────────────────────────────────────────────┘  │       │
│                                                                  │       │
│                                                 ┌────────────────┘       │
│                                                 ▼                        │
│                                         ┌──────────────┐                 │
│                                         │valid_order_q │                 │
│                                         │ ArrayQueue   │                 │
│                                         └──────┬───────┘                 │
│                                                │                         │
│   Thread 3: Matching Engine                    ▼                         │
│   ┌──────────────────────────────────────────────────────────┐          │
│   │  ME.process_order():                                     │          │
│   │    1. Match against OrderBook                            │──┐       │
│   │    2. Generate TradeEvents                               │  │       │
│   │    3. Push to trade_queue                                │  │       │
│   └──────────────────────────────────────────────────────────┘  │       │
│                                                                  │       │
│                                                 ┌────────────────┘       │
│                                                 ▼                        │
│                                         ┌──────────────┐                 │
│                                         │ trade_queue  │                 │
│                                         │ ArrayQueue   │                 │
│                                         └──────┬───────┘                 │
│                                                │                         │
│                              ┌─────────────────┴─────────────────┐       │
│                              │                                   │       │
│   Thread 4: Settlement       ▼                Thread 5: UBSCore  ▼       │
│   ┌──────────────────────────────┐         (Settle Balance)              │
│   │  Settlement:                 │         ┌──────────────────────────┐  │
│   │    1. Persist TradeEvents     │         │  UBSCore.settle_trade(): │  │
│   │    2. Write Ledger            │         │    1. spend_frozen       │  │
│   │                              │         │    2. deposit            │  │
│   └──────────────────────────────┘         └──────────────────────────┘  │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## 3. 实现步骤

### Phase 1: 定义 Ring Buffer 模块 ✅ 已完成

创建 `src/pipeline.rs`：

```rust
use crossbeam_queue::ArrayQueue;
use std::sync::Arc;

/// Pipeline 的 Ring Buffer 容量配置 (已实现)
pub const ORDER_QUEUE_CAPACITY: usize = 4096;
pub const VALID_ORDER_QUEUE_CAPACITY: usize = 4096;
pub const TRADE_QUEUE_CAPACITY: usize = 16384;  // 1 order may generate multiple trades

/// Pipeline 共享的 Ring Buffers (已实现)
pub struct PipelineQueues {
    pub order_queue: Arc<ArrayQueue<SequencedOrder>>,
    pub valid_order_queue: Arc<ArrayQueue<ValidOrder>>,
    pub trade_queue: Arc<ArrayQueue<TradeEvent>>,
}

/// Pipeline 统计 (已实现)
pub struct PipelineStats {
    pub orders_ingested: AtomicU64,
    pub orders_accepted: AtomicU64,
    pub orders_rejected: AtomicU64,
    pub trades_generated: AtomicU64,
    pub trades_settled: AtomicU64,
    pub backpressure_events: AtomicU64,
}

/// 单线程 Pipeline Runner (已实现)
pub struct SingleThreadPipeline {
    pub queues: PipelineQueues,
    pub stats: Arc<PipelineStats>,
    pub shutdown: Arc<ShutdownSignal>,
}
```

**已实现功能**：
- ✅ `PipelineQueues` - 三个 Ring Buffer
- ✅ `SequencedOrder` - 带序号的订单
- ✅ `PipelineStats` - 原子统计计数
- ✅ `ShutdownSignal` - 优雅关闭信号
- ✅ `SingleThreadPipeline` - 单线程 Pipeline Runner
- ✅ `push_with_backpressure()` - 带背压的推送
- ✅ 10 个单元测试

### Phase 2: 定义 Worker Traits

```rust
/// Pre-Trade Worker (UBSCore)
pub trait PreTradeWorker {
    fn process(&mut self, order: InputOrder) -> Result<ValidOrder, OrderEvent>;
}

/// Matching Worker (ME)  
pub trait MatchingWorker {
    fn process(&mut self, order: ValidOrder) -> Vec<TradeEvent>;
}

/// Settlement Worker
pub trait SettlementWorker {
    fn process(&mut self, trade: &TradeEvent);
}

/// Balance Settlement Worker (UBSCore)
pub trait BalanceSettleWorker {
    fn settle(&mut self, trade: &TradeEvent) -> Result<Vec<BalanceEvent>, Error>;
}
```

### Phase 3: 单线程 Pipeline (验证正确性)

先实现单线程版本，确保 Ring Buffer 通信正确：

```rust
pub fn run_pipeline_single_thread(
    orders: Vec<InputOrder>,
    ubscore: &mut UBSCore,
    engine: &mut Engine,
    ledger: &mut LedgerWriter,
) -> PipelineStats {
    let queues = PipelineQueues::new();
    
    // 1. Push all orders to queue
    for order in orders {
        queues.order_queue.push(order).unwrap();
    }
    
    // 2. Process loop (single thread)
    loop {
        // UBSCore: order_queue → valid_order_queue
        if let Some(order) = queues.order_queue.pop() {
            match ubscore.process_order(order) {
                Ok(valid) => queues.valid_order_queue.push(valid).unwrap(),
                Err(rejected) => { /* log */ }
            }
        }
        
        // ME: valid_order_queue → trade_queue
        if let Some(valid_order) = queues.valid_order_queue.pop() {
            let trades = engine.process_order(valid_order);
            for trade in trades {
                queues.trade_queue.push(trade).unwrap();
            }
        }
        
        // Settlement: trade_queue → persist
        if let Some(trade) = queues.trade_queue.pop() {
            ubscore.settle_trade(&trade);
            ledger.write(&trade);
        }
        
        // Exit condition
        if all_queues_empty() && all_orders_processed() {
            break;
        }
    }
}
```

### Phase 4: 多线程 Pipeline

使用 `std::thread` 或 Rayon 并行化：

```rust
pub fn run_pipeline_multi_thread(
    orders: Vec<InputOrder>,
    queues: PipelineQueues,
) {
    let (shutdown_tx, shutdown_rx) = crossbeam_channel::bounded(1);
    
    // Thread 1: Order Ingestion
    let order_q = queues.order_queue.clone();
    let ingestion = std::thread::spawn(move || {
        for order in orders {
            while order_q.push(order.clone()).is_err() {
                std::hint::spin_loop();  // Backpressure
            }
        }
    });
    
    // Thread 2: UBSCore Pre-Trade
    let ubscore_thread = std::thread::spawn(move || {
        loop {
            if let Some(order) = order_queue.pop() {
                // process...
            }
        }
    });
    
    // Thread 3: Matching Engine
    // Thread 4: Settlement
    // ...
    
    // Wait for completion
    ingestion.join().unwrap();
    // Graceful shutdown...
}
```

---

## 4. 关键设计决策

### 4.1 Backpressure 策略

当 Ring Buffer 满时：

| 策略 | 描述 | 适用场景 |
|------|------|----------|
| Spin Wait | 忙等待 (`spin_loop()`) | 低延迟 |
| Yield | `std::thread::yield_now()` | 中等 |
| Block | Condvar 阻塞 | 省 CPU |

**选择 Spin Wait**：HFT 场景优先低延迟

### 4.2 Shutdown 机制

使用 Poison Pill 模式：

```rust
enum PipelineMessage<T> {
    Data(T),
    Shutdown,
}
```

### 4.3 错误处理

- Pre-Trade 失败 → 记录 Rejected Event
- Matching 保证成功（余额已锁定）
- Settlement 必须成功（重试 or panic）

---

## 5. 性能对比目标

| 指标 | 串行模式 | Pipeline 模式 | 目标 |
|------|----------|---------------|------|
| 延迟 | 15µs/order | <10µs/order | -30% |
| 吞吐量 | 72k ops/s | >100k ops/s | +40% |
| CPU 利用率 | 单核 | 多核 | ↑ |

---

## 6. 待解决问题

### 6.1 Pipeline 确定性

已通过 **分离 Version 空间** 解决（见 0x08-c）

### 6.2 Graceful Shutdown

需要实现：
1. Stop accepting new orders
2. Drain all queues
3. Flush WAL
4. Report final stats

### 6.3 Backpressure Monitoring

添加 metrics：
- Queue depth
- Push failures
- Drain time

---

## 7. 验证计划

### 7.1 正确性验证

```bash
# 运行 Pipeline 模式
cargo run --release -- --pipeline

# 比较 baseline
python3 scripts/verify_baseline_equivalence.py
python3 scripts/verify_balance_events.py
```

### 7.2 性能验证

```bash
# 对比测试
cargo run --release              # 串行
cargo run --release -- --ubscore # UBSCore 串行
cargo run --release -- --pipeline # Pipeline 并行
```

---

## 进度追踪

| Phase | 任务 | 状态 |
|-------|------|------|
| 1 | 创建 `src/pipeline.rs` | ✅ |
| 1 | 实现 `PipelineQueues` | ✅ |
| 1 | 实现 `SequencedOrder` | ✅ |
| 1 | 实现 `PipelineStats` | ✅ |
| 1 | 实现 `SingleThreadPipeline` | ✅ |
| 2 | 定义 Worker Traits | ✅ (直接在 runner 中实现) |
| 3 | 实现单线程 Pipeline 完整流程 | ✅ |
| 3 | 验证正确性（baseline 对比） | ✅ |
| **4** | **实现多线程 Pipeline** | **✅** |
| 4 | 性能测试 | ✅ |

---

## 多线程 Pipeline (Phase 4)

### 架构

```
┌──────────────────────────────────────────────────────────────────────────┐
│                        Multi-Thread Pipeline                              │
├──────────────────────────────────────────────────────────────────────────┤
│                                                                           │
│   Thread 1: Ingestion              Thread 2: UBSCore (Pre-Trade + Settle)│
│   ┌─────────────────┐              ┌─────────────────┐                   │
│   │ Read orders     │              │ Lock balance    │                   │
│   │ Push to queue   │ ──────────▶  │ WAL write       │                   │
│   └─────────────────┘  order_queue │ Settle trades   │                   │
│                                    └────────┬────────┘                   │
│                                             │ valid_order_queue          │
│                                             ▼                             │
│   Thread 3: Matching Engine        ┌─────────────────┐                   │
│   ┌─────────────────┐              │ Match orders    │                   │
│   │ OrderBook       │◀─────────────│ Generate trades │                   │
│   └─────────────────┘              └────────┬────────┘                   │
│                                             │ settle_request_queue       │
│                                             ▼                             │
│   Thread 4: Ledger Writer          event_queue (from all threads)        │
│   ┌─────────────────┐              ┌─────────────────┐                   │
│   │ Write events    │◀─────────────│ Centralized log │                   │
│   └─────────────────┘              └─────────────────┘                   │
│                                                                           │
└──────────────────────────────────────────────────────────────────────────┘
```

### 新增类型

```rust
/// 结算请求 (ME → UBSCore)
pub struct SettleRequest {
    pub trade_event: TradeEvent,
    pub price_improvement: Option<PriceImprovement>,
}

/// Pipeline 事件 (All → Ledger)
pub enum PipelineEvent {
    OrderAccepted { seq_id, order_id, user_id },
    OrderRejected { order_id, reason },
    BalanceLocked { user_id, asset_id, amount, ... },
    SettleSpend { ... },
    SettleReceive { ... },
    // ...
}

/// 多线程队列
pub struct MultiThreadQueues {
    pub order_queue: Arc<ArrayQueue<SequencedOrder>>,
    pub valid_order_queue: Arc<ArrayQueue<ValidOrder>>,
    pub trade_queue: Arc<ArrayQueue<TradeEvent>>,
    pub settle_request_queue: Arc<ArrayQueue<SettleRequest>>,
    pub event_queue: Arc<ArrayQueue<PipelineEvent>>,
}
```

### 运行命令

```bash
# 单线程 Pipeline
cargo run --release -- --pipeline

# 多线程 Pipeline
cargo run --release -- --pipeline-mt

# UBSCore 模式 (baseline)
cargo run --release -- --ubscore
```

---

## 验证结果 (2025-12-17)

### 正确性验证

| 数据集 | Pipeline vs UBSCore | 结果 |
|--------|---------------------|------|
| 100k orders | MD5 match | ✅ |
| 1.3M orders (含 30 万 cancel) | MD5 match | ✅ |

### 最终性能对比 (100k orders)

| 模式 | 执行时间 | 吞吐量 | vs UBSCore |
|------|----------|--------|------------|
| UBSCore | 586ms | 170k ops/s | baseline |
| Single-Thread Pipeline | 430ms | 232k ops/s | **+36%** |
| **Multi-Thread Pipeline** | **412ms** | **242k ops/s** | **+42%** |

### 分析

实际加速比超过 Amdahl's Law 预测的原因：
1. **Ledger 异步化** - 文件 I/O 不再阻塞关键路径
2. **CPU 流水线优化** - 多线程利用现代 CPU 的并行执行单元
3. **减少内存竞争** - 每个线程有独立的工作集

---

## 文件变更

| 文件 | 变更 |
|------|------|
| `src/pipeline.rs` | 添加 `SettleRequest`, `PipelineEvent`, `MultiThreadQueues` |
| `src/pipeline_runner.rs` | 单线程 Pipeline Runner |
| `src/pipeline_mt.rs` | **新增**：多线程 Pipeline 实现 |
| `src/lib.rs` | 导出新模块 |
| `src/main.rs` | 添加 `--pipeline`, `--pipeline-mt` 模式 |
