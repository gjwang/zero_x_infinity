# 0x08-f Ring Buffer Pipeline 实现

> **📦 代码变更**: [查看 Diff](https://github.com/gjwang/zero_x_infinity/compare/v0.8-e-perf-bottleneck-profiling...v0.8-f-ring-buffer-pipeline)

> **目标**：使用 Ring Buffer 串接不同服务，实现真正的 Pipeline 架构

---

## 目录

- [Part 1: 单线程 Pipeline](#part-1-单线程-pipeline)
- [Part 2: 多线程 Pipeline](#part-2-多线程-pipeline)
- [验证与性能](#验证与性能)

---

# Part 1: 单线程 Pipeline

## 1.1 背景

### 已有组件

| 组件 | 文件 | 状态 |
|------|------|------|
| UBSCore | `src/ubscore.rs` | ✅ 实现 |
| WAL | `src/wal.rs` | ✅ 实现 |
| Messages | `src/messages.rs` | ✅ 实现 |
| OrderBook | `src/orderbook.rs` | ✅ 实现 |
| Engine | `src/engine.rs` | ✅ 实现 |
| crossbeam-queue | Cargo.toml | ✅ 依赖 |

### 原始执行模式 (同步串行)

```
for order in orders:
    1. ubscore.process_order(order)     # WAL + Lock
    2. engine.process_order(order)       # Match
    3. ubscore.settle_trade(trade)       # Settle
    4. ledger.write(event)               # Persist
```

**问题**：没有 Pipeline 并行，延迟累加

## 1.2 单线程 Pipeline 架构

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

### 核心数据结构

```rust
/// Pipeline 的 Ring Buffer 容量配置
pub const ORDER_QUEUE_CAPACITY: usize = 4096;
pub const VALID_ORDER_QUEUE_CAPACITY: usize = 4096;
pub const TRADE_QUEUE_CAPACITY: usize = 16384;

/// Pipeline 共享的 Ring Buffers
pub struct PipelineQueues {
    pub order_queue: Arc<ArrayQueue<SequencedOrder>>,
    pub valid_order_queue: Arc<ArrayQueue<ValidOrder>>,
    pub trade_queue: Arc<ArrayQueue<TradeEvent>>,
}

/// Pipeline 统计
pub struct PipelineStats {
    pub orders_ingested: AtomicU64,
    pub orders_accepted: AtomicU64,
    pub orders_rejected: AtomicU64,
    pub trades_generated: AtomicU64,
    pub trades_settled: AtomicU64,
    pub backpressure_events: AtomicU64,
}
```

### 执行流程

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
    
    // 2. Process loop (single thread, round-robin)
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

## 1.3 运行命令

```bash
# 单线程 Pipeline
cargo run --release -- --pipeline
```

---

# Part 2: 多线程 Pipeline

## 2.1 架构

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

队列说明:
① order_queue:           Ingestion → UBSCore           SequencedOrder
③ valid_order_queue:     UBSCore   → ME                ValidOrder
④ trade_queue:           ME        → Settlement        TradeEvent
⑤ balance_update_queue:  ME        → UBSCore           BalanceUpdateRequest
⑥ balance_event_queue:   UBSCore   → Settlement        BalanceEvent
```

## 2.2 关键设计点

1. **ME Fan-out**: ME 将 `TradeEvent` **并行**发送到：
   - `trade_queue` → Settlement (持久化交易记录)
   - `balance_update_queue` → UBSCore (余额结算)

2. **UBSCore 是余额操作的唯一入口**:
   - **Pre-Trade**: `process_order()` - 验证订单、锁定余额 → 生成 `Lock` 事件
   - **Post-Trade**: `settle_trade()` - 结算成交 → 生成 `SpendFrozen` + `Credit` 事件
   - **Cancel/Reject**: `unlock()` - 解锁余额 → 生成 `Unlock` 事件
   - **Deposit/Withdraw**: 充值提现 → 生成 `Deposit`/`Withdraw` 事件

3. **Settlement 接收两个队列**:
   - `trade_queue`: 交易事件 (来自 ME)
   - `balance_event_queue`: **所有**余额变更事件 (来自 UBSCore)

4. **BalanceEvent 是完整的审计日志**:
   - 每一笔余额变更都生成 BalanceEvent
   - Settlement 持久化到 DB/Ledger
   - 支持完整的余额重建和审计

## 2.3 数据类型

### BalanceUpdateRequest (ME → UBSCore)

```rust
#[derive(Clone)]
pub struct BalanceUpdateRequest {
    pub trade_event: TradeEvent,
    pub price_improvement: Option<PriceImprovement>,
}
```

### BalanceEvent (UBSCore → Settlement)

```rust
/// 余额变更事件 (UBSCore → Settlement)
/// 
/// 重要：这是 **所有** 余额变更事件的通道，包括但不限于：
/// - Deposit/Withdraw (充值/提现) - 待实现
/// - Pre-Trade Lock (下单锁定) - ✅ 已实现
/// - Post-Trade Settle (成交结算: spend_frozen + credit) - ✅ 已实现
/// - Cancel/Reject Unlock (取消/拒绝解锁) - 待实现
/// - Price Improvement RefundFrozen (价格改善退款) - ✅ 已实现
#[derive(Debug, Clone)]
pub struct BalanceEvent {
    pub user_id: u64,
    pub asset_id: u32,
    pub event_type: BalanceEventType,
    pub amount: u64,
    pub order_id: Option<u64>,      // 关联订单 (如有)
    pub trade_id: Option<u64>,      // 关联成交 (如有)
    pub version: u64,               // 余额版本号 (用于审计)
    pub avail_after: u64,           // 操作后可用余额
    pub frozen_after: u64,          // 操作后冻结余额
    pub timestamp_ns: u64,          // 时间戳 (纳秒)
    // TODO: pub ref_id: Option<String>,  // 外部参考ID (充值/提现时使用)
}

/// 余额事件类型 - 覆盖所有余额变更场景
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BalanceEventType {
    // === External Operations (待实现) ===
    Deposit,        // 充值: avail += amount
    Withdraw,       // 提现: avail -= amount
    
    // === Pre-Trade (Lock) ===
    Lock,           // 下单锁定: avail -= amount, frozen += amount
    
    // === Post-Trade (Settle) ===
    SpendFrozen,    // 成交扣减冻结: frozen -= amount
    Credit,         // 成交入账: avail += amount
    
    // === Cancel/Reject (待实现) ===
    Unlock,         // 取消/拒绝解锁: frozen -= amount, avail += amount
    
    // === Price Improvement ===
    RefundFrozen,   // 价格改善退款: frozen -= amount, avail += amount
}
```

### MultiThreadQueues

```rust
/// 多线程队列 (完整版)
pub struct MultiThreadQueues {
    // Pre-Trade Flow
    pub order_queue: Arc<ArrayQueue<SequencedOrder>>,
    pub valid_order_queue: Arc<ArrayQueue<ValidOrder>>,
    
    // ME → Settlement (Trade Events)
    pub trade_queue: Arc<ArrayQueue<TradeEvent>>,
    
    // ME → UBSCore (Balance Update Requests)
    pub balance_update_queue: Arc<ArrayQueue<BalanceUpdateRequest>>,
    
    // UBSCore → Settlement (Balance Events)
    pub balance_event_queue: Arc<ArrayQueue<BalanceEvent>>,
}
```

## 2.4 实现状态

| 组件 | 状态 |
|------|------|
| `order_queue` | ✅ 已实现 |
| `valid_order_queue` | ✅ 已实现 |
| `trade_queue` | ✅ 已实现 |
| `balance_update_queue` | ✅ 已实现 |
| `balance_event_queue` | ✅ 已实现 |
| UBSCore 生成 BalanceEvent | ✅ 已实现 (Lock, SpendFrozen, Credit, RefundFrozen) |
| Settlement 消费 BalanceEvent | ✅ 已实现 (计数统计，持久化待补充) |

### BalanceEvent 类型实现状态

| 事件类型 | 触发场景 | 状态 |
|----------|----------|------|
| `Lock` | Pre-Trade 下单锁定 | ✅ 已实现 |
| `SpendFrozen` | Post-Trade 扣减冻结 | ✅ 已实现 |
| `Credit` | Post-Trade 入账 | ✅ 已实现 |
| `RefundFrozen` | Price Improvement 退款 | ✅ 已实现 |
| `Unlock` | Cancel/Reject 解锁 | ⏳ 待实现 |
| `Deposit` | 外部充值 | ⏳ 待实现 |
| `Withdraw` | 外部提现 | ⏳ 待实现 |

## 2.5 运行命令

```bash
# 多线程 Pipeline
cargo run --release -- --pipeline-mt

# UBSCore 模式 (baseline)
cargo run --release -- --ubscore
```

---

# 验证与性能

## 正确性验证

```bash
# 运行 E2E 测试
./scripts/test_e2e.sh
```

| 数据集 | Pipeline vs UBSCore | 结果 |
|--------|---------------------|------|
| 100k orders | MD5 match | ✅ |
| 1.3M orders (含 30 万 cancel) | MD5 match | ✅ |

## 性能对比 (2025-12-17)

### 1.3M 订单数据集 (含 30 万 cancel)

| 模式 | 执行时间 | 吞吐量 | Trades |
|------|----------|--------|--------|
| UBSCore | 23.5s | 55k ops/s | 538,487 |
| Single-Thread Pipeline | 22.1s | 59k ops/s | 538,487 |
| Multi-Thread Pipeline | 29.1s | 45k ops/s | 489,804 |

**观察**:
- 多线程模式跳过 cancel 订单（30 万），Trades 数量不一致
- 多线程模式反而比单线程慢 ~30%
- **待调查**: 原因待分析

### 100k 订单数据集 (纯新订单，无 cancel)

| 模式 | 执行时间 | 吞吐量 | vs UBSCore |
|------|----------|--------|------------|
| UBSCore | 755ms | 132k ops/s | baseline |
| Single-Thread Pipeline | 519ms | 193k ops/s | **+46%** |
| **Multi-Thread Pipeline** | **391ms** | **256k ops/s** | **+93%** |

**观察**:
- 100k 小数据集上多线程表现最佳
- 1.3M 大数据集上多线程反而退化

### 已知不一致

| 差异项 | 单线程 Pipeline | 多线程 Pipeline |
|--------|-----------------|-----------------|
| Cancel 订单处理 | ✅ 处理 | ❌ 跳过 |
| Trades 数量 (1.3M) | 538,487 | 489,804 |
| BalanceEvent 队列 | ❌ 不使用 (本地生成) | ✅ 使用 `balance_event_queue` |
| BalanceEvent 类型 | `messages::BalanceEvent` | `pipeline::BalanceEvent` |

**待办**:
1. 多线程实现 cancel 订单处理 (生成 `Unlock` 事件)
2. Trades 数量一致后重新对比性能
3. 分析 1.3M 数据集上多线程变慢的根本原因

---

## 关键设计决策

### Backpressure 策略

| 策略 | 描述 | 适用场景 |
|------|------|----------|
| Spin Wait | 忙等待 (`spin_loop()`) | 低延迟 |
| Yield | `std::thread::yield_now()` | 中等 |
| Block | Condvar 阻塞 | 省 CPU |

**选择 Spin Wait**：HFT 场景优先低延迟

### Shutdown 机制

使用 `ShutdownSignal` 原子标记优雅关闭：
1. Stop accepting new orders
2. Drain all queues
3. Flush WAL
4. Report final stats

### 错误处理

- Pre-Trade 失败 → 记录 Rejected Event
- Matching 保证成功（余额已锁定）
- Settlement 必须成功（无限重试直到成功）

---

## 文件变更

| 文件 | 说明 |
|------|------|
| `src/pipeline.rs` | Ring Buffer 队列、BalanceEvent 类型定义 |
| `src/pipeline_runner.rs` | 单线程 Pipeline Runner |
| `src/pipeline_mt.rs` | 多线程 Pipeline 实现 |
| `src/lib.rs` | 导出模块 |
| `src/main.rs` | `--pipeline`, `--pipeline-mt` 模式 |
