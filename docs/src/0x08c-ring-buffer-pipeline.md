# 0x08c 完整事件流与验证

> **核心目标**：实现完整的事件溯源架构，验证与旧版本的等效性，升级 baseline。

---

## 本章问题

上一章（0x08b）我们实现了 UBSCore 服务，但发现了几个问题：

### 1. Ledger 不完整

当前 Ledger 只记录结算操作（Credit/Debit），缺失其他余额变更：

| 操作 | 当前记录 | 生产要求 |
|------|----------|----------|
| Deposit | ❌ | ✅ |
| **Lock** | ❌ | ✅ |
| **Unlock** | ❌ | ✅ |
| Settle | ❌ | ✅ |

### 2. Pipeline 确定性问题

当采用 Ring Buffer 多阶段 Pipeline 时，Lock 和 Settle 的交错顺序不确定：

```
运行 1: [Lock1, Lock2, Lock3, Settle1, Settle2, Settle3]
运行 2: [Lock1, Settle1, Lock2, Settle2, Lock3, Settle3]
```

**最终状态相同，但中间 version 序列不同** → 无法直接 diff 验证。

---

## 本章目标

### 1. 实现分离 Version 空间

```rust
struct Balance {
    avail: u64,
    frozen: u64,
    lock_version: u64,    // 只在 lock/unlock 时递增
    settle_version: u64,  // 只在 settle 时递增
}
```

### 2. 扩展 BalanceEvent

```rust
struct BalanceEvent {
    user_id: u64,
    asset_id: u32,
    event_type: EventType,  // Deposit | Lock | Unlock | Settle
    version: u64,           // 在对应 version 空间内递增
    source_type: SourceType,// Order | Trade | External
    source_id: u64,         // order_seq_id | trade_id | ref_id
    delta: i64,
    avail_after: u64,
    frozen_after: u64,
}
```

### 3. 记录所有余额操作

```
Order(seq=5) ──触发──→ Lock(buyer USDT, lock_version=1)
     │
     └──→ Trade(id=3)
              │
              ├──触发──→ Settle(buyer: -USDT, +BTC, settle_version=1)
              └──触发──→ Settle(seller: -BTC, +USDT, settle_version=1)
```

### 4. 验证等效性并升级 Baseline

确保重构后的系统与重构前产生相同的最终状态。

---

## 实现进度

### Phase 1: 分离 Version 空间 ✅ 已完成

**目标**：解决 Pipeline 确定性问题

#### 1.1 修改 Balance 结构

```rust
// src/balance.rs
pub struct Balance {
    avail: u64,
    frozen: u64,
    lock_version: u64,    // lock/unlock/deposit/withdraw 操作递增
    settle_version: u64,  // spend_frozen/deposit 操作递增
}
```

#### 1.2 Version 递增逻辑

| 操作 | 递增的 Version |
|------|----------------|
| `deposit()` | lock_version AND settle_version |
| `withdraw()` | lock_version |
| `lock()` | lock_version |
| `unlock()` | lock_version |
| `spend_frozen()` | settle_version |

#### 1.3 等效性验证 ✅

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

╔════════════════════════════════════════════════════════════╗
║     ✅ Baseline equivalence verified!                      ║
╚════════════════════════════════════════════════════════════╝
```

---

### Phase 2: 扩展 BalanceEvent ✅ 已完成

**目标**：完整的事件溯源

#### 2.1 事件类型和结构

已在 `src/messages.rs` 中实现：

```rust
pub enum BalanceEventType { Deposit, Withdraw, Lock, Unlock, Settle }
pub enum SourceType { Order, Trade, External }
pub enum VersionSpace { Lock, Settle, Both }

pub struct BalanceEvent {
    pub user_id: u64,
    pub asset_id: u32,
    pub event_type: BalanceEventType,
    pub version: u64,
    pub source_type: SourceType,
    pub source_id: u64,
    pub delta: i64,
    pub avail_after: u64,
    pub frozen_after: u64,
}
```

#### 2.2 工厂方法

```rust
impl BalanceEvent {
    pub fn lock(...) -> Self;
    pub fn unlock(...) -> Self;
    pub fn settle_spend(...) -> Self;
    pub fn settle_receive(...) -> Self;
    pub fn deposit(...) -> Self;
}
```

---

### Phase 3: Ledger 记录所有操作 ✅ 已完成

**目标**：每个余额变更都有记录

#### 3.1 事件日志文件

UBSCore 模式下生成 `output/t2_events.csv`：

```csv
user_id,asset_id,event_type,version,source_type,source_id,delta,avail_after,frozen_after
655,2,lock,2,order,1,-3315478,996684522,3315478
96,2,settle,2,trade,1,-92889,999907111,0
604,1,deposit,1,external,1,10000000000,10000000000,0
```

#### 3.2 当前记录的操作

| 操作 | 状态 | 说明 |
|------|------|------|
| **Deposit** | ✅ | 初始充值时记录 |
| **Lock** | ✅ | 下单锁定后记录 |
| **Settle** | ✅ | 成交结算后记录 |
| Unlock | ⏳ | 取消订单时记录（当前测试无取消）|
| Withdraw | ⏳ | 提现时记录（当前测试无提现）|

#### 3.3 事件统计

```
Total events: 293,544
  Deposit events: 2,000 (= users × 2 assets)
  Lock events: 100,000 (= accepted orders)
  Settle events: 191,544 (= trades × 4)
```

---

### Phase 4: 验证测试 ✅ 已完成

**目标**：验证事件正确性

#### 4.1 事件正确性验证

`scripts/verify_balance_events.py` - 7 项检查：

| 检查项 | 说明 | 状态 |
|--------|------|------|
| Lock 事件数量 | = 接受的订单数 | ✅ |
| Settle 事件数量 | = 成交数 × 4 | ✅ |
| Lock 版本连续性 | 每个用户-资产对内递增 | ✅ |
| Settle 版本连续性 | 每个用户-资产对内递增 | ✅ |
| Delta 守恒 | 每笔成交的 delta 总和 = 0 | ✅ |
| Source 类型一致性 | Lock→Order, Settle→Trade | ✅ |
| Deposit 事件 | 正 delta + source_type=external | ✅ |

#### 4.2 Events Baseline 验证

`scripts/verify_events_baseline.py` - 严格比较所有 9 个字段：

```bash
$ python3 scripts/verify_events_baseline.py

╔════════════════════════════════════════════════════════════╗
║     Events Baseline Verification                          ║
╚════════════════════════════════════════════════════════════╝

Comparing by event type...
  deposit: output=2000, baseline=2000 ✅
  lock: output=100000, baseline=100000 ✅
  settle: output=191544, baseline=191544 ✅

╔════════════════════════════════════════════════════════════╗
║     ✅ Events match baseline!                             ║
╚════════════════════════════════════════════════════════════╝
```

#### 4.3 完整 E2E 测试

运行 `scripts/test_ubscore_e2e.sh`：

```bash
$ bash scripts/test_ubscore_e2e.sh

=== Step 1: Run with UBSCore mode ===
...
=== Step 2: Verify standard baselines ===
  t1_balances_deposited.csv: ✅ MATCH
  t2_balances_final.csv: ✅ MATCH
  t2_orderbook.csv: ✅ MATCH

=== Step 3: Verify balance events correctness ===
  ✅ All 7 checks passed!

=== Step 4: Verify events baseline ===
  ✅ Events match baseline!

╔════════════════════════════════════════════════════════════╗
║     ✅ All UBSCore E2E tests passed!                       ║
╚════════════════════════════════════════════════════════════╝
```

---

## Baseline 文件

| 文件 | 说明 |
|------|------|
| `baseline/t2_balances_final.csv` | 最终余额状态 |
| `baseline/t2_orderbook.csv` | 最终订单簿状态 |
| `baseline/t2_events.csv` | 事件日志 (293,544 事件) |

---

## 下一步

- **0x08d: 多线程 Pipeline** - 实现 Ring Buffer 连接各服务
- **0x09: 多 Symbol 支持** - 扩展到多交易对

---

## 参考

- [Event Sourcing](https://martinfowler.com/eaaDev/EventSourcing.html) - 事件溯源模式
- [LMAX Disruptor](https://lmax-exchange.github.io/disruptor/) - Ring Buffer 架构原型
