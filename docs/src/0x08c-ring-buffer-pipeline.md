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

### Phase 2: 分离 Version 空间

**目标**：解决 Pipeline 确定性问题

#### 2.1 修改 Balance 结构

```rust
// src/user_account.rs
pub struct Balance {
    avail: u64,
    frozen: u64,
    lock_version: u64,    // 新增
    settle_version: u64,  // 新增
}
```

#### 2.2 Version 递增逻辑

| 操作 | 递增的 Version |
|------|----------------|
| `lock()` | lock_version |
| `unlock()` | lock_version |
| `spend_frozen()` + `deposit()` | settle_version |

#### 2.3 验收标准

- [ ] Balance 结构包含两个 version 字段
- [ ] 每种操作递增正确的 version
- [ ] 单元测试验证

---

### Phase 3: 扩展 BalanceEvent

**目标**：完整的事件溯源

#### 3.1 新增事件类型

```rust
// src/messages.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BalanceEventType {
    Lock,       // 下单锁定
    Unlock,     // 取消释放
    Settle,     // 成交结算
    Deposit,    // 充值
    Withdraw,   // 提现
}

pub struct BalanceEvent {
    pub user_id: u64,
    pub asset_id: u32,
    pub event_type: BalanceEventType,
    pub version: u64,           // 同类型内递增
    pub source_type: SourceType,// Order | Trade | Deposit
    pub source_id: u64,         // order_seq_id | trade_id | ref_id
    pub delta: i64,             // 变更量（可正可负）
    pub avail_after: u64,       // 变更后可用余额
    pub frozen_after: u64,      // 变更后冻结余额
}
```

#### 3.2 验收标准

- [ ] BalanceEvent 定义完整
- [ ] 支持序列化/反序列化
- [ ] 能从 Order/Trade 正确生成事件

---

### Phase 4: Ledger 记录所有操作

**目标**：每个余额变更都有记录

#### 4.1 Lock 事件记录

```rust
// 在 UBSCore::process_order() 中
fn process_order(&mut self, order: InternalOrder) -> Result<...> {
    let seq_id = self.wal.append(&order)?;
    
    // Lock balance
    if let Ok(()) = balance.lock(amount) {
        // 记录 Lock 事件
        let event = BalanceEvent {
            event_type: BalanceEventType::Lock,
            version: balance.lock_version,
            source_type: SourceType::Order,
            source_id: seq_id,
            delta: -(amount as i64),
            // ...
        };
        self.emit_balance_event(event);
    }
}
```

#### 4.2 Settle 事件记录

```rust
// 在 Settlement 中
fn settle_trade(&mut self, trade: &Trade) {
    // Buyer: -quote, +base
    self.emit_balance_event(BalanceEvent {
        event_type: BalanceEventType::Settle,
        source_type: SourceType::Trade,
        source_id: trade.trade_id,
        // ...
    });
}
```

#### 4.3 验收标准

- [ ] Lock 操作生成 BalanceEvent
- [ ] Settle 操作生成 BalanceEvent
- [ ] 事件写入 Ledger 文件

---

### Phase 5: 分类验证测试

**目标**：Pipeline 下的确定性验证

#### 5.1 测试策略

```bash
# 生成输出
cargo run --release

# 分类验证
python scripts/verify_ledger.py output/ledger.csv baseline/ledger.csv
```

验证脚本逻辑：

```python
def verify_ledger(output, baseline):
    # 1. 分类提取
    output_locks = extract_by_type(output, 'Lock')
    output_settles = extract_by_type(output, 'Settle')
    
    baseline_locks = extract_by_type(baseline, 'Lock')
    baseline_settles = extract_by_type(baseline, 'Settle')
    
    # 2. 按 source_id 排序
    output_locks.sort(key=lambda x: x.source_id)
    baseline_locks.sort(key=lambda x: x.source_id)
    
    # 3. 比较
    assert output_locks == baseline_locks, "Lock events mismatch"
    assert output_settles == baseline_settles, "Settle events mismatch"
    
    # 4. 验证 version 连续性
    for user in users:
        lock_versions = get_versions(output_locks, user)
        assert is_consecutive(lock_versions), f"User {user} lock_version not consecutive"
```

#### 5.2 验收标准

- [ ] 验证脚本完成
- [ ] Lock 事件集合匹配
- [ ] Settle 事件集合匹配
- [ ] Version 连续性验证通过

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
