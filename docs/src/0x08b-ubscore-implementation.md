# Chapter 0x08b: UBSCore Implementation

> 从设计到实现：构建安全第一的余额核心服务

---

## 概述

在上一章（0x08a）中，我们设计了完整的 HFT 交易流水线架构。现在，是时候实现核心组件了。本章我们将构建：

1. **Ring Buffer** - 服务间无锁通信
2. **Write-Ahead Log (WAL)** - 订单持久化
3. **UBSCore Service** - 余额核心服务

## 1. 技术选型：安全第一

在金融系统中，**成熟稳定**比极致性能更重要。

### 1.1 Ring Buffer 选型

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

## 2. Write-Ahead Log (WAL)

WAL 是系统的**唯一事实来源 (Single Source of Truth)**。

### 2.1 设计原则

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

### 2.2 Group Commit 策略

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

impl Default for WalConfig {
    fn default() -> Self {
        Self {
            path: "wal/orders.wal".to_string(),
            flush_interval_entries: 100,
            sync_on_flush: true,
        }
    }
}
```

### 2.3 WAL 条目格式

当前使用 CSV 格式（开发阶段可读性好）：

```
seq_id,timestamp_ns,order_id,user_id,price,qty,side,order_type
1,1702742400000000000,1001,100,85000000000,100000000,Buy,Limit
```

生产环境可切换为二进制格式（54 bytes/entry）以提升性能。

## 3. UBSCore Service

UBSCore 是所有余额操作的**唯一入口**。

### 3.1 职责

1. **Balance State Management** - 内存中的余额状态
2. **Order WAL Writing** - 持久化订单
3. **Balance Operations** - lock/unlock/spend_frozen/deposit

### 3.2 核心结构

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

### 3.3 订单处理流程

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

### 3.4 成交结算

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

## 4. 消息类型

服务间通过明确定义的消息类型通信：

```rust
// Gateway → UBSCore
pub struct OrderMessage {
    pub seq_id: SeqNum,
    pub order: Order,
    pub timestamp_ns: u64,
}

// UBSCore → ME
pub struct ValidOrder {
    pub seq_id: SeqNum,
    pub order: Order,
    pub locked_amount: u64,
    pub locked_asset_id: AssetId,
}

// ME → UBSCore + Settlement
pub struct TradeEvent {
    pub trade: Trade,
    pub taker_order_id: OrderId,
    pub maker_order_id: OrderId,
    pub taker_side: Side,
    pub base_asset_id: AssetId,
    pub quote_asset_id: AssetId,
}

// 订单状态变更
pub enum OrderEvent {
    Accepted { seq_id, order_id, user_id },
    Rejected { seq_id, order_id, user_id, reason },
    Filled { order_id, filled_qty, avg_price },
    PartialFilled { order_id, filled_qty, remaining_qty },
    Cancelled { order_id, unfilled_qty },
}
```

## 5. 集成与使用

### 5.1 命令行参数

```bash
# 原始流水线
cargo run --release

# UBSCore 流水线（启用 WAL）
cargo run --release -- --ubscore
```

### 5.2 性能对比

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

## 6. 测试

### 6.1 单元测试

```bash
cargo test

# 31 tests passing
# - messages::tests (3)
# - wal::tests (3)
# - ubscore::tests (4)
# - ... (21 others)
```

### 6.2 E2E 测试

```bash
sh scripts/test_e2e.sh

# ✅ t1_balances_deposited.csv: MATCH
# ✅ t2_balances_final.csv: MATCH
# ✅ t2_ledger.csv: MATCH
# ✅ t2_orderbook.csv: MATCH
# ✅ All tests passed!
```

## 7. 新增文件

| 文件 | 行数 | 描述 |
|------|------|------|
| `src/messages.rs` | 265 | 服务间消息类型 |
| `src/wal.rs` | 340 | Write-Ahead Log |
| `src/ubscore.rs` | 490 | User Balance Core |

## 8. 关键学习

### 8.1 安全第一

在金融系统中：
- **成熟稳定** > 极致性能
- **可审计** > 快速开发
- **用它睡得着觉** 是选型的最高标准

### 8.2 WAL 是唯一事实来源

```
All state = f(WAL)
```

任何时刻，系统状态都可以从 WAL 100% 重建。这是：
- **灾难恢复**的基础
- **审计合规**的保障
- **确定性测试**的前提

### 8.3 单线程是优势

UBSCore 选择单线程不是因为简单，而是因为：
- 自然的原子性（无锁）
- 不可能双重支付
- 可预测的延迟

---

## 下一步

Chapter 0x08c 将探索：
1. Ring Buffer 流水线连接
2. 多线程 Settlement
3. Ledger I/O 优化
4. 二进制 WAL 格式
