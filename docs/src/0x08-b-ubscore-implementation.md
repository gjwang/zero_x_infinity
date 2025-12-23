# 0x08-b UBSCore Implementation

<h3>
  <a href="#-english">🇺🇸 English</a>
  &nbsp;&nbsp;&nbsp;|&nbsp;&nbsp;&nbsp;
  <a href="#-chinese">🇨🇳 中文</a>
</h3>

<div id="-english"></div>

## 🇺🇸 English

> **📦 Code Changes**: [View Diff](https://github.com/gjwang/zero_x_infinity/compare/v0.8-a-trading-pipeline-design...v0.8-b-ubscore-implementation)

> **Objective**: From design to implementation: Building a Safety-First Balance Core Service.

In the previous chapter (0x08-a), we designed the full HFT pipeline architecture. Now, it's time to implement the core components. This chapter covers:

1.  **Ring Buffer** - Lock-free inter-service communication.
2.  **Write-Ahead Log (WAL)** - Order persistence.
3.  **UBSCore Service** - The core balance service.

### 1. Technology Selection: Safety First

In financial systems, **maturity and stability** outweigh extreme performance.

#### 1.1 Ring Buffer Selection

| Crate | Maturity | Security | Performance |
|-------|----------|----------|-------------|
| `crossbeam-queue` | 🌟🌟🌟🌟🌟 (3.3M+ DLs) | Heavily Audited | Very Low Latency |
| `ringbuf` | 🌟🌟🌟🌟 (600K+ DLs) | Community Verified | Lower Latency |
| `rtrb` | 🌟🌟🌟 (Newer) | Less Vetted | Lowest Latency |

**Our Choice: `crossbeam-queue`**

Reasons:
*   Maintained by Rust core team members.
*   Base dependency for `tokio`, `actix`, `rayon`.
*   If it has a bug, half the Rust ecosystem collapses.

> **Financial System Selection Principle**: Use what lets you sleep at night.

```rust
use crossbeam_queue::ArrayQueue;

// Create fixed-size ring buffer
let queue: ArrayQueue<OrderMessage> = ArrayQueue::new(1024);

// Producer: Non-blocking push
queue.push(order_msg).unwrap();

// Consumer: Non-blocking pop
if let Some(msg) = queue.pop() {
    process(msg);
}
```

### 2. Write-Ahead Log (WAL)

WAL is the system's **Single Source of Truth**.

#### 2.1 Design Principles

```rust
/// Write-Ahead Log for Orders
///
/// Principles:
/// 1. Append-Only: Sequential I/O, max performance.
/// 2. Group Commit: Batch fsyncs.
/// 3. Monotonic sequence_id: Deterministic replay.
pub struct WalWriter {
    writer: BufWriter<File>,
    next_seq: SeqNum,
    pending_count: usize,
    config: WalConfig,
}
```

#### 2.2 Group Commit Strategy

| Flush Strategy | Latency | Throughput | Safety |
|----------------|---------|------------|--------|
| Every Entry | ~50µs | ~20K/s | Highest |
| Every 100 Entries | ~5µs (amortized) | ~200K/s | High |
| Every 1ms | ~1µs (amortized) | ~1M/s | Medium |

We choose **Every 100 Entries** to balance performance and safety:

```rust
pub struct WalConfig {
    pub path: String,
    pub flush_interval_entries: usize,  // Flush every N entries
    pub sync_on_flush: bool,            // Whether to call fsync
}
```

#### 2.3 WAL Entry Format

Currently CSV (readable for dev):
```
seq_id,timestamp_ns,order_id,user_id,price,qty,side,order_type
1,1702742400000000000,1001,100,85000000000,100000000,Buy,Limit
```

In production, switch to Binary (54 bytes/entry) for better performance.

### 3. UBSCore Service

UBSCore is the **Single Entry Point** for all balance operations.

#### 3.1 Responsibilities

1.  **Balance State Management**: In-memory balance state.
2.  **Order WAL Writing**: Persist orders.
3.  **Balance Operations**: lock/unlock/spend_frozen/deposit.

#### 3.2 Core Structure

```rust
pub struct UBSCore {
    /// User Accounts - Authoritative Balance State
    accounts: FxHashMap<UserId, UserAccount>,
    /// Write-Ahead Log
    wal: WalWriter,
    /// Configuration
    config: TradingConfig,
    /// Pending Orders (Locked but not filled)
    pending_orders: FxHashMap<OrderId, PendingOrder>,
    /// Statistics
    stats: UBSCoreStats,
}
```

#### 3.3 Order Processing Flow

```
process_order(order):
  │
  ├─ 1. Write to WAL ──────────► Get seq_id
  │
  ├─ 2. Validate order ────────► Check price/qty
  │
  ├─ 3. Get user account ──────► Lookup user
  │
  ├─ 4. Calculate lock amount ─► Buy: price * qty / qty_unit
  │                              Sell: qty
  │
  └─ 5. Lock balance ──────────► Success → Ok(ValidOrder)
                                 Fail    → Err(Rejected)
```

Implementation:

```rust
pub fn process_order(&mut self, order: Order) -> Result<ValidOrder, OrderEvent> {
    // Step 1: Write to WAL FIRST (persist before any state change)
    let seq_id = self.wal.append(&order)?;

    // Step 2-4: Validate and calculate
    // ...

    // Step 5: Lock balance
    let lock_result = account
        .get_balance_mut(locked_asset_id)
        .and_then(|balance| balance.lock(locked_amount));

    match lock_result {
        Ok(()) => {
            // Track pending order
            self.pending_orders.insert(order.id, PendingOrder { ... });
            Ok(ValidOrder::new(seq_id, order, locked_amount, locked_asset_id))
        }
        Err(_) => Err(OrderEvent::Rejected { ... })
    }
}
```

#### 3.4 Settlement

```rust
pub fn settle_trade(&mut self, event: &TradeEvent) -> Result<(), &'static str> {
    let trade = &event.trade;
    let quote_amount = trade.price * trade.qty / self.config.qty_unit();

    // Buyer: spend USDT, receive BTC
    buyer.get_balance_mut(quote_id)?.spend_frozen(quote_amount)?;
    buyer.get_balance_mut(base_id)?.deposit(trade.qty)?;

    // Seller: spend BTC, receive USDT
    seller.get_balance_mut(base_id)?.spend_frozen(trade.qty)?;
    seller.get_balance_mut(quote_id)?.deposit(quote_amount)?;

    Ok(())
}
```

### 4. Message Types

Services communicate via defined message types:

```rust
// Gateway → UBSCore
pub struct OrderMessage {
    pub seq_id: SeqNum,
    pub order: Order,
    // ...
}

// UBSCore → ME
pub struct ValidOrder {
    pub seq_id: SeqNum,
    pub order: Order,
    pub locked_amount: u64,
    // ...
}

// ME → UBSCore + Settlement
pub struct TradeEvent {
    pub trade: Trade,
    pub taker_order_id: OrderId,
    pub maker_order_id: OrderId,
    // ...
}
```

### 5. Integration & Usage

#### 5.1 CLI Arguments

```bash
# Original Pipeline
cargo run --release

# UBSCore Pipeline (Enable WAL)
cargo run --release -- --ubscore
```

#### 5.2 Performance Comparison

| Metric | Original | UBSCore | Change |
|--------|----------|---------|--------|
| Throughput | 15,070 ops/s | 14,314 ops/s | -5% |
| WAL Entries | N/A | 100,000 | 6.67 MB |
| Balance Check | 0.3% | 1.3% | +1% |
| Matching | 45.5% | 45.5% | - |
| Settlement | 0.1% | 0.2% | - |
| Ledger I/O | 54.0% | 53.0% | -1% |

**Analysis**:
*   WAL introduces ~5% overhead.
*   **Acceptable cost** for safety.
*   Main bottleneck remains Ledger I/O.

### 6. Tests

#### 6.1 Unit Tests

```bash
cargo test
# 31 tests passing
```

#### 6.2 E2E Tests

```bash
sh scripts/test_e2e.sh
# ✅ All tests passed!
```

### 7. New Files

| File | Lines | Description |
|------|-------|-------------|
| `src/messages.rs` | 265 | Inter-service messages |
| `src/wal.rs` | 340 | Write-Ahead Log |
| `src/ubscore.rs` | 490 | User Balance Core |

### 8. Key Learnings

#### 8.1 Safety First
*   **Maturity > Performance**
*   **Auditable > Rapid Dev**

#### 8.2 WAL is Single Source of Truth
`All state = f(WAL)`. Foundation for Disaster Recovery and Audit.

#### 8.3 Single Thread Advantage
UBSCore uses single thread for natural atomicity (no locking needed for balance ops) and predictable latency.

### 9. Critical Bug Fix: Cost Calculation Overflow

#### 9.1 The Issue
Testing with `--ubscore` revealed **1032 rejected orders** that were accepted in the legacy mode.

#### 9.2 Root Cause
**Overflow in `price * qty` (u64)**.

Example Order #21:
*   Price: 84,956.01 USDT (6 decimals) -> `84,956,010,000`
*   Qty: 2.56 BTC (8 decimals) -> `256,284,400`
*   Product: `2.177 × 10^19` > `u64::MAX`

#### 9.3 Why Legacy Mode Passed?
**Release Code Wrapping Arithmetic**:
Legacy code `cost = price * qty` wrapped around, resulting in a **much smaller, incorrect value**. users were locked for 33k USDT but bought 217k USDT worth of BTC!

#### 9.4 The Fix

```rust
// Use u128 for intermediate calculation
let cost_128 = (self.price as u128) * (self.qty as u128) / (qty_unit as u128);
if cost_128 > u64::MAX as u128 {
    Err(CostError::Overflow)
}
```

#### 9.5 Configuration Issue
USDT with 6 decimals is risky. Recommended: 2 decimals.
**Binance uses 2 decimals for USDT price.**

### 10. Improvement: Ledger Integrity & Determinism

#### 10.1 Incomplete Ledger
Current Ledger lacks `Deposit`, `Lock`, `Unlock`, `SpendFrozen`. Only tracks `Settlement`.

#### 10.2 Pipeline Non-Determinism
Pipeline concurrency means `Lock` and `Settlement` events interleave non-deterministically.
Snapshot comparison is impossible.

#### 10.3 Solution: Version Space Separation
Separate version counters for Lock events and Settle events.

| Version Space | Increment On | Sort By | Determinism |
|---------------|--------------|---------|-------------|
| `lock_version` | Lock/Unlock | `order_seq_id` | ✅ Deterministic |
| `settle_version` | Settle | `trade_id` | ✅ Deterministic |

**Validation Strategy**:
Verify the **Final Set** of events, sorted by their respective versions/source IDs, rather than checking snapshot consistency at arbitrary times.

### 11. Design Discussion: Causal Chain

UBSCore has inputs from `OrderQueue` and `TradeQueue`. Interleaving is random.

**Solution**:
1.  **OrderQueue** strictly follows `order_seq_id`.
2.  **TradeQueue** strictly follows `trade_id`.
3.  Link every Balance Event to its source (`order_seq_id` or `trade_id`).
4.  This forms a **Causal Chain** for audit.

```rust
struct BalanceEvent {
    // ...
    source_type: SourceType, // Order | Trade
    source_id: u64,          // order_seq_id | trade_id
}
```

This allows offline verification:
`Lock(source=Order N)` must exist if `Order N` exists.
`Settle(source=Trade M)` must exist if `Trade M` exists.

### 12. Next Steps (0x08-c)

1.  Implement Version Space Separation.
2.  Expand `BalanceEvent` with causal links.
3.  Integrate Ring Buffer.
4.  Develop Causal Chain Audit Tools.

<br>
<div align="right"><a href="#-english">↑ Back to Top</a></div>
<br>

---

<div id="-chinese"></div>

## 🇨🇳 中文

> **📦 代码变更**: [查看 Diff](https://github.com/gjwang/zero_x_infinity/compare/v0.8-a-trading-pipeline-design...v0.8-b-ubscore-implementation)

> 从设计到实现：构建安全第一的余额核心服务

### 概述

在上一章（0x08-a）中，我们设计了完整的 HFT 交易流水线架构。现在，是时候实现核心组件了。本章我们将构建：

1. **Ring Buffer** - 服务间无锁通信
2. **Write-Ahead Log (WAL)** - 订单持久化
3. **UBSCore Service** - 余额核心服务

### 1. 技术选型：安全第一

在金融系统中，**成熟稳定**比极致性能更重要。

#### 1.1 Ring Buffer 选型

| 库 | 成熟度 | 安全性 | 性能 |
|----|--------|--------|------|
| `crossbeam-queue` | 🌟🌟🌟🌟🌟 (330万+下载) | 最严苛审计 | 极低延迟 |
| `ringbuf` | 🌟🌟🌟🌟 (60万+下载) | 社区验证 | 更低延迟 |
| `rtrb` | 🌟🌟🌟 (较新) | 较少审查 | 最低延迟 |

**我们的选择：`crossbeam-queue`**

理由：
- Rust 核心团队成员参与维护
- 被 tokio, actix, rayon 作为底层依赖
- 如果它有 Bug，半个 Rust 生态都会崩

> **金融系统选型原则**：用它睡得着觉。

```rust
use crossbeam_queue::ArrayQueue;

// 创建固定容量的 ring buffer
let queue: ArrayQueue<OrderMessage> = ArrayQueue::new(1024);

// 生产者：非阻塞 push
queue.push(order_msg).unwrap();

// 消费者：非阻塞 pop
if let Some(msg) = queue.pop() {
    process(msg);
}
```

### 2. Write-Ahead Log (WAL)

WAL 是系统的**唯一事实来源 (Single Source of Truth)**。

#### 2.1 设计原则

```rust
/// Write-Ahead Log for Orders
///
/// 设计原则:
/// 1. 追加写 (Append-Only) - 顺序 I/O，最大化性能
/// 2. Group Commit - 批量刷盘，减少 fsync 次数
/// 3. 单调递增 sequence_id - 保证确定性重放
pub struct WalWriter {
    writer: BufWriter<File>,
    next_seq: SeqNum,
    pending_count: usize,
    config: WalConfig,
}
```

#### 2.2 Group Commit 策略

| 刷盘策略 | 延迟 | 吞吐量 | 数据安全 |
|----------|------|--------|----------|
| 每条 fsync | ~50µs | ~20K/s | 最高 |
| 每 100 条 | ~5µs (均摊) | ~200K/s | 高 |
| 每 1ms | ~1µs (均摊) | ~1M/s | 中 |

我们选择 **每 100 条刷盘**，在性能和安全间取得平衡：

```rust
pub struct WalConfig {
    pub path: String,
    pub flush_interval_entries: usize,  // 每 N 条刷盘
    pub sync_on_flush: bool,            // 是否调用 fsync
}
```

#### 2.3 WAL 条目格式

当前使用 CSV 格式（开发阶段可读性好）：

```
seq_id,timestamp_ns,order_id,user_id,price,qty,side,order_type
1,1702742400000000000,1001,100,85000000000,100000000,Buy,Limit
```

生产环境可切换为二进制格式（54 bytes/entry）以提升性能。

### 3. UBSCore Service

UBSCore 是所有余额操作的**唯一入口**。

#### 3.1 职责

1. **Balance State Management** - 内存中的余额状态
2. **Order WAL Writing** - 持久化订单
3. **Balance Operations** - lock/unlock/spend_frozen/deposit

#### 3.2 核心结构

```rust
pub struct UBSCore {
    /// 用户账户 - 权威余额状态
    accounts: FxHashMap<UserId, UserAccount>,
    /// Write-Ahead Log
    wal: WalWriter,
    /// 交易配置
    config: TradingConfig,
    /// 待处理订单（已锁定但未成交）
    pending_orders: FxHashMap<OrderId, PendingOrder>,
    /// 统计信息
    stats: UBSCoreStats,
}
```

#### 3.3 订单处理流程

```
process_order(order):
  │
  ├─ 1. Write to WAL ──────────► 获得 seq_id
  │
  ├─ 2. Validate order ────────► 价格/数量检查
  │
  ├─ 3. Get user account ──────► 查找用户
  │
  ├─ 4. Calculate lock amount ─► Buy: price * qty / qty_unit
  │                              Sell: qty
  │
  └─ 5. Lock balance ──────────► Success → Ok(ValidOrder)
                                 Fail    → Err(Rejected)
```

代码实现：

```rust
pub fn process_order(&mut self, order: Order) -> Result<ValidOrder, OrderEvent> {
    // Step 1: Write to WAL FIRST (persist before any state change)
    let seq_id = self.wal.append(&order)?;

    // Step 2-4: Validate and calculate
    // ...

    // Step 5: Lock balance
    let lock_result = account
        .get_balance_mut(locked_asset_id)
        .and_then(|balance| balance.lock(locked_amount));

    match lock_result {
        Ok(()) => {
            // Track pending order
            self.pending_orders.insert(order.id, PendingOrder { ... });
            Ok(ValidOrder::new(seq_id, order, locked_amount, locked_asset_id))
        }
        Err(_) => Err(OrderEvent::Rejected { ... })
    }
}
```

#### 3.4 成交结算

```rust
pub fn settle_trade(&mut self, event: &TradeEvent) -> Result<(), &'static str> {
    let trade = &event.trade;
    let quote_amount = trade.price * trade.qty / self.config.qty_unit();

    // Buyer: spend USDT, receive BTC
    buyer.get_balance_mut(quote_id)?.spend_frozen(quote_amount)?;
    buyer.get_balance_mut(base_id)?.deposit(trade.qty)?;

    // Seller: spend BTC, receive USDT
    seller.get_balance_mut(base_id)?.spend_frozen(trade.qty)?;
    seller.get_balance_mut(quote_id)?.deposit(quote_amount)?;

    Ok(())
}
```

### 4. 消息类型

服务间通过明确定义的消息类型通信：

```rust
// Gateway → UBSCore
pub struct OrderMessage {
    pub seq_id: SeqNum,
    pub order: Order,
    // ...
}

// UBSCore → ME
pub struct ValidOrder {
    pub seq_id: SeqNum,
    pub order: Order,
    pub locked_amount: u64,
    // ...
}

// ME → UBSCore + Settlement
pub struct TradeEvent {
    pub trade: Trade,
    pub taker_order_id: OrderId,
    pub maker_order_id: OrderId,
    // ...
}
```

### 5. 集成与使用

#### 5.1 命令行参数

```bash
# 原始流水线
cargo run --release

# UBSCore 流水线（启用 WAL）
cargo run --release -- --ubscore
```

#### 5.2 性能对比

| 指标 | 原始 | UBSCore | 变化 |
|------|------|---------|------|
| 吞吐量 | 15,070 ops/s | 14,314 ops/s | -5% |
| WAL 条目 | N/A | 100,000 | 6.67 MB |
| 余额检查 | 0.3% | 1.3% | +1% |
| 匹配引擎 | 45.5% | 45.5% | - |
| 结算 | 0.1% | 0.2% | - |
| 账本 I/O | 54.0% | 53.0% | -1% |

**分析**：
- WAL 写入引入约 5% 的开销
- 这是**可接受的代价**，换取了数据安全性
- 主要瓶颈仍是 Ledger I/O（下一章优化目标）

### 6. 测试

#### 6.1 单元测试

```bash
cargo test
# 31 tests passing
```

#### 6.2 E2E 测试

```bash
sh scripts/test_e2e.sh
# ✅ All tests passed!
```

### 7. 新增文件

| 文件 | 行数 | 描述 |
|------|------|------|
| `src/messages.rs` | 265 | 服务间消息类型 |
| `src/wal.rs` | 340 | Write-Ahead Log |
| `src/ubscore.rs` | 490 | User Balance Core |

### 8. 关键学习

#### 8.1 安全第一
- **成熟稳定** > 极致性能
- **可审计** > 快速开发
- **用它睡得着觉** 是选型的最高标准

#### 8.2 WAL 是唯一事实来源
`All state = f(WAL)`。任何时刻，系统状态都可以从 WAL 100% 重建。这也是灾难恢复和审计合规的基础。

#### 8.3 单线程是优势
UBSCore 选择单线程不是因为简单，而是因为：
- 自然的原子性（无锁）
- 不可能双重支付
- 可预测的延迟

### 9. 重要 Bug 修复：Cost 计算溢出

#### 9.1 问题发现
在实现 UBSCore 并运行 `--ubscore` 模式测试时，发现了 **1032 个订单被拒绝**，而传统模式全部接受。

#### 9.2 根本原因
**Cost 计算时 `price * qty` 溢出 u64**。

订单 #21:
- `price = 84,956,010,000` (84956.01 USDT，6位精度)
- `qty = 256,284,400` (2.562844 BTC，8位精度)
- `price * qty = 2.177 × 10^19` > u64::MAX

#### 9.3 传统模式为什么没报错？
**Release 模式的 wrapping arithmetic！**
传统模式下，溢出后值变小，虽然通过了检查，但是**锁定的金额严重不足**！这是一个巨大的金融漏洞。

#### 9.4 修复方案

```rust
// 使用 u128 进行中间计算
let cost_128 = (self.price as u128) * (self.qty as u128) / (qty_unit as u128);
if cost_128 > u64::MAX as u128 {
    Err(CostError::Overflow)
}
```

#### 9.5 配置问题：USDT 精度过高
USDT 使用 6 位精度导致溢出风险。建议使用 2 位精度（Binance 标准）。

### 10. 待改进：Ledger 完整性与确定性

#### 10.1 当前 Ledger 不完整
当前 Ledger 缺失 Deposit, Lock, Unlock, SpendFrozen 等操作。

#### 10.2 Pipeline 模式的确定性问题
由于 Ring Buffer 并行处理，Lock 和 Settle 事件的交错顺序不固定，导致无法通过快照对比来验证一致性。

#### 10.3 解决方案：分离 Version 空间
为每种事件类型维护独立的 version：

| Version 空间 | 递增条件 | 排序依据 | 确定性 |
|-------------|----------|----------|--------|
| `lock_version` | Lock/Unlock 事件 | `order_seq_id` | ✅ 确定 |
| `settle_version` | Settle 事件 | `trade_id` | ✅ 确定 |

**验证策略**：
不再验证任意时刻的快照，而是验证处理完成后的最终事件集合（按各自 Version 排序）。

### 11. 设计讨论全记录

#### 11.1 因果链设计

UBSCore 有两个输入源：OrderQueue 和 TradeQueue。
为了审计，我们建立了因果链：

```rust
struct BalanceEvent {
    // ...
    source_type: SourceType, // Order | Trade
    source_id: u64,          // order_seq_id | trade_id
}
```

这不仅解决了审计问题，还让我们可以快速定位问题源头：Lock 必定对应一个 Order，Settle 必定对应一个 Trade。

### 12. 下一章任务 (0x08-c)

1. 实现分离 Version 空间 - `lock_version` / `settle_version`
2. 扩展 `BalanceEvent` - 添加 `event_type`, `version`, `source_id`
3. Ring Buffer 集成
4. 因果链审计工具

