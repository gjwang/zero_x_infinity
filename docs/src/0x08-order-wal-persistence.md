# 0x08 订单WAL持久化 (Order WAL Persistence)

> **核心目的**：确保每个订单在进入撮合引擎前被持久化，保证订单的完整生命周期。

本章解决撮合引擎最关键的问题：**订单的持久化和确定性排序**。

---

## 1. 为什么需要持久化？

### 1.1 问题场景

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

### 1.2 解决方案：先持久化，后撮合

```
用户 A 发送买单 → WAL 持久化 → ME 撮合 → 系统崩溃
                    ↓              ↓
               订单已保存      可以重放恢复!
```

---

## 2. 唯一排序 (Unique Ordering)

### 2.1 为什么需要唯一排序？

在分布式系统中，多个节点必须对订单顺序达成一致：

| 场景 | 问题 |
|------|------|
| 节点 A 先收到订单 1，再收到订单 2 | |
| 节点 B 先收到订单 2，再收到订单 1 | 顺序不一致！ |

**结果**：两个节点的撮合结果可能不同！

### 2.2 解决方案：单点排序 + 全局序号

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

---

## 3. 订单生命周期

### 3.1 先持久化，后执行

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

### 3.2 Pre-Check：减少无效订单

Pre-Check 通过查询 UBSCore 获取余额信息，**只读，无副作用**：

```
┌───────────────────────────────────────────────────────────────────┐
│                     Pre-Check 流程 (伪代码)                        │
├───────────────────────────────────────────────────────────────────┤
│                                                                   │
│  async fn pre_check(order: Order) -> Result<Order, Reject> {      │
│      // 1. 查询 UBSCore 获取余额 (只读查询)                        │
│      let balance = ubscore.query_balance(order.user_id, asset);   │
│                                                                   │
│      // 2. 计算所需金额                                            │
│      let required = match order.side {                            │
│          Buy  => order.price * order.qty / QTY_UNIT,  // quote    │
│          Sell => order.qty,                            // base    │
│      };                                                           │
│                                                                   │
│      // 3. 余额检查 (只读，不锁定)                                  │
│      if balance.avail < required {                                │
│          return Err(Reject::InsufficientBalance);                 │
│      }                                                            │
│                                                                   │
│      // 4. 检查通过，放行订单到下一阶段                             │
│      Ok(order)                                                    │
│  }                                                                │
│                                                                   │
│  注意：Pre-Check 不锁定余额！                                       │
│  余额可能在 Pre-Check 和 WAL 之间被其他订单消耗                     │
│  这是允许的，WAL 后的 Balance Lock 会处理这种情况                   │
│                                                                   │
└───────────────────────────────────────────────────────────────────┘
```

**为什么需要 Pre-Check？**

| 不 Pre-Check | 有 Pre-Check |
|--------------|--------------|
| 无效订单也被持久化 | 无效订单提前拒绝 |
| 浪费 WAL 空间 | 节省存储 |
| 增加系统负载 | 减少无效订单进入核心流程 |

**重要**：Pre-Check 是"尽力而为"的过滤器，不保证 100% 准确。
通过 Pre-Check 的订单，仍可能在 WAL + Balance Lock 阶段被拒绝。

### 3.3 一旦持久化，必须完整执行

```
订单被持久化后，无论发生什么，都必须有以下其中一个结果：

┌─────────────────────┐
│ 订单已持久化         │
└─────────────────────┘
           │
           ├──▶ 成交 (Filled)
           ├──▶ 部分成交 (PartialFilled)
           ├──▶ 挂单中 (New/Open)
           ├──▶ 用户取消 (Cancelled)
           ├──▶ 系统过期 (Expired)
           └──▶ 余额不足被拒绝 (Rejected)  ← 也是合法的终态！

❌ 绝对不能：订单消失 / 状态未知
```

### 3.4 ⚠️ 重要：Pre-Check 无副作用，Balance Lock 在 WAL 之后

**Pre-Check 只是 "软过滤器"，不做任何状态修改：**

```rust
// Pre-Check: 只检查，不修改！
fn pre_check_order(order: &Order, accounts: &AccountManager) -> Result<(), RejectReason> {
    let user = accounts.get(order.user_id)?;  // 只读
    
    match order.side {
        Side::Buy => {
            let cost = order.price * order.qty / QTY_UNIT;
            if user.avail(QUOTE_ASSET) < cost {  // 只读检查
                return Err(RejectReason::InsufficientBalance);
            }
        }
        Side::Sell => {
            if user.avail(BASE_ASSET) < order.qty {  // 只读检查
                return Err(RejectReason::InsufficientBalance);
            }
        }
    }
    Ok(())
}
```

**为什么 Pre-Check 不锁定余额？**

| 如果 Pre-Check 锁定余额 | 问题 |
|-------------------------|------|
| 订单 → Lock → 系统崩溃 | 资金被锁，但订单没记录！ |
| 订单 → Lock → WAL 失败 | 资金被锁，但无持久化记录 |

**正确流程：WAL 后才锁定余额**

```
Pre-Check → WAL持久化 → Balance Lock → ME → Settlement
    │            │            │
  无副作用     持久化       原子锁定
  (可能漏)    (不可逆)    (单线程保证)
```

**Balance Lock 在 WAL 之后的执行：**

```rust
// WAL 写入后，执行余额锁定
fn lock_balance_and_process(order: &Order, accounts: &mut AccountManager) -> OrderResult {
    // 此时订单已在 WAL 中，必须有终态
    
    let lock_result = match order.side {
        Side::Buy => {
            let cost = order.price * order.qty / QTY_UNIT;
            accounts.lock(order.user_id, QUOTE_ASSET, cost)
        }
        Side::Sell => {
            accounts.lock(order.user_id, BASE_ASSET, order.qty)
        }
    };
    
    match lock_result {
        Ok(_) => {
            // 锁定成功，进入 ME 撮合
            matching_engine.process(order)
        }
        Err(_) => {
            // 锁定失败，记录为 Rejected
            // 订单仍然完成了它的生命周期！
            OrderResult::rejected(order.id, RejectReason::InsufficientBalance)
        }
    }
}
```

**为什么单线程就能保证无双花？**

```
单线程执行顺序：
  Order A (lock 100 USDT) → 成功，余额 1000 -> 900
  Order B (lock 200 USDT) → 成功，余额 900 -> 700
  Order C (lock 800 USDT) → 失败！余额不足，Rejected

因为是单线程：
  - 不可能同时处理 A 和 B
  - 不需要锁
  - 余额更新是即时可见的
  - 天然原子性
```

---

## 4. WAL：为什么是最佳选择？

### 4.1 什么是 WAL (Write-Ahead Log)?

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

### 4.2 为什么 WAL 是 HFT 最佳实践？

| 持久化方式 | 写入模式 | 延迟 | 吞吐量 | HFT 适用性 |
|-----------|----------|------|--------|-----------|
| 数据库 (MySQL/Postgres) | 随机写 + 事务 | ~1-10ms | ~1K ops/s | ❌ 太慢 |
| KV 存储 (Redis/RocksDB) | 随机写 | ~0.1-1ms | ~10K ops/s | ⚠️ 一般 |
| **WAL 追加写** | **顺序写** | **~1-10µs** | **~1M ops/s** | ✅ **最佳** |

**为什么 WAL 这么快？**

#### 4.2.1 顺序写 vs 随机写

```
随机写 (数据库):
┌─────┐     ┌─────┐     ┌─────┐
│ 写1 │     │ 写2 │     │ 写3 │
└──┬──┘     └──┬──┘     └──┬──┘
   │           │           │
   ▼           ▼           ▼
 磁盘位置 A  磁盘位置 X  磁盘位置 M   ← 磁头需要频繁移动 (寻道)

顺序写 (WAL):
┌─────┬─────┬─────┐
│ 写1 │ 写2 │ 写3 │ ← 追加到文件末尾，磁头不移动
└─────┴─────┴─────┘
```

即使是 SSD，顺序写也比随机写快 **10-100 倍**。

#### 4.2.2 无事务开销

```
数据库写入:
1. 开启事务
2. 获取锁
3. 写 redo log
4. 写数据页
5. 写 binlog
6. 提交事务，释放锁
→ 多次磁盘操作，多次同步

WAL 写入:
1. 序列化数据
2. 追加写入
3. (可选) fsync
→ 最少一次磁盘操作
```

#### 4.2.3 批量刷盘 (Group Commit)

```rust
/// 批量提交 WAL
impl WalWriter {
    /// 写入但不立即刷盘
    pub fn append(&mut self, entry: &[u8]) -> u64 {
        self.buffer.extend_from_slice(entry);
        self.pending_count += 1;
        self.next_seq()
    }
    
    /// 批量刷盘（每 N 个订单或每 T 毫秒）
    pub fn flush(&mut self) -> io::Result<()> {
        self.file.write_all(&self.buffer)?;
        self.file.sync_data()?;  // fsync
        self.buffer.clear();
        Ok(())
    }
}
```

**Group Commit 效果**：

| 刷盘策略 | 延迟 | 吞吐量 | 数据安全 |
|----------|------|--------|----------|
| 每条 fsync | ~50µs | ~20K/s | 最高 |
| 每 100 条 fsync | ~5µs (均摊) | ~200K/s | 高 |
| 每 1ms fsync | ~1µs (均摊) | ~1M/s | 中 |

---

## 5. 单线程 + Lock-Free 架构

### 5.1 为什么选择单线程？

大多数人直觉认为：并发 = 快。但在 HFT 领域，**单线程往往更快**：

| 多线程 | 单线程 |
|--------|--------|
| 需要锁保护共享状态 | 无锁，无竞争 |
| 缓存失效 (cache invalidation) | 缓存友好 |
| 上下文切换开销 | 无切换开销 |
| 顺序难以保证 | 天然有序 |
| 复杂的同步逻辑 | 代码简单直观 |

### 5.2 Mechanical Sympathy

```
┌─────────────────────────────────────────────────────────────────┐
│                    CPU Cache Hierarchy                          │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│   ┌─────────┐                                                   │
│   │   CPU   │  L1 Cache: ~1ns (32KB)                           │
│   │  Core 0 │  L2 Cache: ~4ns (256KB)                          │
│   └────┬────┘  L3 Cache: ~12ns (shared, MB级)                  │
│        │                                                        │
│        ▼                                                        │
│   ┌─────────┐                                                   │
│   │   RAM   │  ~100ns                                          │
│   └─────────┘                                                   │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘

单线程优势：
- 数据始终在 L1/L2 缓存中（热数据）
- 无 cache line 争用
- 无 false sharing
```

### 5.3 LMAX Disruptor 模式

这种单线程 + Ring Buffer 的架构源自 **LMAX Exchange**（伦敦多资产交易所），号称能在单线程上处理 **600 万订单/秒**：

```
┌─────────────────────────────────────────────────────────────────┐
│                    LMAX Disruptor Architecture                  │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│   Publisher ───▶ Ring Buffer ───▶ Consumer                     │
│   (单线程写)      (无锁队列)       (单线程读)                    │
│                                                                 │
│   关键特性：                                                     │
│   1. 单一 Writer（避免写竞争）                                   │
│   2. 预分配内存（避免 GC/malloc）                                │
│   3. 缓存行填充（避免 false sharing）                           │
│   4. 批量消费（减少同步点）                                      │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

---

## 6. Ring Buffer：服务间通信

### 6.1 为什么使用 Ring Buffer？

服务间通信的选择：

| 方式 | 延迟 | 吞吐量 | 复杂度 |
|------|------|--------|--------|
| HTTP/gRPC | ~1ms | ~10K/s | 低 |
| Kafka | ~1-10ms | ~1M/s | 中 |
| Socket/ZMQ | ~100µs | ~100K/s | 中 |
| **Shared Memory Ring Buffer** | **~100ns** | **~10M/s** | 高 |

### 6.2 Ring Buffer 原理

```
┌─────────────────────────────────────────────────────────────────┐
│                        Ring Buffer                              │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│      write_idx                       read_idx                   │
│          ↓                               ↓                      │
│   ┌───┬───┬───┬───┬───┬───┬───┬───┬───┬───┬───┬───┐            │
│   │ 8 │ 9 │ 10│ 11│ 12│ 13│ 14│ 15│ 0 │ 1 │ 2 │ 3 │ ...        │
│   └───┴───┴───┴───┴───┴───┴───┴───┴───┴───┴───┴───┘            │
│         ↑                               ↑                       │
│     新数据写入                        消费者读取                  │
│                                                                 │
│   特性：                                                         │
│   - 固定大小，循环使用                                           │
│   - 无需动态分配                                                 │
│   - Single Producer, Single Consumer (SPSC) 可完全无锁          │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### 6.3 为什么 Ring Buffer 这么快？

```rust
/// SPSC Ring Buffer 核心实现
pub struct RingBuffer<T, const N: usize> {
    buffer: [MaybeUninit<T>; N],
    write_idx: AtomicUsize,  // 生产者独占
    read_idx: AtomicUsize,   // 消费者独占
}

impl<T, const N: usize> RingBuffer<T, N> {
    /// 生产者写入（无锁）
    pub fn push(&self, item: T) -> bool {
        let write = self.write_idx.load(Ordering::Relaxed);
        let read = self.read_idx.load(Ordering::Acquire);
        
        if (write + 1) % N == read {
            return false;  // 满了
        }
        
        unsafe {
            self.buffer[write].as_mut_ptr().write(item);
        }
        
        self.write_idx.store((write + 1) % N, Ordering::Release);
        true
    }
    
    /// 消费者读取（无锁）
    pub fn pop(&self) -> Option<T> {
        let read = self.read_idx.load(Ordering::Relaxed);
        let write = self.write_idx.load(Ordering::Acquire);
        
        if read == write {
            return None;  // 空的
        }
        
        let item = unsafe { self.buffer[read].as_ptr().read() };
        
        self.read_idx.store((read + 1) % N, Ordering::Release);
        Some(item)
    }
}
```

**关键点**：
- **无锁**：使用 Atomic 操作代替互斥锁
- **无分配**：预分配固定大小的数组
- **缓存友好**：连续内存布局
- **批量操作**：可以一次读取多个条目

---

## 7. 整体架构

### 7.1 核心服务

系统由以下核心服务组成：

| 服务 | 职责 | 状态 |
|------|------|------|
| **Gateway** | 接收客户端请求 | 无状态 |
| **Pre-Check** | 只读查询余额，过滤无效订单 | 无状态 |
| **UBSCore** | 所有余额操作 + Order WAL | 有状态 (余额) |
| **ME** | 纯撮合，生成 Trade Events | 有状态 (OrderBook) |
| **Settlement** | 持久化 events，未来写 DB | 无状态 |

### 7.2 UBSCore Service (User Balance Core)

**UBSCore 是所有账户余额操作的唯一入口**，单线程执行保证原子性。

#### 7.2.1 为什么需要 UBSCore？

| 分散处理余额 | 集中到 UBSCore |
|-------------|----------------|
| 多处修改余额，需要分布式锁 | 唯一入口，单线程无锁 |
| 双花风险高 | 天然原子，无双花 |
| 难以审计 | 所有变更可追踪 |
| 恢复困难 | 单一 WAL 可完整恢复 |

#### 7.2.2 UBSCore 提供的操作

```rust
// 余额查询 (只读)
fn query_balance(user_id: UserId, asset_id: AssetId) -> Balance;

// 余额操作 (修改)
fn lock(user_id: UserId, asset_id: AssetId, amount: u64) -> Result<()>;
fn unlock(user_id: UserId, asset_id: AssetId, amount: u64) -> Result<()>;
fn spend_frozen(user_id: UserId, asset_id: AssetId, amount: u64) -> Result<()>;
fn deposit(user_id: UserId, asset_id: AssetId, amount: u64) -> Result<()>;
```

#### 7.2.3 UBSCore 在订单流程中的角色

```
订单到达 UBSCore 后：
1. Write Order WAL (持久化订单)
2. Lock Balance (锁定资金)
   - 成功 → 转发到 ME
   - 失败 → 标记为 Rejected，不进入 ME
3. 收到 Trade Events 后执行 Settlement
   - Buyer: spend_frozen(quote), deposit(base)
   - Seller: spend_frozen(base), deposit(quote)
```

### 7.3 Matching Engine (ME)

**ME 是纯撮合引擎，不关心余额**。

| ME 做的事 | ME 不做的事 |
|----------|------------|
| 维护 OrderBook | 检查余额 |
| 价格-时间优先撮合 | 锁定/解锁余额 |
| 生成 Trade Events | 更新余额 |
|  | 持久化任何数据 |

**Trade Event 包含足够信息生成 Balance Update**：

```rust
struct TradeEvent {
    trade_id: TradeId,
    buyer_order_id: OrderId,
    seller_order_id: OrderId,
    buyer_user_id: UserId,
    seller_user_id: UserId,
    price: u64,
    qty: u64,
    // 可以从这些信息计算出：
    // - buyer 需要 spend_frozen(quote, price * qty)
    // - buyer 需要 deposit(base, qty)
    // - seller 需要 spend_frozen(base, qty)
    // - seller 需要 deposit(quote, price * qty)
}
```

### 7.4 Settlement Service

**Settlement 负责持久化，不修改余额**。

| Settlement 做的事 | Settlement 不做的事 |
|------------------|-------------------|
| 持久化 Trade Events | 更新余额 (由 UBSCore 做) |
| 持久化 Order Events | 撮合订单 |
| 未来写入 DB 供查询 | |
| 生成审计日志 | |

### 7.5 完整架构图

```
┌──────────────────────────────────────────────────────────────────────────────────┐
│                         0xInfinity HFT Architecture                               │
├──────────────────────────────────────────────────────────────────────────────────┤
│                                                                                   │
│   Client Orders                                                                   │
│        │                                                                          │
│        ▼                                                                          │
│   ┌──────────────┐                                                                │
│   │   Gateway    │  ← 多线程接收网络请求                                           │
│   └──────┬───────┘                                                                │
│          │                                                                        │
│          ▼                                                                        │
│   ┌──────────────┐         query balance          ┌────────────────────────────┐ │
│   │  Pre-Check   │ ──────────────────────────────▶│                            │ │
│   │  (只读查询)   │◀────────────────────────────── │                            │ │
│   └──────┬───────┘         return balance         │                            │ │
│          │                                        │                            │ │
│          │ 过滤明显无效订单                        │                            │ │
│          ▼                                        │                            │ │
│   ┌──────────────┐                                │      UBSCore Service       │ │
│   │ Order Buffer │                                │   (User Balance Core)      │ │
│   └──────┬───────┘                                │                            │ │
│          │ Ring Buffer                            │   ┌────────────────────┐   │ │
│          ▼                                        │   │  Balance State     │   │ │
│   ┌──────────────────────────────────────────┐    │   │  (内存, 单线程)    │   │ │
│   │  UBSCore: Order Processing               │    │   └────────────────────┘   │ │
│   │  1. Write Order WAL (持久化)              │    │                            │ │
│   │  2. Lock Balance                         │    │   Operations:              │ │
│   │     - OK → forward to ME                 │    │   - lock / unlock          │ │
│   │     - Fail → Rejected (记录状态)         │    │   - spend_frozen           │ │
│   └──────────────┬───────────────────────────┘    │   - deposit                │ │
│                  │                                │                            │ │
│                  │ Ring Buffer (valid orders)     │                            │ │
│                  ▼                                │                            │ │
│   ┌──────────────────────────────────────────┐    │                            │ │
│   │         Matching Engine (ME)             │    │                            │ │
│   │                                          │    │                            │ │
│   │  纯撮合，不关心 Balance                   │    │                            │ │
│   │  输出: Trade Events                      │    │                            │ │
│   │                                          │    │                            │ │
│   └──────────────┬───────────────────────────┘    │                            │ │
│                  │                                │                            │ │
│                  │ Ring Buffer (Trade Events)     │                            │ │
│                  │                                │                            │ │
│         ┌───────┴────────┐                        │                            │ │
│         │                │                        │                            │ │
│         ▼                ▼                        │                            │ │
│   ┌───────────┐   ┌─────────────────────────┐     │                            │ │
│   │ Settlement│   │ Balance Update Events   │────▶│  执行余额更新:             │ │
│   │           │   │ (from Trade Events)     │     │  - Buyer: -quote, +base    │ │
│   │ 持久化:    │   └─────────────────────────┘     │  - Seller: -base, +quote   │ │
│   │ - Trades  │                                   │                            │ │
│   │ - Orders  │                                   └────────────────────────────┘ │
│   │ - Ledger  │                                                                   │
│   │           │                                                                   │
│   │ 未来 → DB │                                                                   │
│   └───────────┘                                                                   │
│                                                                                   │
└──────────────────────────────────────────────────────────────────────────────────┘
```

### 7.6 数据流详解

```
┌─────────────────────────────────────────────────────────────────────────────────┐
│                              订单处理流程                                         │
├─────────────────────────────────────────────────────────────────────────────────┤
│                                                                                  │
│  [1] Pre-Check                                                                   │
│      │                                                                           │
│      ├── Query UBSCore: 获取用户余额 (只读)                                       │
│      ├── 检查: 余额是否足够？                                                     │
│      └── 过滤: 明显无效订单不进入系统                                             │
│                                                                                  │
│  [2] Order Buffer → Ring Buffer → UBSCore                                       │
│      │                                                                           │
│      ├── UBSCore 收到订单                                                        │
│      ├── Step 1: Write Order WAL (先持久化)                                      │
│      ├── Step 2: Lock Balance                                                   │
│      │     ├── 成功 → 订单有效，转发到 ME                                        │
│      │     └── 失败 → 订单 Rejected (仍有记录)                                   │
│      └── Step 3: Forward to ME (via Ring Buffer)                                │
│                                                                                  │
│  [3] ME                                                                          │
│      │                                                                           │
│      ├── 收到有效订单                                                            │
│      ├── 撮合: 价格-时间优先                                                      │
│      └── 输出: Trade Events                                                      │
│                                                                                  │
│  [4] Trade Events → Fan-out                                                      │
│      │                                                                           │
│      ├── → UBSCore: Balance Update                                              │
│      │     ├── Buyer: spend_frozen(quote, cost), deposit(base, qty)             │
│      │     └── Seller: spend_frozen(base, qty), deposit(quote, cost)            │
│      │                                                                           │
│      └── → Settlement: Persist                                                   │
│            ├── 持久化 Trade Events                                               │
│            ├── 持久化 Order Events (状态变更)                                    │
│            └── 写审计日志 (Ledger)                                               │
│                                                                                  │
└─────────────────────────────────────────────────────────────────────────────────┘
```

### 7.7 为什么这样设计？

| 设计决策 | 原因 |
|---------|------|
| UBSCore 单线程 | 余额操作天然原子，无双花 |
| ME 不关心余额 | 职责分离，ME 只做撮合 |
| Trade Events 驱动 | Balance update 从 events 生成，可重放 |
| Settlement 只持久化 | 与余额操作解耦，方便扩展 |
| Pre-Check 只读 | 无副作用，可水平扩展 |

### 7.8 恢复流程

```
系统重启后：

1. UBSCore 从 Snapshot 恢复 Balance State
   └── 最近的 checkpoint

2. UBSCore 重放 Order WAL
   └── 从 checkpoint 之后的订单开始
   └── 重新执行 Lock Balance + Forward to ME

3. ME 重新撮合
   └── 生成 Trade Events

4. Trade Events → Balance Update
   └── UBSCore 执行余额更新

5. 系统恢复到崩溃前状态
```

---

## 8. Summary

本章核心设计：

| 设计点 | 解决的问题 | 方案 |
|--------|------------|------|
| 订单丢失 | 系统崩溃后无法恢复 | 先持久化，后撮合 |
| 顺序不一致 | 分布式节点顺序不同 | 单点 Sequencer + 全局序号 |
| 无效订单 | 浪费持久化空间 | Pre-Check 余额校验 (只读) |
| 持久化性能 | 数据库太慢 | WAL 追加写 + Group Commit |
| 锁竞争 | 多线程同步开销 | 单线程 + Lock-Free |
| 服务间通信 | 网络调用延迟高 | Shared Memory Ring Buffer |
| **双花风险** | 并发修改余额 | **UBSCore 单线程处理所有余额操作** |
| **职责不清** | ME 既撮合又管余额 | **ME 纯撮合，UBSCore 管余额** |

**核心服务职责**：

| 服务 | 职责 |
|------|------|
| **Pre-Check** | 只读查询 UBSCore，过滤无效订单 |
| **UBSCore** | 所有余额操作 + Order WAL (单线程) |
| **ME** | 纯撮合，生成 Trade Events |
| **Settlement** | 持久化 events，写 DB |

**核心理念**：

> 在 HFT 领域，**简单就是快**。单线程 + 顺序写 + 无锁设计，
> 比复杂的多线程 + 随机写 + 加锁设计，往往快 10-100 倍。
>
> **职责分离**：UBSCore 管余额，ME 管撮合，Settlement 管持久化。
> 每个服务单线程，自然原子，无需分布式锁。

---

## 9. 下一步

1. **实现 UBSCore Service**：`src/ubscore.rs`
   - Balance state 管理
   - Order WAL 写入
   - Balance lock/unlock/spend_frozen/deposit
   
2. **实现 Ring Buffer**：`src/ringbuffer.rs`
   - 服务间无锁通信
   
3. **重构 ME**：`src/engine.rs`
   - 移除所有 balance 相关代码
   - 只负责撮合，输出 Trade Events
   
4. **实现 Settlement**：`src/settlement.rs`
   - 持久化 Trade/Order Events
   - 写审计日志
   
5. **集成测试**：验证完整流程

