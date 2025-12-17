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

### Phase 1: 定义 Ring Buffer 模块

创建 `src/pipeline.rs`：

```rust
use crossbeam_queue::ArrayQueue;
use std::sync::Arc;

/// Pipeline 的 Ring Buffer 容量配置
pub const ORDER_QUEUE_CAPACITY: usize = 1024;
pub const VALID_ORDER_QUEUE_CAPACITY: usize = 1024;
pub const TRADE_QUEUE_CAPACITY: usize = 4096;  // 1 order may generate multiple trades

/// Pipeline 共享的 Ring Buffers
pub struct PipelineQueues {
    /// Input orders → UBSCore
    pub order_queue: Arc<ArrayQueue<InputOrder>>,
    
    /// UBSCore → ME (validated orders with locked balance)
    pub valid_order_queue: Arc<ArrayQueue<ValidOrder>>,
    
    /// ME → Settlement + UBSCore (trade events)
    pub trade_queue: Arc<ArrayQueue<TradeEvent>>,
}

impl PipelineQueues {
    pub fn new() -> Self {
        Self {
            order_queue: Arc::new(ArrayQueue::new(ORDER_QUEUE_CAPACITY)),
            valid_order_queue: Arc::new(ArrayQueue::new(VALID_ORDER_QUEUE_CAPACITY)),
            trade_queue: Arc::new(ArrayQueue::new(TRADE_QUEUE_CAPACITY)),
        }
    }
}
```

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

## 下一步

1. 创建 `src/pipeline.rs`
2. 实现 `PipelineQueues`
3. 实现单线程 Pipeline
4. 验证正确性
5. 实现多线程 Pipeline
6. 性能测试
