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

---

## 9. 重要 Bug 修复：Cost 计算溢出

### 9.1 问题发现

在实现 UBSCore 并运行 `--ubscore` 模式测试时，发现了 **1032 个订单被拒绝**，而传统模式全部接受。

```bash
# UBSCore 模式
$ cargo run --release -- --ubscore
  Accepted: 98968
  Rejected: 1032  # ← 异常！

# 传统模式
$ cargo run --release
  Accepted: 100000
  Rejected: 0
```

### 9.2 根本原因

**Cost 计算时 `price * qty` 溢出 u64**

以真实订单 #21 为例：
- `price = 84,956,010,000` (84956.01 USDT，6位精度)
- `qty = 256,284,400` (2.562844 BTC，8位精度)
- `price * qty = 2.177 × 10^19`
- `u64::MAX = 1.844 × 10^19`

**超过 u64 上限！**

### 9.3 传统模式为什么没报错？

**Release 模式的 wrapping arithmetic！**

```rust
// 传统模式代码
let cost = input.price * input.qty / qty_unit;
```

在 Release 模式下，u64 乘法溢出会 **wrapping（取模 2^64）**，得到一个**看似合理但完全错误的值**：

| 计算方式 | 结果 | 解释 |
|----------|------|------|
| 正确 (u128) | 217,729,000,492 USDT | 应锁定金额 |
| 错误 (u64 wrapping) | 33,261,559,755 USDT | 实际锁定金额 |
| **差异** | **184,467,440,737 USDT** | **少锁了 1844 亿！** |

**这是严重的金融安全漏洞：用户只被锁定了 33,261 USDT，却买了价值 217,729 USDT 的 BTC！**

### 9.4 修复方案

```rust
/// 使用 u128 进行中间计算，返回明确的错误类型
pub fn calculate_cost(&self, qty_unit: u64) -> Result<u64, CostError> {
    match self.side {
        Side::Buy => {
            // 使用 u128 避免中间计算溢出
            let cost_128 = (self.price as u128) * (self.qty as u128) / (qty_unit as u128);
            
            // 如果最终结果超过 u64，返回明确错误
            if cost_128 > u64::MAX as u128 {
                Err(CostError::Overflow { price, qty, qty_unit })
            } else {
                Ok(cost_128 as u64)
            }
        }
        Side::Sell => Ok(self.qty),
    }
}
```

**设计原则：金融级系统禁止静默填充默认值**

### 9.5 配置问题：USDT 精度过高

进一步分析发现，**USDT 使用 6 位精度（decimals=6）是溢出的根本原因**：

| 配置 | price 精度 | qty 精度 | 最大可交易 BTC @ $85000 |
|------|------------|----------|-------------------------|
| **当前** | 6 位 | 8 位 | **2.17 BTC** ❌ |
| **推荐** | 2 位 | 8 位 | **21,702 BTC** ✅ |

**Binance 使用 2 位价格精度**，可以安全交易超过 21,000 BTC。

当前配置：
```csv
# fixtures/assets_config.csv
asset_id,asset,decimals,display_decimals
2,USDT,6,4  # ← 6 位精度导致溢出风险
```

建议修改为：
```csv
2,USDT,2,2  # 或最多 4 位
```

> ⚠️ **配置精度时的关键检查**
> 
> 在配置 `price_decimal` 和 `qty_decimal` 时，**必须验证最大可交易额是否在合理范围内**：
> 
> ```
> max_tradeable_value = u64::MAX / (10^price_decimal × 10^qty_decimal)
>                     = 1.84×10^19 / 10^(price_decimal + qty_decimal)
> ```
> 
> | price + qty 精度 | 最大交易额 (以基础单位计) | 举例 |
> |------------------|---------------------------|------|
> | 6 + 8 = 14 位 | 1,844 单位 | 仅 1.8 BTC @ $100k |
> | 4 + 8 = 12 位 | 184,467 单位 | 184 BTC @ $100k |
> | 2 + 8 = 10 位 | 18,446,744 单位 | 18,446 BTC @ $100k |
> 
> **建议**：确保 `最大可交易额` 远大于业务预期的最大单笔订单量。

### 9.6 测试用例

添加了关键测试用例记录此问题：

```rust
#[test]
fn test_buy_cost_real_world_overflow_case() {
    // CRITICAL: Real-world case from production test data
    // Order #21: Buy 2.562844 BTC @ 84956.01 USDT
    //
    // With naive u64: price * qty = 2.177×10^19 > u64::MAX
    //   → wrapping overflow → 33,261,559,755 (WRONG!)
    //
    // With u128 intermediate: 217,729,000,492 (CORRECT!)
    
    let price = 84_956_010_000u64;
    let qty = 256_284_400u64;
    let qty_unit = 100_000_000u64;
    
    let order = buy_order(price, qty);
    let cost = order.calculate_cost(qty_unit);
    
    assert_eq!(cost, Ok(217_729_000_492));
    
    // 验证这在 naive u64 乘法中确实会溢出
    assert!(price.checked_mul(qty).is_none());
}
```

### 9.7 教训总结

1. **永远使用 checked 算术或显式溢出处理**
2. **金融系统禁止静默填充默认值**（如 `unwrap_or(u64::MAX)`）
3. **精度设计要考虑乘法溢出边界**
4. **多模式测试能发现隐藏 bug**（传统模式看似正确，UBSCore 暴露问题）
