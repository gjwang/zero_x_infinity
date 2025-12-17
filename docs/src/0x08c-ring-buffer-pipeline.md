# 0x08c Ring Buffer Pipeline

> **核心目标**：用 Ring Buffer 连接各服务，优化 Ledger I/O，实现完整的事件溯源架构。

---

## 本章问题

上一章（0x08b）我们实现了 UBSCore 服务，但发现了几个问题：

### 1. Ledger I/O 是性能瓶颈

```
=== Performance Breakdown ===
Balance Check:       23.64ms (  0.4%)
Matching Engine:   2857.50ms ( 46.0%)
Settlement:           8.45ms (  0.1%)
Ledger I/O:        3325.39ms ( 53.5%)  ← 超过一半时间在这里！
```

**Ledger I/O 占用 53.5% 的时间**，成为最大的性能瓶颈。

### 2. Ledger 不完整

当前 Ledger 只记录结算操作（Credit/Debit），缺失其他余额变更：

| 操作 | 当前记录 | 生产要求 |
|------|----------|----------|
| Deposit | ❌ | ✅ |
| **Lock** | ❌ | ✅ |
| **Unlock** | ❌ | ✅ |
| Spend Frozen | ❌ | ✅ |
| Credit | ✅ | ✅ |
| Debit | ✅ | ✅ |

### 3. Pipeline 确定性问题

当采用 Ring Buffer 多阶段 Pipeline 时，Lock 和 Settle 的交错顺序不确定：

```
运行 1: [Lock1, Lock2, Lock3, Settle1, Settle2, Settle3]
运行 2: [Lock1, Settle1, Lock2, Settle2, Lock3, Settle3]
```

**最终状态相同，但中间 version 序列不同** → 无法直接 diff 验证。

---

## 本章目标

### 1. 优化 Ledger I/O 性能

将 53.5% 降到 < 20%，目标方案：

| 方案 | 优化点 | 预期收益 |
|------|--------|----------|
| **批量写入** | 减少系统调用次数 | 3-5x |
| **BufWriter** | 利用缓冲区 | 2-3x |
| **异步写入** | 不阻塞主线程 | 2x |
| **二进制格式** | 替代 CSV | 2x |

### 2. 实现分离 Version 空间

```rust
struct Balance {
    avail: u64,
    frozen: u64,
    lock_version: u64,    // 只在 lock/unlock 时递增
    settle_version: u64,  // 只在 settle 时递增
}
```

### 3. 扩展 BalanceEvent

```rust
struct BalanceEvent {
    user_id: u64,
    asset_id: u32,
    event_type: EventType,  // Lock | Unlock | Settle
    version: u64,           // 只在同类型内递增
    source_id: u64,         // order_seq_id | trade_id
    delta: i64,
    balance_after: u64,
}
```

### 4. 记录所有余额操作

确保每个余额变更都有审计记录：

```
Order(seq=5) ──触发──→ Lock(buyer USDT, lock_version=1)
     │
     └──→ Trade(id=3)
              │
              ├──触发──→ Settle(buyer: -USDT, +BTC, settle_version=1)
              └──触发──→ Settle(seller: -BTC, +USDT, settle_version=1)
```

### 5. 分类验证测试

```
不验证：某时刻的"快照"是否一致（Pipeline 下不可能一致）
验证：  处理完成后的"最终集合"是否一致

分别验证：
  1. Lock 事件集合（按 lock_version 排序）→ 1:1 对应 order_seq_id
  2. Settle 事件集合（按 settle_version 排序）→ 1:1 对应 trade_id
  3. 最终余额 → 完全相同
```

---

## 实现计划

### Phase 1: Ledger I/O 优化

**目标**：将 Ledger I/O 从 53% 降到 < 20%

#### 1.1 分析当前瓶颈

当前实现每次写入都是独立操作：

```rust
// 当前：每笔交易都写一次
for trade in trades {
    ledger.write_entry(&trade);  // 系统调用
}
```

#### 1.2 批量写入优化

```rust
// 优化后：批量缓冲，定期刷盘
impl LedgerWriter {
    buffer: Vec<LedgerEntry>,
    
    pub fn append(&mut self, entry: LedgerEntry) {
        self.buffer.push(entry);
        if self.buffer.len() >= BATCH_SIZE {
            self.flush();
        }
    }
    
    pub fn flush(&mut self) {
        // 一次性写入所有缓冲条目
        for entry in &self.buffer {
            writeln!(self.writer, "{}", entry.to_csv());
        }
        self.writer.flush();
        self.buffer.clear();
    }
}
```

#### 1.3 验收标准

- [ ] Ledger I/O 占比 < 20%
- [ ] 整体吞吐量提升 > 30%
- [ ] E2E 测试通过

---

### Phase 2: 分离 Version 空间 ✅ 已完成

**目标**：解决 Pipeline 确定性问题

#### 2.1 修改 Balance 结构

```rust
// src/balance.rs
pub struct Balance {
    avail: u64,
    frozen: u64,
    lock_version: u64,    // 新增：lock/unlock/deposit/withdraw 操作递增
    settle_version: u64,  // 新增：spend_frozen/deposit 操作递增
}
```

#### 2.2 Version 递增逻辑

| 操作 | 递增的 Version |
|------|----------------|
| `deposit()` | lock_version AND settle_version |
| `withdraw()` | lock_version |
| `lock()` | lock_version |
| `unlock()` | lock_version |
| `spend_frozen()` | settle_version |

#### 2.3 新增 BalanceEvent 类型

```rust
// src/messages.rs
pub enum BalanceEventType { Deposit, Withdraw, Lock, Unlock, Settle }
pub enum SourceType { Order, Trade, External }

pub struct BalanceEvent {
    pub user_id: u64,
    pub asset_id: u32,
    pub event_type: BalanceEventType,
    pub version: u64,           // 在对应 version 空间内
    pub source_type: SourceType,
    pub source_id: u64,         // 因果链 ID
    pub delta: i64,
    pub avail_after: u64,
    pub frozen_after: u64,
}
```

#### 2.4 等效性验证 ✅

**验证脚本**：`scripts/verify_baseline_equivalence.py`

```bash
$ python3 scripts/verify_baseline_equivalence.py

╔════════════════════════════════════════════════════════════╗
║     Baseline Equivalence Verification                      ║
╚════════════════════════════════════════════════════════════╝

=== Step 1: Extract old baseline from v0.8b-ubscore-implementation ===
Old baseline: 2000 rows

=== Step 2: Load current baseline ===
New baseline: 2000 rows

=== Step 3: Compare avail and frozen values ===
✅ EQUIVALENT: avail and frozen values are IDENTICAL

=== Sample version differences (expected) ===
Format: user_id, asset_id | old_version -> new_version
--------------------------------------------------
     1,  1 |   127 ->    79
     1,  2 |   125 ->    95
     2,  1 |   150 ->    84
     2,  2 |   136 ->   109
     3,  1 |   129 ->    86
  ...

=== Explanation ===
Old version = all operations (lock + settle + deposit)
New version = lock_version only (lock + unlock + deposit)

The settle operations now increment a separate settle_version field.

╔════════════════════════════════════════════════════════════╗
║     ✅ Baseline equivalence verified!                      ║
╚════════════════════════════════════════════════════════════╝
```

**关键结论**：
- `avail` 和 `frozen` 值 **完全一致** ✅
- `version` 数值变化 **符合预期**（旧版本 > 新版本）
- 差值 = settle 操作数（现在计入单独的 `settle_version`）

#### 2.5 验收标准 ✅

- [x] Balance 结构包含两个 version 字段
- [x] 每种操作递增正确的 version
- [x] 单元测试验证（50 tests passed）
- [x] E2E 测试通过（4/4 baselines match）
- [x] 等效性验证脚本确认新旧 baseline 等效

---

### Phase 3: 扩展 BalanceEvent ✅ 已完成

**目标**：完整的事件溯源

#### 3.1 事件类型和结构

已在 `src/messages.rs` 中实现：

```rust
pub enum BalanceEventType { Deposit, Withdraw, Lock, Unlock, Settle }
pub enum SourceType { Order, Trade, External }
pub enum VersionSpace { Lock, Settle, Both }

pub struct BalanceEvent {
    pub user_id: u64,
    pub asset_id: u32,
    pub event_type: BalanceEventType,
    pub version: u64,           // 在对应 version 空间内
    pub source_type: SourceType,// Order | Trade | External
    pub source_id: u64,         // order_seq_id | trade_id | ref_id
    pub delta: i64,             // 变更量（可正可负）
    pub avail_after: u64,       // 变更后可用余额
    pub frozen_after: u64,      // 变更后冻结余额
}
```

#### 3.2 工厂方法

```rust
impl BalanceEvent {
    pub fn lock(user_id, asset_id, order_seq_id, amount, ...) -> Self;
    pub fn unlock(user_id, asset_id, order_seq_id, amount, ...) -> Self;
    pub fn settle_spend(user_id, asset_id, trade_id, amount, ...) -> Self;
    pub fn settle_receive(user_id, asset_id, trade_id, amount, ...) -> Self;
    pub fn deposit(user_id, asset_id, ref_id, amount, ...) -> Self;
}
```

#### 3.3 验收标准 ✅

- [x] BalanceEvent 定义完整
- [x] 支持 CSV 序列化 (`to_csv()`, `csv_header()`)
- [x] 能从 Order/Trade 正确生成事件

---

### Phase 4: Ledger 记录所有操作 ✅ 已完成

**目标**：每个余额变更都有记录

#### 4.1 事件日志文件

UBSCore 模式下生成 `output/t2_events.csv`：

```csv
user_id,asset_id,event_type,version,source_type,source_id,delta,avail_after,frozen_after
655,2,lock,2,order,1,-3315478,996684522,3315478
96,2,settle,2,trade,1,-92889,999907111,0
96,1,settle,2,trade,1,1093200,10001093200,0
```

#### 4.2 当前记录的操作

| 操作 | 状态 | 说明 |
|------|------|------|
| **Lock** | ✅ | 下单锁定后记录 |
| **Settle** | ✅ | 成交结算后记录（spend_frozen + receive）|
| Unlock | ⏳ | 取消订单时记录（待实现）|
| Deposit | ⏳ | 充值时记录（待实现）|
| Withdraw | ⏳ | 提现时记录（待实现）|

#### 4.3 事件统计

```
Total events: 291,544
  Lock events: 100,000 (= accepted orders)
  Settle events: 191,544 (= trades × 4)
```

#### 4.4 验收标准 ✅

- [x] Lock 操作生成 BalanceEvent
- [x] Settle 操作生成 BalanceEvent
- [x] 事件写入独立的事件日志文件

---

### Phase 5: 分类验证测试 ✅ 已完成

**目标**：验证事件正确性

#### 5.1 验证脚本

`scripts/verify_balance_events.py`

```bash
$ python3 scripts/verify_balance_events.py

╔════════════════════════════════════════════════════════════╗
║     Balance Events Verification                           ║
╚════════════════════════════════════════════════════════════╝

Loaded 291544 events
  Lock events: 100000
  Settle events: 191544
  Unlock events: 0
  Deposit events: 0

=== Check 1: Lock events vs Accepted orders ===
✅ Lock events (100000) = Accepted orders (100000)

=== Check 2: Settle events vs Trades ===
✅ Settle events (191544) = Trades * 4 (191544)

=== Check 3: Lock version continuity ===
✅ All lock versions are increasing (2000 user-asset pairs)

=== Check 4: Settle version continuity ===
✅ All settle versions are increasing (2000 user-asset pairs)

=== Check 5: Settle delta conservation by trade ===
✅ All trades have zero sum delta (47886 trades)

=== Check 6: Source type consistency ===
✅ All lock events have source_type='order'
✅ All settle events have source_type='trade'

╔════════════════════════════════════════════════════════════╗
║     ✅ All balance event checks passed!                   ║
╚════════════════════════════════════════════════════════════╝
```

#### 5.2 验证项目

| 检查项 | 说明 | 状态 |
|--------|------|------|
| Lock 事件数量 | = 接受的订单数 | ✅ |
| Settle 事件数量 | = 成交数 × 4 | ✅ |
| Lock 版本连续性 | 每个用户-资产对内递增 | ✅ |
| Settle 版本连续性 | 每个用户-资产对内递增 | ✅ |
| Delta 守恒 | 每笔成交的 delta 总和 = 0 | ✅ |
| Source 类型一致性 | Lock→Order, Settle→Trade | ✅ |

#### 5.3 验收标准 ✅

- [x] 验证脚本完成
- [x] Lock 事件数量正确
- [x] Settle 事件数量正确
- [x] Version 连续性验证通过
- [x] Delta 守恒验证通过

---

### Phase 6: Ring Buffer 集成（可选）

**目标**：完整的多阶段 Pipeline

> ⚠️ 这是可选任务，当前单线程模式已满足性能需求。

```
Gateway → [Ring Buffer] → UBSCore → [Ring Buffer] → ME → [Ring Buffer] → Settlement
```

如果需要实现：

```rust
use crossbeam_queue::ArrayQueue;

// UBSCore → ME
let order_queue: Arc<ArrayQueue<ValidOrder>> = Arc::new(ArrayQueue::new(1024));

// ME → Settlement
let trade_queue: Arc<ArrayQueue<TradeEvent>> = Arc::new(ArrayQueue::new(1024));
```

---

## 预期成果

| 指标 | 当前 (0x08b) | 目标 (0x08c) |
|------|--------------|--------------|
| Ledger I/O 占比 | 53.5% | < 20% |
| 整体吞吐量 | ~16K ops/s | > 20K ops/s |
| Ledger 完整性 | Credit/Debit only | All ops |
| 确定性验证 | 全局 diff | 分类验证 |

---

## 下一步

完成本章后，可以进入：

- **0x09: 多 Symbol 支持** - 扩展到多交易对
- **0x0A: 网络层** - 真实的 Gateway 服务
- **0x0B: 持久化** - 数据库集成

---

## 参考

- [LMAX Disruptor](https://lmax-exchange.github.io/disruptor/) - Ring Buffer 架构原型
- [Event Sourcing](https://martinfowler.com/eaaDev/EventSourcing.html) - 事件溯源模式
- [crossbeam-queue](https://docs.rs/crossbeam-queue/) - Rust 无锁队列
