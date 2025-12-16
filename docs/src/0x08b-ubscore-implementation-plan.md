# 0x08b UBSCore Implementation Plan

> **分支**: `0x08b-ubscore-implementation`
> **创建时间**: 2025-12-16

---

## 目标 (Goals)

基于 0x08a 的架构设计，实现 UBSCore (User Balance Core) 服务及相关组件。

---

## 阶段划分 (Phases)

### Phase 1: Ring Buffer - 使用 `crossbeam-queue` 库 (安全第一)

**依赖**: `crossbeam-queue = "0.3"` (已添加到 Cargo.toml)

**目的**: 服务间无锁通信的基础设施

#### 🛡️ 为什么选择 `crossbeam-queue`? (Safety First)

| 维度 | crossbeam-queue | 其他选项 |
|------|-----------------|----------|
| **成熟度** | 🌟🌟🌟🌟🌟 (330万+ 累计下载) | 较少 |
| **安全审计** | 最严苛 (Loom 形式化验证) | 一般 |
| **维护团队** | Rust 核心团队成员参与 | 社区 |
| **生产依赖** | tokio, actix, rayon | - |
| **API 风险** | 极低 (误用很难编译通过) | 中等 |

> **金融系统选型原则**: 用它睡得着觉。如果 crossbeam 有 Bug，半个 Rust 生态都会崩。

#### 使用方式

```rust
use crossbeam_queue::ArrayQueue;

// 创建固定容量的 ring buffer
let queue: ArrayQueue<OrderMessage> = ArrayQueue::new(1024);

// Producer: 非阻塞 push
queue.push(order_msg).unwrap();

// Consumer: 非阻塞 pop  
if let Some(msg) = queue.pop() {
    process(msg);
}
```

#### 性能说明

虽然 `ArrayQueue` 是 MPMC 架构，但在 SPSC 场景下：
- 现代 CPU 分支预测极强
- 额外原子操作开销仅几纳秒
- **安全性远比几纳秒更重要**

**Phase 1 完成！✅**

---

### Phase 2: OrderMessage 类型定义

**文件**: `src/messages.rs`

**目的**: 定义服务间传递的消息类型

```rust
/// 订单消息 - 在 Ring Buffer 中传递
#[derive(Debug, Clone)]
pub struct OrderMessage {
    pub seq_id: SeqNum,       // WAL 分配的全局序号
    pub order: Order,         // 订单内容
    pub timestamp_ns: u64,    // 纳秒时间戳
}

/// Trade Event - ME 输出的成交事件
#[derive(Debug, Clone)]
pub struct TradeEvent {
    pub trade: Trade,
    pub taker_order_id: OrderId,
    pub maker_order_id: OrderId,
}

/// Order Event - 订单状态变更事件
#[derive(Debug, Clone)]
pub enum OrderEvent {
    Accepted { order: Order },
    Rejected { order: Order, reason: &'static str },
    Filled { order: Order },
    PartialFilled { order: Order },
    Cancelled { order: Order },
}
```

---

### Phase 3: WAL (Write-Ahead Log) 实现

**文件**: `src/wal.rs`

**目的**: 订单持久化，是系统的唯一事实来源

```rust
/// Write-Ahead Log for Orders
/// 
/// # 设计原则:
/// - 追加写 (Append-Only)
/// - Group Commit 批量刷盘
/// - 单调递增的 sequence_id
pub struct OrderWal {
    file: BufWriter<File>,
    next_seq: SeqNum,
    pending_count: usize,
}
```

**接口**:
- `append(&mut self, order: &Order) -> SeqNum` - 写入订单，返回序号
- `flush(&mut self) -> io::Result<()>` - 刷盘
- `replay<F>(&self, f: F)` - 重放 WAL（恢复用）

**Group Commit 策略**:
- 每 N 个订单刷一次（可配置，默认 100）
- 或每 T 毫秒刷一次（可配置，默认 1ms）

---

### Phase 4: UBSCore Service 实现

**文件**: `src/ubscore.rs`

**目的**: 所有余额操作的唯一入口，单线程保证原子性

```rust
/// User Balance Core Service
/// 
/// 职责:
/// 1. 管理 Balance State (内存)
/// 2. 写 Order WAL (持久化)
/// 3. 执行 Balance Lock/Unlock/Spend/Deposit
pub struct UBSCore {
    accounts: FxHashMap<UserId, UserAccount>,
    wal: OrderWal,
    config: TradingConfig,
}

impl UBSCore {
    // 查询 (只读)
    pub fn query_balance(&self, user_id: UserId, asset_id: AssetId) -> Balance;
    
    // 订单处理
    pub fn process_order(&mut self, order: Order) -> Result<SeqNum, RejectReason>;
    
    // 成交结算
    pub fn settle_trade(&mut self, trade: &Trade) -> Result<(), &'static str>;
    
    // 取消订单
    pub fn cancel_order(&mut self, order_id: OrderId) -> Result<(), &'static str>;
}
```

**订单处理流程**:
```
process_order(order):
  1. Write to WAL → get seq_id
  2. Calculate required amount
  3. Lock balance
     - Success → return Ok(seq_id)
     - Fail → write Reject event, return Err
```

---

### Phase 5: 重构 main.rs - 集成 UBSCore

**目的**: 将 main.rs 中的余额操作移到 UBSCore

**当前流程** (main.rs):
```rust
// 直接操作 accounts
accounts.get_mut(user_id).get_balance_mut(asset_id).lock(amount);
book.add_order(order);
// 结算
accounts.settle_as_buyer(...);
```

**重构后流程**:
```rust
// 通过 UBSCore
let seq_id = ubscore.process_order(order)?;
let result = engine.process_order(&mut book, order);
for trade in result.trades {
    ubscore.settle_trade(&trade)?;
    ledger.write_entry(&trade);
}
```

---

### Phase 6: 文档更新

**文件**: `docs/src/0x08b-ubscore-implementation.md`

**内容**:
1. UBSCore 服务详解
2. Ring Buffer 原理
3. WAL 设计与 Group Commit
4. 代码示例
5. 性能数据

---

## 验收标准 (Acceptance Criteria)

### 功能测试
- [ ] Ring Buffer: push/pop 正确
- [ ] WAL: 写入/重放正确
- [ ] UBSCore: 余额操作正确
- [ ] E2E: `scripts/test_e2e.sh` 通过

### 性能测试
- [ ] Ring Buffer: > 10M ops/s
- [ ] WAL: > 500K writes/s (with group commit)
- [ ] 整体吞吐量: 不低于当前 baseline

### 代码质量
- [ ] `cargo fmt` 通过
- [ ] `cargo clippy` 无警告
- [ ] `cargo test` 全部通过

---

## 实现顺序 (Implementation Order)

```
Step 1: src/ringbuffer.rs (Ring Buffer)
        ↓
Step 2: src/messages.rs (Message Types)
        ↓
Step 3: src/wal.rs (Write-Ahead Log)
        ↓
Step 4: src/ubscore.rs (UBSCore Service)
        ↓
Step 5: Refactor src/main.rs (集成)
        ↓
Step 6: 测试 + 文档
```

---

## 风险与缓解 (Risks & Mitigations)

| 风险 | 缓解措施 |
|------|---------|
| WAL 性能影响 | Group Commit 减少 fsync 次数 |
| Ring Buffer 容量不足 | 合理设置容量，监控队列深度 |
| 重构引入 bug | 保持测试通过，增量提交 |

---

## 预计时间 (Estimated Time)

| Phase | 预计时间 |
|-------|---------|
| Phase 1: Ring Buffer | ✅ 已完成 (使用 rtrb) |
| Phase 2: Messages | 15 min |
| Phase 3: WAL | 45 min |
| Phase 4: UBSCore | 60 min |
| Phase 5: 集成 | 45 min |
| Phase 6: 文档 | 30 min |
| **Total** | **~3.5 hours** |

---

## 下一步 (Next Step)

开始 **Phase 1: Ring Buffer 实现**

准备好开始实现吗？
