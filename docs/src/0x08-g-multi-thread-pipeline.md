# 0x08-g 多线程 Pipeline 设计

> **目标**：将单线程 Pipeline 扩展为多线程，提高吞吐量

---

## 1. 当前状态

### 1.1 单线程 Pipeline (0x08-f)

```
┌──────────────────────────────────────────────────────────────┐
│                   Single Thread Pipeline                      │
├──────────────────────────────────────────────────────────────┤
│                                                               │
│   for order in orders:                                        │
│       1. UBSCore.process_order()     # Pre-Trade (WAL+Lock)  │
│       2. MatchingEngine.process()    # 撮合                   │
│       3. UBSCore.settle_trade()      # 结算                   │
│       4. Ledger.write()              # 账本                   │
│                                                               │
└──────────────────────────────────────────────────────────────┘
```

**性能数据** (1.3M orders, 含 300k cancel):

| 阶段 | 耗时 | 占比 |
|------|------|------|
| Pre-Trade | 678ms | 4.2% |
| **Matching** | **15.28s** | **93.5%** |
| Settlement | 26ms | 0.2% |
| Event Log | 353ms | 2.2% |

**结论**：Matching Engine 是绝对瓶颈，占 93% 时间。

---

## 2. 多线程 Pipeline 架构

### 2.1 目标架构

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
│                                             │                             │
│                                    valid_order_queue                      │
│                                             │                             │
│                                             ▼                             │
│                                    ┌─────────────────┐                   │
│   Thread 3: Matching Engine        │ Match orders    │                   │
│   ┌─────────────────┐              │ Generate trades │                   │
│   │ OrderBook       │◀─────────────│                 │                   │
│   └─────────────────┘              └────────┬────────┘                   │
│                                             │                             │
│                                    trade_queue (结算请求)                  │
│                                             │                             │
│                                             ▼                             │
│   Thread 4: Ledger Writer          settle_request_queue                  │
│   ┌─────────────────┐              ┌─────────────────┐                   │
│   │ Write events    │◀─────────────│ (回到 Thread 2) │                   │
│   └─────────────────┘  event_queue └─────────────────┘                   │
│                                                                           │
└──────────────────────────────────────────────────────────────────────────┘
```

### 2.2 线程职责

| 线程 | 职责 | 状态管理 |
|------|------|----------|
| **Thread 1: Ingestion** | 读取订单，推入 order_queue | 无状态 |
| **Thread 2: UBSCore** | Pre-Trade (Lock) + Settlement (Settle) | 拥有 UBSCore |
| **Thread 3: ME** | 订单撮合 | 拥有 OrderBook |
| **Thread 4: Ledger** | 写入事件日志 | 拥有 LedgerWriter |

---

## 3. 关键设计决策

### 3.1 为什么 Pre-Trade 和 Settlement 在同一线程？

**问题**：UBSCore 包含所有用户的余额状态

```rust
pub struct UBSCore {
    accounts: FxHashMap<UserId, UserAccount>,  // 共享可变状态！
    wal: WalWriter,
}
```

**选项对比**：

| 方案 | 优点 | 缺点 |
|------|------|------|
| A. 同一线程 | 无锁，简单 | Pre-Trade 和 Settlement 串行 |
| B. Mutex 保护 | 可并行 | 锁竞争，可能更慢 |
| C. 分片锁 (per-user) | 高并行度 | 复杂，跨用户交易需要多锁 |

**选择方案 A**：
- UBSCore 操作（Pre-Trade + Settlement）只占 4.4% 时间
- 引入锁可能带来的开销 > 并行收益
- 保持代码简单

### 3.2 ME 为什么独占线程？

**问题**：OrderBook 是有状态的，撮合需要读写

```rust
pub struct OrderBook {
    bids: BTreeMap<Price, VecDeque<InternalOrder>>,  // 可变状态
    asks: BTreeMap<Price, VecDeque<InternalOrder>>,  // 可变状态
    order_index: FxHashMap<OrderId, (Price, Side)>,
}
```

**解决**：ME 独占 OrderBook，通过 queue 接收订单

### 3.3 Ledger 为什么独占线程？

**问题**：文件写入需要同步

**解决**：所有线程通过 `event_queue` 发送事件，Ledger 线程负责写入

---

## 4. 新增 Queue 设计

### 4.1 Queue 列表

```rust
/// 原有队列
pub order_queue: Arc<ArrayQueue<SequencedOrder>>,       // Ingestion → UBSCore
pub valid_order_queue: Arc<ArrayQueue<ValidOrder>>,     // UBSCore → ME
pub trade_queue: Arc<ArrayQueue<TradeEvent>>,           // ME → UBSCore (Settlement)

/// 新增队列
pub settle_request_queue: Arc<ArrayQueue<SettleRequest>>,  // ME → UBSCore (结算请求)
pub event_queue: Arc<ArrayQueue<PipelineEvent>>,           // All → Ledger (事件写入)
```

### 4.2 新增消息类型

```rust
/// 结算请求（ME → UBSCore）
pub struct SettleRequest {
    pub trade_event: TradeEvent,
    pub price_improvement_refund: Option<(UserId, AssetId, u64)>,
}

/// Pipeline 事件（All → Ledger）
pub enum PipelineEvent {
    OrderAccepted { seq_id: u64, order_id: u64, user_id: u64 },
    OrderRejected { order_id: u64, reason: RejectReason },
    BalanceLocked { user_id: u64, asset_id: AssetId, amount: u64, ... },
    TradeSettled { trade_id: u64, ... },
    // ...
}
```

---

## 5. 线程间通信流程

```
                         order_queue
    [Ingestion] ─────────────────────────▶ [UBSCore Thread]
                                                │
                                                │ valid_order_queue (accepted)
                                                │ event_queue (Lock/Reject events)
                                                ▼
                        trade_queue       ┌──────────────┐
         ◀────────────────────────────────│  ME Thread   │
                                          └──────────────┘
                                                │
                                                │ event_queue (Trade events)
                                                ▼
                settle_request_queue      ┌──────────────┐
    [UBSCore] ◀───────────────────────────│  (回流)      │
                                          └──────────────┘
                                                │
                                                │ event_queue (Settle events)
                                                ▼
                                          ┌──────────────┐
                                          │Ledger Thread │
                                          └──────────────┘
```

---

## 6. 实现步骤

### Phase 4a: 重构准备

1. **定义新的消息类型**
   - `SettleRequest` - 结算请求
   - `PipelineEvent` - 统一的事件类型

2. **扩展 PipelineQueues**
   - 添加 `settle_request_queue`
   - 添加 `event_queue`

3. **重构 Ledger 接口**
   - 接收 `PipelineEvent` 而非直接写入

### Phase 4b: 实现多线程 Runner

```rust
pub fn run_pipeline_multi_thread(
    orders: Vec<InputOrder>,
    ubscore: UBSCore,
    book: OrderBook,
    ledger: LedgerWriter,
    symbol_mgr: SymbolManager,
    active_symbol_id: u32,
) -> PipelineResult {
    let queues = MultiThreadPipelineQueues::new();
    let shutdown = Arc::new(ShutdownSignal::new());
    
    // Thread 1: Ingestion
    let t1 = spawn_ingestion_thread(...);
    
    // Thread 2: UBSCore (Pre-Trade + Settlement)
    let t2 = spawn_ubscore_thread(...);
    
    // Thread 3: Matching Engine
    let t3 = spawn_me_thread(...);
    
    // Thread 4: Ledger Writer
    let t4 = spawn_ledger_thread(...);
    
    // Wait for completion
    t1.join()?;
    shutdown.request();
    t2.join()?;
    t3.join()?;
    t4.join()?;
    
    // Collect stats
}
```

### Phase 4c: 验证正确性

1. 运行 100k orders 对比单线程输出
2. 运行 1.3M orders 对比
3. MD5 校验余额和账本

### Phase 4d: 性能测试

1. 对比单线程 vs 多线程吞吐量
2. 分析瓶颈是否转移
3. 调优 queue 大小

---

## 7. 预期收益分析

### 7.1 理论上限

当前瓶颈分布：

| 阶段 | 占比 | 可否并行 |
|------|------|----------|
| Pre-Trade | 4.2% | ❌ (与 Settlement 共享状态) |
| **Matching** | **93.5%** | ❌ (OrderBook 有状态) |
| Settlement | 0.2% | ❌ (与 Pre-Trade 共享状态) |
| Event Log | 2.2% | ✅ (可独立线程) |

**Amdahl's Law**：

```
可并行部分 = 2.2% (Event Log)
不可并行部分 = 97.8%

最大加速比 = 1 / (0.978 + 0.022/N) ≈ 1.02x  (N→∞)
```

**结论**：由于 Matching 占 93.5%，且无法并行，多线程提升非常有限。

### 7.2 实际价值

虽然吞吐量提升有限，多线程架构仍有价值：

| 价值 | 说明 |
|------|------|
| **延迟优化** | Ledger 异步写入减少关键路径 |
| **代码解耦** | 各服务独立，便于维护 |
| **可扩展性** | 未来可引入分片 ME |
| **容错性** | 独立线程可独立恢复 |

---

## 8. 风险和注意事项

### 8.1 死锁风险

**场景**：所有 queue 都满

**缓解**：
- 合理设置 queue 容量
- 实现背压机制
- 添加超时检测

### 8.2 顺序保证

**问题**：多线程可能打乱事件顺序

**解决**：
- 每个事件带 `seq_id`
- Ledger 线程按 `seq_id` 排序（可选）
- 或接受无序写入，验证时按 seq 排序

### 8.3 Graceful Shutdown

**步骤**：
1. Ingestion 停止推送
2. 等待所有 queue 排空
3. 发送 shutdown 信号
4. 等待所有线程退出

---

## 9. 决策：是否实施？

### 9.1 收益 vs 成本

| 维度 | 评估 |
|------|------|
| 吞吐量提升 | ~2% (Amdahl's Law 限制) |
| 延迟优化 | 有 (Event Log 异步化) |
| 开发成本 | 中等 (2-4 小时) |
| 维护成本 | 增加 (多线程调试困难) |
| 学习价值 | 高 (掌握 Rust 多线程模式) |

### 9.2 建议

**推荐实施**，原因：
1. 教学价值高 - 完整的 Pipeline 架构示范
2. 为未来优化做准备 - 分片 ME、多交易对等
3. 代码解耦 - 更清晰的服务边界

---

## 下一步

1. [ ] 定义 `SettleRequest` 和 `PipelineEvent`
2. [ ] 扩展 `PipelineQueues`
3. [ ] 实现 `run_pipeline_multi_thread()`
4. [ ] 验证正确性
5. [ ] 性能测试
