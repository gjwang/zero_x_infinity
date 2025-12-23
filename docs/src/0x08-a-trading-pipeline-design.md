# 0x08-a Trading Pipeline Design

<h3>
  <a href="#-english">🇺🇸 English</a>
  &nbsp;&nbsp;&nbsp;|&nbsp;&nbsp;&nbsp;
  <a href="#-chinese">🇨🇳 中文</a>
</h3>

<div id="-english"></div>

## 🇺🇸 English

> **📦 Code Changes**: [View Diff](https://github.com/gjwang/zero_x_infinity/compare/v0.7-b-perf-baseline...v0.8-a-trading-pipeline-design)

> **Core Objective**: To design a complete trading pipeline architecture that ensures order persistence, balance consistency, and system recoverability.

This chapter addresses the most critical design issues in a matching engine: **Service Partitioning, Data Flow, and Atomicity Guarantees**.

### 1. Why Persistence?

#### 1.1 The Problem Scenario

Suppose the system crashes during matching:

```
User A sends Buy Order → ME receives & fills → System Crash
                                               ↓
                                        User A's funds deducted
                                        But no trade record
                                        Order Lost!
```

**Consequences of No Persistence**:
*   **Order Loss**: User orders vanish.
*   **Inconsistent State**: Funds changed but no record exists.
*   **Unrecoverable**: Upon restart, valid orders are unknown.

#### 1.2 Solution: Persist First, Match Later

```
User A Buy Order → WAL Persist → ME Match → System Crash
                     ↓             ↓
                Order Saved    Replay & Recover!
```

### 2. Unique Ordering

#### 2.1 Why Unique Ordering?

In distributed systems, multiple nodes must agree on order sequence:

| Scenario | Problem |
|----------|---------|
| Node A receives Order 1 then Order 2 | |
| Node B receives Order 2 then Order 1 | Inconsistent Order! |

**Result**: Matching results differ between nodes!

#### 2.2 Solution: Single Sequencer + Global Sequence ID

```
All Orders → Sequencer → Assign Global sequence_id → Persist → Dispatch to ME
              ↓
         Unique Arrival Order
```

| Field | Description |
|-------|-------------|
| `sequence_id` | Monotonically increasing global ID |
| `timestamp` | Nanosecond precision timestamp |
| `order_id` | Business level Order ID |

### 3. Order Lifecycle

#### 3.1 Persist First, Execute Later

```
┌─────────────────────────────────────────────────────────────────────────┐
│                          Order Lifecycle                                │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│   ┌─────────┐    ┌─────────┐    ┌─────────┐    ┌─────────┐             │
│   │ Gateway │───▶│Pre-Check│───▶│   WAL   │───▶│   ME    │             │
│   │(Receiver)│    │(Balance) │    │(Persist)│    │ (Match) │             │
│   └─────────┘    └─────────┘    └─────────┘    └─────────┘             │
│        │              │              │              │                   │
│        ▼              ▼              ▼              ▼                   │
│   Receive Order   Insufficient?   Disk Write     Execute Match           │
│                   Early Reject    Assign SeqID   Guaranteed Exec         │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

#### 3.2 Pre-Check: Reducing Invalid Orders

Pre-Check queries **UBSCore** (User Balance Core Service) for balance info. **Read-Only, No Side Effects**.

```rust
async fn pre_check(order: Order) -> Result<Order, Reject> {
    // 1. Query UBSCore for balance (Read-Only)
    let balance = ubscore.query_balance(order.user_id, asset);

    // 2. Calculate required amount
    let required = match order.side {
        Buy  => order.price * order.qty / QTY_UNIT,  // quote
        Sell => order.qty,                            // base
    };

    // 3. Balance Check (Read-Only, No Lock)
    if balance.avail < required {
        return Err(Reject::InsufficientBalance);
    }

    // 4. Pass
    Ok(order)
}
// Note: Balance might be consumed by others between Pre-Check and WAL.
// This is allowed; WAL's Balance Lock will handle it.
```

**Why Pre-Check?**

The Core Flow (WAL + Balance Lock + Matching) is expensive. We must filter garbage orders **fast**.

| No Pre-Check | With Pre-Check |
|--------------|----------------|
| Garbage enters core flow | Filters most invalid orders |
| Core wastes latency on invalid orders | Core processes mostly valid orders |
| Vulnerable to spam attacks | Reduces impact of malicious requests |

 **Pre-Check Items**:
*   ✅ Balance Check
*   📋 User Status (Banned?)
*   📋 Format Validation
*   📋 Rate Limiting
*   📋 Risk Rules

#### 3.3 Must Execute Once Persisted

Once an order is persisted, it MUST end in one of these states:

```
┌─────────────────────┐
│   Order Persisted   │
└─────────────────────┘
           │
           ├──▶ Filled
           ├──▶ PartialFilled
           ├──▶ New (Booked)
           ├──▶ Cancelled
           ├──▶ Expired
           └──▶ Rejected (Insufficient Balance) ← Valid Final State!

❌ Never: Logged but state unknown.
```

### 4. WAL: Why it's the Best Choice?

#### 4.1 What is WAL (Write-Ahead Log)?

WAL is an **Append-Only** log structure:

```
┌─────────────────────────────────────────────────────────────────┐
│                          WAL File                               │
├─────────────────────────────────────────────────────────────────┤
│  Entry 1  │  Entry 2  │  Entry 3  │  Entry 4  │  ...  │ ← Append│
│ (seq=1)   │ (seq=2)   │ (seq=3)   │ (seq=4)   │       │         │
└─────────────────────────────────────────────────────────────────┘
                                                          ↑
                                                     Append Only!
```

#### 4.2 Why WAL for HFT?

| Method | Write Pattern | Latency | Throughput | HFT Suitability |
|--------|---------------|---------|------------|-----------------|
| DB (MySQL) | Random + Txn | ~1-10ms | ~1K ops/s | ❌ Too Slow |
| KV (Redis) | Random | ~0.1-1ms | ~10K ops/s | ⚠️ Average |
| **WAL** | **Sequential** | **~1-10µs** | **~1M ops/s** | ✅ **Best** |

**Why is WAL fast?**

1.  **Sequential Write vs Random Write**:
    *   HDD: No seek time (~10ms saved).
    *   SSD: Reduces Write Amplification.
    *   Result: **10-100x faster**.
2.  **No Transaction Overhead**:
    *   DB: Txn start, lock, redo log, data page, binlog, commit...
    *   WAL: Serialize -> Append -> (Optional) Fsync.
3.  **Group Commit**:
    *   Batch multiple writes into one `fsync`.

```rust
// Group Commit Logic
pub fn flush(&mut self) -> io::Result<()> {
    self.file.write_all(&self.buffer)?;
    self.file.sync_data()?;  // fsync once for N orders
    self.buffer.clear();
    Ok(())
}
```

### 5. Single Thread + Lock-Free Architecture

#### 5.1 Why Single Thread?

Intuition: Concurrency = Fast.
Reality in HFT: **Single Thread is Faster**.

| Multi-Thread | Single Thread |
|--------------|---------------|
| Locks & Contention | Lock-Free |
| Cache Invalidation | Cache Friendly |
| Context Switch Overhead | No Context Switch |
| Hard Ordering | Naturally Ordered |
| Complex Sync Logic | Simple Code |

#### 5.2 Mechanical Sympathy

**CPU Cache Hierarchy**:
*   L1 Cache: ~1ns
*   L2 Cache: ~4ns
*   RAM: ~100ns

Single Thread Advantage: Data stays in L1/L2 (Hot). No cache line contention.

#### 5.3 LMAX Disruptor Pattern

Originating from LMAX Exchange (6M TPS on single thread):

1.  **Single Writer** (Avoid write contention)
2.  **Pre-allocated Memory** (Avoid GC/malloc)
3.  **Cache Padding** (Avoid false sharing)
4.  **Batch Consumption**

### 6. Ring Buffer: Inter-Service Communication

#### 6.1 Why Ring Buffer?

| Method | Latency | Throughput |
|--------|---------|------------|
| HTTP/gRPC | ~1ms | ~10K/s |
| Kafka | ~1-10ms | ~1M/s |
| **Shared Memory Ring Buffer** | **~100ns** | **~10M/s** |

#### 6.2 Ring Buffer Principle

```
      write_idx                       read_idx
          ↓                               ↓
   ┌───┬───┬───┬───┬───┬───┬───┬───┬───┬───┐
   │ 8 │ 9 │10 │11 │12 │13 │14 │ 0 │ 1 │ 2 │ ...
   └───┴───┴───┴───┴───┴───┴───┴───┴───┴───┘
         ↑                               ↑
     New Data                        Consumer
```

*   Fixed size, circular.
*   Zero allocation during runtime.
*   SPSC (Single Producer Single Consumer) is lock-free.

### 7. Overall Architecture

#### 7.1 Core Services

| Service | Responsibility | State |
|---------|----------------|-------|
| **Gateway** | Receive Requests | Stateless |
| **Pre-Check** | Read-only Balance Check | Stateless |
| **UBSCore** | Balance Ops + Order WAL | Stateful (Balance) |
| **ME** | Matching, Generate Trades | Stateful (OrderBook) |
| **Settlement** | Persist Events | Stateless |

#### 7.2 UBSCore Service (User Balance Core)

**Single Entry Point for ALL Balance Operations**.

**Why UBSCore?**
*   **Atomic**: Single thread = No Double Spend.
*   **Audit**: Complete trace of all changes.
*   **Recovery**: Single WAL restores state.

**Pipeline Role**:
1.  **Write Order WAL** (Persist)
2.  **Lock Balance**
    *   Success → Forward to ME
    *   Fail → Rejected
3.  **Handle Trade Events** (Settlement)
    *   Update buyer/seller balances.

#### 7.3 Matching Engine (ME)

**ME is Pure Matching. It ignores Balances.**

*   Does: Maintain OrderBook, Match by Price/Time, Generate Trade Events.
*   Does NOT: Check balance, lock funds, persist data.

**Trade Event Drive Balance Update**:
`TradeEvent` contains `{price, qty, user_ids}` → sufficient to calculate balance changes.

#### 7.4 Settlement Service

**Settlement Persists, does not modify Balances.**

*   Persist Trade Events, Order Events.
*   Write Audit Log (Ledger).

#### 7.5 Architecture Diagram

```
┌──────────────────────────────────────────────────────────────────────────────────┐
│                         0xInfinity HFT Architecture                               │
├──────────────────────────────────────────────────────────────────────────────────┤
│   Client Orders                                                                   │
│        │                                                                          │
│        ▼                                                                          │
│   ┌──────────────┐                                                                │
│   │   Gateway    │                                                                │
│   └──────┬───────┘                                                                │
│          ▼                                                                        │
│   ┌──────────────┐         query balance                                          │
│   │  Pre-Check   │ ──────────────────────────────▶   UBSCore Service              │
│   └──────┬───────┘                                                                │
│          ▼                                                                        │
│   ┌──────────────┐                                   ┌────────────────────┐       │
│   │ Order Buffer │                                   │  Balance State     │       │
│   └──────┬───────┘                                   │  (RAM, Single Thd) │       │
│          │ Ring Buffer                               └────────────────────┘       │
│          ▼                                                                        │
│   ┌──────────────────────────────────────────┐                                    │
│   │  UBSCore: Order Processing               │       Operations:                  │
│   │  1. Write Order WAL (Persist)            │       - lock / unlock              │
│   │  2. Lock Balance                         │       - spend_frozen               │
│   │     - OK → forward to ME                 │       - deposit                    │
│   │     - Fail → Rejected                    │                                    │
│   └──────────────┬───────────────────────────┘                                    │
│                  │ Ring Buffer (valid orders)                                     │
│                  ▼                                                                │
│   ┌──────────────────────────────────────────┐                                    │
│   │         Matching Engine (ME)             │                                    │
│   │                                          │                                    │
│   │  Pure Matching, Ignore Balance           │                                    │
│   │  Output: Trade Events                    │                                    │
│   └──────────────┬───────────────────────────┘                                    │
│                  │ Ring Buffer (Trade Events)                                     │
│         ┌───────┴────────┐                                                        │
│         ▼                ▼                                                        │
│   ┌───────────┐   ┌─────────────────────────┐                                     │
│   │ Settlement│   │ Balance Update Events   │────▶   Execute Balance Update       │
│   │           │   │ (from Trade Events)     │                                     │
│   │ Persist:  │   └─────────────────────────┘                                     │
│   │ - Trades  │                                                                   │
│   │ - Ledger  │                                                                   │
│   └───────────┘                                                                   │
└───────────────────────────────────────────────────────────────────────────────────┘
```

#### 7.7 Event Sourcing + Pure State Machine

**Order WAL = Single Source of Truth**

```
State(t) = Replay(Order_WAL[0..t])
```

Any state (Balance, OrderBook) can be 100% reconstructed by replaying the Order WAL.

**Pure State Machines**:
*   **UBSCore**: Order Events → Balance Events (Deterministic)
*   **ME**: Valid Orders → Trade Events (Deterministic)

**Recovery Flow**:
1.  Load Checkpoint (Snapshot).
2.  Replay Order WAL from checkpoint.
3.  ME re-matches and generates events.
4.  UBSCore applies balance updates.
5.  System Restored.

### 8. Summary

**Core Decisions**:
*   **Persist First**: WAL ensures recoverability.
*   **Pre-Check**: Filters invalid orders early.
*   **Single Thread + Lock-Free**: Avoids contention, maximizes throughput.
*   **UBSCore**: Centralized, atomic balance management.
*   **Responsibility Segregation**: UBSCore (Money), ME (Match), Settlement (Log).

**Refactoring**:
For the upcoming implementation, we refactored the code structure:
*   `lib.rs`, `main.rs`, `core_types.rs`, `config.rs`
*   `orderbook.rs`, `balance.rs`, `engine.rs`
*   `csv_io.rs`, `ledger.rs`, `perf.rs`

Next: Detailed implementation of UBSCore and Ring Buffer.

<br>
<div align="right"><a href="#-english">↑ Back to Top</a></div>
<br>

---

<div id="-chinese"></div>

## 🇨🇳 中文

> **📦 代码变更**: [查看 Diff](https://github.com/gjwang/zero_x_infinity/compare/v0.7-b-perf-baseline...v0.8-a-trading-pipeline-design)

> **核心目的**：设计完整的交易流水线架构，确保订单持久化、余额一致性和系统可恢复性。

本章解决撮合引擎最关键的设计问题：**服务划分、数据流和原子性保证**。

### 1. 为什么需要持久化？

#### 1.1 问题场景

假设系统在撮合过程中崩溃：

```
用户 A 发送买单 → ME 接收并成交 → 系统崩溃
                                    ↓
                            用户 A 的钱扣了
                            但没有成交记录
                            订单丢失!
```

**没有持久化的后果**：
- **订单丢失**：用户下的单消失了
- **状态不一致**：资金变动了但没有记录
- **无法恢复**：重启后不知道有哪些订单

#### 1.2 解决方案：先持久化，后撮合

```
用户 A 发送买单 → WAL 持久化 → ME 撮合 → 系统崩溃
                    ↓              ↓
               订单已保存      可以重放恢复!
```

### 2. 唯一排序 (Unique Ordering)

#### 2.1 为什么需要唯一排序？

在分布式系统中，多个节点必须对订单顺序达成一致：

| 场景 | 问题 |
|------|------|
| 节点 A 先收到订单 1，再收到订单 2 | |
| 节点 B 先收到订单 2，再收到订单 1 | 顺序不一致！ |

**结果**：两个节点的撮合结果可能不同！

#### 2.2 解决方案：单点排序 + 全局序号

```
所有订单 → Sequencer → 分配全局 sequence_id → 持久化 → 分发到 ME
              ↓
         唯一的到达顺序
```

| 字段 | 说明 |
|------|------|
| `sequence_id` | 单调递增的全局序号 |
| `timestamp` | 精确到纳秒的时间戳 |
| `order_id` | 业务层订单 ID |

### 3. 订单生命周期

#### 3.1 先持久化，后执行

```
┌─────────────────────────────────────────────────────────────────────────┐
│                         订单生命周期                                      │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│   ┌─────────┐    ┌─────────┐    ┌─────────┐    ┌─────────┐             │
│   │ Gateway │───▶│Pre-Check│───▶│   WAL   │───▶│   ME    │             │
│   │(接收订单)│    │(余额校验)│    │ (持久化)│    │ (撮合) │             │
│   └─────────┘    └─────────┘    └─────────┘    └─────────┘             │
│        │              │              │              │                   │
│        ▼              ▼              ▼              ▼                   │
│    接收订单      余额不足?        写入磁盘        执行撮合               │
│                  提前拒绝        分配seq_id      保证执行               │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

#### 3.2 Pre-Check：减少无效订单

Pre-Check 通过查询 **UBSCore** (User Balance Core Service，用户余额核心服务，详见第 7.2 节) 获取余额信息，**只读，无副作用**：

```rust
async fn pre_check(order: Order) -> Result<Order, Reject> {
     // 1. 查询 UBSCore 获取余额 (只读查询)
     let balance = ubscore.query_balance(order.user_id, asset);

     // 2. 计算所需金额
     let required = match order.side {
         Buy  => order.price * order.qty / QTY_UNIT,  // quote
         Sell => order.qty,                            // base
     };

     // 3. 余额检查 (只读，不锁定)
     if balance.avail < required {
         return Err(Reject::InsufficientBalance);
     }

     // 4. 检查通过，放行订单到下一阶段
     Ok(order)
}
// 注意：Pre-Check 不锁定余额！
// 余额可能在 Pre-Check 和 WAL 之间被其他订单消耗
// 这是允许的，WAL 后的 Balance Lock 会处理这种情况
```

**为什么需要 Pre-Check？**

核心流程（WAL 持久化、Balance Lock、撮合）的延迟成本很高。
用户可能提交大量垃圾订单，我们需要**最快速**地预过滤，减少进入核心流程的订单量。

| 不 Pre-Check | 有 Pre-Check |
|--------------|--------------|
| 垃圾订单直接进入核心流程 | 快速过滤大部分无效订单 |
| 核心流程处理无效订单，浪费延迟 | 核心流程只处理可能有效的订单 |
| 系统容易被刷单攻击 | 减少恶意请求的影响 |

**Pre-Check 可以包含多种快速检查**：
- ✅ 余额检查（当前实现）
- 📋 用户状态检查（是否被禁用）
- 📋 订单格式校验
- 📋 频率限制 (Rate Limit)
- 📋 风控规则（未来扩展）

**重要**：Pre-Check 是"尽力而为"的过滤器，不保证 100% 准确。
通过 Pre-Check 的订单，仍可能在 WAL + Balance Lock 阶段被拒绝。

#### 3.3 一旦持久化，必须完整执行

```
订单被持久化后，无论发生什么，都必须有以下其中一个结果：

┌─────────────────────┐
│ 订单已持久化         │
└─────────────────────┘
           │
           ├──▶ 成交 (Filled)
           ├──▶ 部分成交 (PartialFilled)
           ├──▶ 挂单中 (New)
           ├──▶ 用户取消 (Cancelled)
           ├──▶ 系统过期 (Expired)
           └──▶ 余额不足被拒绝 (Rejected)  ← 也是合法的终态！

❌ 绝对不能：订单消失 / 状态未知
```

### 4. WAL：为什么是最佳选择？

#### 4.1 什么是 WAL (Write-Ahead Log)?

WAL 是一种**追加写** (Append-Only) 的日志结构：

```
┌─────────────────────────────────────────────────────────────────┐
│                          WAL File                               │
├─────────────────────────────────────────────────────────────────┤
│  Entry 1  │  Entry 2  │  Entry 3  │  Entry 4  │  ...  │ ← 追加  │
│ (seq=1)   │ (seq=2)   │ (seq=3)   │ (seq=4)   │       │         │
└─────────────────────────────────────────────────────────────────┘
                                                          ↑
                                                     只追加，不修改
```

#### 4.2 为什么 WAL 是 HFT 最佳实践？

| 持久化方式 | 写入模式 | 延迟 | 吞吐量 | HFT 适用性 |
|-----------|----------|------|--------|-----------|
| 数据库 (MySQL/Postgres) | 随机写 + 事务 | ~1-10ms | ~1K ops/s | ❌ 太慢 |
| KV 存储 (Redis/RocksDB) | 随机写 | ~0.1-1ms | ~10K ops/s | ⚠️ 一般 |
| **WAL 追加写** | **顺序写** | **~1-10µs** | **~1M ops/s** | ✅ **最佳** |

**为什么 WAL 这么快？**

1.  **顺序写 vs 随机写**：
    *   机械硬盘不用寻道。
    *   SSD 减少写放大。
    *   结果：快 10-100 倍。
2.  **无事务开销**：
    *   无需锁、redo log、binlog 等数据库复杂机制。
3.  **批量刷盘 (Group Commit)**：
    *   合并多次写入一次 fsync。

### 5. 单线程 + Lock-Free 架构

#### 5.1 为什么选择单线程？

大多数人直觉认为：并发 = 快。但在 HFT 领域，**单线程往往更快**：

| 多线程 | 单线程 |
|--------|--------|
| 需要锁保护共享状态 | 无锁，无竞争 |
| 缓存失效 (cache invalidation) | 缓存友好 |
| 上下文切换开销 | 无切换开销 |
| 顺序难以保证 | 天然有序 |
| 复杂的同步逻辑 | 代码简单直观 |

#### 5.2 Mechanical Sympathy

**CPU Cache Hierarchy**:
*   L1 Cache: ~1ns
*   L2 Cache: ~4ns
*   RAM: ~100ns

单线程优势：数据始终在 L1/L2 缓存中（热数据），无 cache line 争用。

#### 5.3 LMAX Disruptor 模式

这种单线程 + Ring Buffer 的架构源自 **LMAX Exchange**（伦敦多资产交易所），号称能在单线程上处理 **600 万订单/秒**：

1.  **Single Writer** (避免写竞争)
2.  **Pre-allocated Memory** (避免 GC/malloc)
3.  **Cache Padding** (避免 false sharing)
4.  **Batch Consumption**

### 6. Ring Buffer：服务间通信

#### 6.1 为什么使用 Ring Buffer？

服务间通信的选择：

| 方式 | 延迟 | 吞吐量 |
|------|------|--------|
| HTTP/gRPC | ~1ms | ~10K/s |
| Kafka | ~1-10ms | ~1M/s |
| **Shared Memory Ring Buffer** | **~100ns** | **~10M/s** |

#### 6.2 Ring Buffer 原理

```
      write_idx                       read_idx
          ↓                               ↓
   ┌───┬───┬───┬───┬───┬───┬───┬───┬───┬───┐
   │ 8 │ 9 │ 10│ 11│ 12│ 13│ 14│ 15│ 0 │ 1 │ ...
   └───┴───┴───┴───┴───┴───┴───┴───┴───┴───┘
         ↑                               ↑
     新数据写入                        消费者读取
```

*   固定大小，循环使用
*   无需动态分配
*   Single Producer, Single Consumer ({SPSC) 可完全无锁

### 7. 整体架构

#### 7.1 核心服务

| 服务 | 职责 | 状态 |
|------|------|------|
| **Gateway** | 接收客户端请求 | 无状态 |
| **Pre-Check** | 只读查询余额，过滤无效订单 | 无状态 |
| **UBSCore** | 所有余额操作 + Order WAL | 有状态 (余额) |
| **ME** | 纯撮合，生成 Trade Events | 有状态 (OrderBook) |
| **Settlement** | 持久化 events，未来写 DB | 无状态 |

#### 7.2 UBSCore Service (User Balance Core)

**UBSCore 是所有账户余额操作的唯一入口**，单线程执行保证原子性。

**应用场景**：
1.  **Write Order WAL** (持久化)
2.  **Lock Balance** (锁定)
3.  **Handle Trade Events** (成交后结算)

#### 7.3 Matching Engine (ME)

**ME 是纯撮合引擎，不关心余额**。

*   负责：维护 OrderBook，撮合，生成 Trade Events。
*   不负责：检查余额，锁定资金，持久化。

**Trade Event 驱动余额更新**：
`TradeEvent` 包含 {price, qty, user_ids}，足够计算出余额变化。

#### 7.4 Settlement Service

**Settlement 负责持久化，不修改余额**。

*   持久化 Trade Events，Order Events。
*   写审计日志 (Ledger)。

#### 7.5 完整架构图

```
┌──────────────────────────────────────────────────────────────────────────────────┐
│                         0xInfinity HFT Architecture                               │
├──────────────────────────────────────────────────────────────────────────────────┤
│   Client Orders                                                                   │
│        │                                                                          │
│        ▼                                                                          │
│   ┌──────────────┐                                                                │
│   │   Gateway    │                                                                │
│   └──────┬───────┘                                                                │
│          ▼                                                                        │
│   ┌──────────────┐         query balance                                          │
│   │  Pre-Check   │ ──────────────────────────────▶   UBSCore Service              │
│   └──────┬───────┘                                                                │
│          ▼                                                                        │
│   ┌──────────────┐                                   ┌────────────────────┐       │
│   │ Order Buffer │                                   │  Balance State     │       │
│   └──────┬───────┘                                   │  (RAM, Single Thd) │       │
│          │ Ring Buffer                               └────────────────────┘       │
│          ▼                                                                        │
│   ┌──────────────────────────────────────────┐                                    │
│   │  UBSCore: Order Processing               │       Operations:                  │
│   │  1. Write Order WAL (持久化)              │       - lock / unlock              │
│   │  2. Lock Balance                         │       - spend_frozen               │
│   │     - OK → forward to ME                 │       - deposit                    │
│   │     - Fail → Rejected                    │                                    │
│   └──────────────┬───────────────────────────┘                                    │
│                  │ Ring Buffer (valid orders)                                     │
│                  ▼                                                                │
│   ┌──────────────────────────────────────────┐                                    │
│   │         Matching Engine (ME)             │                                    │
│   │                                          │                                    │
│   │  纯撮合，不关心 Balance                   │                                    │
│   │  输出: Trade Events                      │                                    │
│   └──────────────┬───────────────────────────┘                                    │
│                  │ Ring Buffer (Trade Events)                                     │
│         ┌───────┴────────┐                                                        │
│         ▼                ▼                                                        │
│   ┌───────────┐   ┌─────────────────────────┐                                     │
│   │ Settlement│   │ Balance Update Events   │────▶   执行余额更新                 │
│   │           │   │ (from Trade Events)     │                                     │
│   │ 持久化:    │   └─────────────────────────┘                                     │
│   │ - Trades  │                                                                   │
│   │ - Ledger  │                                                                   │
│   └───────────┘                                                                   │
└───────────────────────────────────────────────────────────────────────────────────┘
```

#### 7.7 Event Sourcing + Pure State Machine

**Order WAL = Single Source of Truth**

```
State(t) = Replay(Order_WAL[0..t])
```

只要有 Order WAL，就能恢复整个系统状态！

**Pure State Machines**:
*   **UBSCore**: Order Events → Balance Events (确定性)
*   **ME**: Valid Orders → Trade Events (确定性)

**恢复流程**:
1.  加载最近快照 Checkpoint。
2.  重放 Order WAL。
3.  系统恢复到崩溃前状态。

### 8. Summary

**核心设计**：
*   **先持久化**：WAL 保证可恢复性。
*   **Pre-Check**：提前过滤无效订单。
*   **单线程 + 无锁**：避免锁竞争，最大化吞吐。
*   **UBSCore**：集中式、原子的余额管理。
*   **职责分离**：UBSCore (钱)，ME (撮合)，Settlement (日志)。

**代码重构**：
为后续章节准备，我们重构了 `src` 目录结构，模块化了 `main.rs`, `core_types.rs` 等。

下一步：实现 UBSCore 和 Ring Buffer。
