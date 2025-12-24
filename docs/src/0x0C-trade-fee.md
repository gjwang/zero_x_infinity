# 0x0C Trade Fee System | 交易手续费系统

<h3>
  <a href="#-english">🇺🇸 English</a>
  &nbsp;&nbsp;&nbsp;|&nbsp;&nbsp;&nbsp;
  <a href="#-chinese">🇨🇳 中文</a>
</h3>

<div id="-english"></div>

## 🇺🇸 English

> **📦 Code Changes**: [View Diff](https://github.com/gjwang/zero_x_infinity/compare/v0.0B-a-transfer...v0.0C-a-trade-fee) *(after implementation)*

---

## 1. Overview

### 1.1 Connecting the Dots: From Transfer to Trading

在 **0x0B** 章节中，我们建立了资金划转的 FSM 机制，让用户可以在 Funding 账户和 Spot 账户之间转移资产。但资金进入 Spot 账户后，交易所需要有收入来源。

这就是本章的主题：**交易手续费 (Trade Fee)**。

每当买卖双方成交时，交易所收取一定比例的手续费。这是交易所最核心的商业模式，也是整个系统能够持续运营的基础。

> **设计哲学**: 手续费的实现看似简单（不就是扣个百分比吗？），但实际涉及多个关键决策：
> - 费率在哪里配置？（Symbol 级别 vs 全局）
> - 从什么资产扣除？（支付的 vs 收到的）
> - 扣除时机在哪里？（ME 里扣 vs Settlement 扣）
> - 如何确保精度不丢失？（u64 * bps / 10000 的溢出问题）

### 1.2 Goal

Implement **Maker/Taker fee model** for trade execution. Fees are the primary revenue source for exchanges

### 1.3 Key Concepts

| Term | Definition |
|------|------------|
| **Maker** | Order that adds liquidity (resting on orderbook) |
| **Taker** | Order that removes liquidity (matches immediately) |
| **Fee Rate** | Percentage of trade value charged |
| **bps** | Basis points (1 bps = 0.01% = 0.0001) |

---

## 2. Fee Model Design

### 2.1 Why Maker/Taker Model?

传统股票交易所往往采用固定费率，但加密货币交易所普遍采用 **Maker/Taker** 模型。这不是随意的选择：

| 问题 | Maker/Taker 如何解决 |
|------|----------------------|
| 流动性不足 | 低 Maker 费率鼓励挂单 |
| 价格发现 | 盘口深度越深，价差越小 |
| 公平性 | 谁消耗流动性谁多付费 |

> **行业实践**: Binance、OKX、Bybit 等主流交易所都采用此模型。

### 2.2 Fee Rate Architecture

**两层费率体系**: Symbol 基础费率 × VIP 折扣系数

```
最终费率 = Symbol.base_fee × VipDiscountTable[user.vip_level] / 100
```

#### Layer 1: Symbol 基础费率

每个交易对定义自己的基础费率（不同交易对可能有不同费率）：

| 字段 | 精度 | 默认值 | 说明 |
|------|-----|-------|------|
| `base_maker_fee` | 10^6 | 1000 | 0.10% |
| `base_taker_fee` | 10^6 | 2000 | 0.20% |

#### Layer 2: VIP 折扣系数

VIP 等级和折扣从数据库配置（不硬编码级数）。

**VIP 等级表设计**:

| 字段 | 类型 | 说明 |
|------|------|------|
| `level` | SMALLINT PK | VIP 等级 (0, 1, 2, ...) |
| `discount_percent` | SMALLINT | 折扣百分比 (100=无折扣, 50=50%折扣) |
| `min_volume` | DECIMAL | 升级所需交易量 (可选) |
| `description` | VARCHAR | 等级描述 (可选) |

**示例数据**:

| level | discount_percent | description |
|-------|-----------------|-------------|
| 0 | 100 | Normal |
| 1 | 90 | VIP 1 |
| 2 | 80 | VIP 2 |
| 3 | 70 | VIP 3 |
| ... | ... | ... |

> 运营可配置任意数量的 VIP 等级，代码从数据库加载。

**示例计算**:
```
BTC_USDT: base_taker_fee = 2000 (0.20%)
User VIP 5: discount = 50%
最终费率 = 2000 × 50 / 100 = 1000 (0.10%)
```

> **Why 10^6 精度？**
> - 10^4 (bps) 只能表示到 0.01%，不够精细
> - 10^6 可以表示 0.0001%，足够支持 VIP 折扣和返佣
> - 与 u64 乘法不会溢出 (u64 * 10^6 / 10^6)

### 2.3 Fee Collection Point

```
Trade: Alice (Taker, BUY) ← → Bob (Maker, SELL)
       Alice buys 1 BTC @ 100,000 USDT

┌──────────────────────────────────────────────────────────┐
│ Before Fee:                                              │
│   Alice: -100,000 USDT, +1 BTC                          │
│   Bob:   +100,000 USDT, -1 BTC                          │
├──────────────────────────────────────────────────────────┤
│ After Fee (deducted from RECEIVED asset):               │
│   Alice (Taker 0.20%): -100,000 USDT, +0.998 BTC        │
│   Bob (Maker 0.10%):   +99,900 USDT,  -1 BTC            │
│                                                          │
│   Exchange collects: 0.002 BTC + 100 USDT               │
└──────────────────────────────────────────────────────────┘
```

**Rule**: Fee is always deducted from **what you receive**, not what you pay.

> **Why 从收到的资产扣除？**
> 1. **简化用户心理账单**: 用户支付 100 USDT，就是 100 USDT，不会多扣
> 2. **避免预算超支**: 买 1 BTC 不会因为手续费导致需要 100,020 USDT
> 3. **行业惯例**: Binance、Coinbase 都是这样做的

### 2.4 Why No Lock Reservation Needed

由于手续费从**收到的资产**中扣除，**不需要预留手续费**：

```
┌─────────────────────────────────────────────────────────────────────┐
│ 从 Gain（收到资产）扣费的好处                                        │
├─────────────────────────────────────────────────────────────────────┤
│ 用户收到 1 BTC → 扣 0.002 BTC 手续费 → 实际到账 0.998 BTC           │
│                                                                     │
│ ✅ 永远不会"余额不足付手续费"                                        │
│ ✅ 支付金额 = 实际支付金额（不多不少）                               │
│ ✅ 无需复杂的预留/退还逻辑                                           │
└─────────────────────────────────────────────────────────────────────┘
```

**对比从支付资产扣费**:

| 方案 | 锁定金额 | 问题 |
|------|---------|------|
| 从 Gain 扣 | `base_cost` | 无需额外预留 ✅ |
| 从 Pay 扣 | `base_cost + max_fee` | 余额可能不足，需预留 ❌ |

> **设计决策**: 采用"从 Gain 扣费"模式，简化锁定逻辑。
> - 买单锁定 USDT，手续费从收到的 BTC 中扣
> - 卖单锁定 BTC，手续费从收到的 USDT 中扣

### 2.5 Fee Responsibility: UBSCore (第一性原理)

**核心问题**: 谁负责计费？

```
费用扣除 = 余额变动 = 必须由 UBSCore 执行
```

| 问题 | 答案 |
|------|------|
| 谁知道成交了？ | ME |
| 谁管理余额？ | **UBSCore** |
| 谁能执行扣款？ | **UBSCore** |
| 谁负责计费？ | **UBSCore** |

**数据流**:
```
ME ──▶ Trade{role} ──▶ UBSCore ──▶ BalanceEvent{fee} ──▶ Settlement ──▶ TDengine
                          │
                     ① 获取 VIP 等级 (内存)
                     ② 获取 Symbol 费率 (内存)
                     ③ 计算 fee = received × rate
                     ④ credit(net_amount)
```

### 2.6 High Performance Design

**高效的关键**: 所有配置在 UBSCore 内存中

```
UBSCore 内存结构 (启动时加载):
├── user_vip_levels: HashMap<UserId, u8>
├── vip_discounts: HashMap<u8, u8>  // level → discount%
└── symbol_fees: HashMap<SymbolId, (u64, u64)>  // (maker, taker)

费用计算 = 纯内存操作, O(1)
```

| 组件 | 职责 | 阻塞？ |
|------|------|-------|
| UBSCore | 计算 fee, 更新余额 | ❌ 纯内存 |
| BalanceEvent | 传递 fee 信息 | ❌ 异步通道 |
| Settlement | 写入 TDengine | ❌ 独立线程 |

> **Why 高效？**
> - 没有 I/O 在关键路径上
> - 所有数据都在内存
> - 输出复用现有 BalanceEvent 通道

### 2.7 Per-User BalanceEvent Design

**核心洞察**: 一个 Trade 产生两个用户的余额变动 → 两个 BalanceEvent

```
Trade ──▶ UBSCore ──┬──▶ BalanceEvent{user: buyer}  ──▶ WS + TDengine
                    │
                    └──▶ BalanceEvent{user: seller} ──▶ WS + TDengine
```

**Per-User 事件结构**:

| 字段 | 类型 | 说明 |
|------|------|------|
| `trade_id` | u64 | 关联原始 Trade |
| `user_id` | u64 | 这个事件属于谁 |
| `debit_asset` | u32 | 支出资产 |
| `debit_amount` | u64 | 支出金额 |
| `credit_asset` | u32 | 收入资产 |
| `credit_amount` | u64 | 收入金额 (净额, 已扣 fee) |
| `fee` | u64 | 手续费 |
| `is_maker` | bool | 是否 Maker |

**示例代码 (伪代码, 仅供参考)**:
```rust
// ⚠️ 伪代码 - 实现时可能有调整
BalanceEvent::TradeSettled {
    trade_id: u64,         // 关联原始 Trade
    user_id: u64,          // 这个事件属于谁
    
    debit_asset: u32,      // 支出
    debit_amount: u64,
    credit_asset: u32,     // 收入 (净额)
    credit_amount: u64,
    
    fee: u64,              // 手续费
    is_maker: bool,        // 角色
}
```

> **Why Per-User 设计？**
> - **单一职责**: 一个事件 = 一个用户的余额变动
> - **解耦**: 用户不需要知道对手方
> - **WebSocket 友好**: 按 user_id 直接路由推送
> - **查询友好**: TDengine 按 user_id 分区
> - **隐私安全**: 用户只看自己数据

---

## 3. Data Model

### 3.1 Symbol 基础费率配置

```sql
-- Symbol 基础费率 (10^6 精度: 1000 = 0.10%)
ALTER TABLE symbols_tb ADD COLUMN base_maker_fee INTEGER NOT NULL DEFAULT 1000;
ALTER TABLE symbols_tb ADD COLUMN base_taker_fee INTEGER NOT NULL DEFAULT 2000;
```

### 3.2 User VIP 等级

```sql
-- User VIP 等级 (0-9, 0=普通用户, 9=顶级用户)
ALTER TABLE users_tb ADD COLUMN vip_level SMALLINT NOT NULL DEFAULT 0;
```

### 3.3 Trade Record Enhancement

Existing `Trade` struct already has:
- `fee: u64` - Amount of fee charged (in received asset's scaled units)
- `role: u8` - 0=Maker, 1=Taker

### 3.4 Fee Record Storage

手续费信息**已包含在 Trade 记录中**：

| 存储位置 | 内容 |
|---------|------|
| `trades_tb` (TDengine) | `fee`, `fee_asset`, `role` 字段 |
| Trade Event | 实时推送给下游 (WS, Kafka) |

### 3.5 Event Sourcing: BalanceEventBatch (资产可溯源)

**核心设计**: 一个 Trade 产生一组 BalanceEvent 作为**原子整体**

```
Trade ──▶ UBSCore ──▶ BalanceEventBatch{trade_id, events: [...]}
                              │
                              ├── TradeSettled{user: buyer}   // 买方
                              ├── TradeSettled{user: seller}  // 卖方
                              ├── FeeReceived{account: REVENUE, from: buyer}
                              └── FeeReceived{account: REVENUE, from: seller}
```

**示例结构 (伪代码)**:
```rust
// ⚠️ 伪代码 - 实现时可能有调整
BalanceEventBatch {
    trade_id: u64,
    ts: Timestamp,
    events: [
        TradeSettled{user: buyer_id, debit_asset, debit_amount, credit_asset, credit_amount, fee},
        TradeSettled{user: seller_id, debit_asset, debit_amount, credit_asset, credit_amount, fee},
        FeeReceived{account: REVENUE_ID, asset: base_asset, amount: buyer_fee, from_user: buyer_id},
        FeeReceived{account: REVENUE_ID, asset: quote_asset, amount: seller_fee, from_user: seller_id},
    ]
}
```

**原子整体特性**:

| 特性 | 说明 |
|------|------|
| 一起生成 | 同一个 trade_id |
| 一起持久化 | 同一批写入 TDengine |
| 一起可追溯 | 通过 trade_id 关联所有事件 |

**资产守恒验证**:
```
buyer.debit(quote)  + buyer.credit(base - fee)   = 0  ✓
seller.debit(base)  + seller.credit(quote - fee) = 0  ✓
revenue.credit(buyer_fee + seller_fee)           = fee_total ✓

Σ 变动 = 0 (资产守恒, 可审计)
```

**TDengine 存储 (Event Sourcing)**:

| 表 | 内容 |
|------|------|
| `balance_events_tb` | 所有 BalanceEvent (TradeSettled + FeeReceived) |

> **Why Event Sourcing?**
> - **每笔可追溯**: 任何 fee 都能追溯到 trade_id + user_id
> - **资产守恒**: 事件批次内守恒可验证
> - **聚合是衍生**: 余额 = SUM(events)，按需计算

---

## 4. Implementation Architecture

### 4.1 Complete Data Flow

```
┌───────────┐    ┌───────────┐    ┌─────────────────────────────────────────┐
│    ME     │───▶│  UBSCore  │───▶│         BalanceEventBatch               │
│  (Match)  │    │ (Fee计算)  │    │  ┌─ TradeSettled{buyer}                 │
└───────────┘    └───────────┘    │  ├─ TradeSettled{seller}                │
                      │           │  ├─ FeeReceived{REVENUE, from:buyer}    │
                      │           │  └─ FeeReceived{REVENUE, from:seller}   │
          内存: VIP等级/费率      └───────────────┬─────────────────────────┘
                                                  │
                                                  ▼
                              ┌──────────────────────────────────────────────┐
                              │              Settlement Service              │
                              │  ① 批量写入 TDengine                         │
                              │  ② WebSocket 推送 (按 user_id 路由)          │
                              │  ③ Kafka 发布 (可选)                         │
                              └──────────────────────────────────────────────┘
```

### 4.2 TDengine Schema Design

**balance_events 超级表**:
```sql
CREATE STABLE balance_events (
    ts          TIMESTAMP,
    event_type  TINYINT,       -- 1=TradeSettled, 2=FeeReceived, 3=Deposit...
    trade_id    BIGINT,
    debit_asset INT,
    debit_amt   BIGINT,
    credit_asset INT,
    credit_amt  BIGINT,
    fee         BIGINT,
    fee_asset   INT,
    is_maker    BOOL,
    from_user   BIGINT         -- FeeReceived: 来源用户
) TAGS (
    account_id  BIGINT         -- user_id 或 REVENUE_ID
);

-- 每个账户一个子表
CREATE TABLE user_1001_events USING balance_events TAGS (1001);
CREATE TABLE user_1002_events USING balance_events TAGS (1002);
CREATE TABLE revenue_events   USING balance_events TAGS (0);  -- REVENUE_ID=0
```

**设计要点**:

| 设计 | 理由 |
|------|------|
| 按 account_id 分表 | 用户查询只扫自己的表 |
| 时间戳索引 | TDengine 原生优化 |
| event_type 字段 | 区分不同事件类型 |

### 4.3 Query Patterns

**用户查询手续费历史**:
```sql
SELECT ts, trade_id, fee, fee_asset, is_maker
FROM user_1001_events
WHERE event_type = 1  -- TradeSettled
  AND ts > NOW() - 30d
ORDER BY ts DESC
LIMIT 100;
```

**平台 Fee 收入统计**:
```sql
SELECT fee_asset, SUM(credit_amt) as total_fee
FROM revenue_events
WHERE ts > NOW() - 1d
GROUP BY fee_asset;
```

**追溯某笔 Trade 的所有事件**:
```sql
SELECT * FROM balance_events
WHERE trade_id = 12345
ORDER BY ts;
```

### 4.4 Consumer Architecture

```
BalanceEventBatch
       │
       ├──▶ TDengine Writer (批量写入, 高吞吐)
       │       └── 按 account_id 路由到子表
       │
       ├──▶ WebSocket Router (实时推送)
       │       └── 按 user_id 路由到 WS 连接
       │
       └──▶ Kafka Publisher (可选, 下游订阅)
               └── Topic: balance_events
```

### 4.5 Performance Considerations

| 优化点 | 策略 |
|--------|------|
| **批量写入** | BalanceEventBatch 一次性写入 |
| **分表策略** | 按 user_id 分表，避免热点 |
| **时间分区** | TDengine 自动按时间分区 |
| **异步处理** | UBSCore 发送后不等待 |

---


## 5. API Changes

### 5.1 Trade Response

```json
{
  "trade_id": "12345",
  "price": "100000.00",
  "qty": "1.00000000",
  "fee": "0.00200000",       // NEW: Fee amount
  "fee_asset": "BTC",        // NEW: Fee asset
  "role": "TAKER"            // NEW: Maker/Taker
}
```

### 5.2 WebSocket Trade Update

```json
{
  "e": "trade.update",
  "data": {
    "trade_id": "12345",
    "fee": "0.002",
    "fee_asset": "BTC",
    "is_maker": false
  }
}
```

---

## 6. Edge Cases

| Case | Handling |
|------|----------|
| Fee rounds to 0 | Minimum fee = 1 (smallest unit) |
| Zero-fee symbol | Allow `maker_fee_bps = 0` |
| Insufficient for fee | Reject order pre-trade (not applicable, fee from received) |

---

## 7. Verification Plan

### 7.1 Unit Tests
- Fee calculation accuracy (multiple precisions)
- Maker vs Taker role assignment

### 7.2 Integration Tests
- E2E trade with fee deduction
- Fee ledger reconciliation

### 7.3 Acceptance Criteria
- [ ] Trades deduct correct fees
- [ ] Fee ledger matches Σ(trade.fee)
- [ ] API returns fee info
- [ ] WS pushes fee info

---

<br>
<div align="right"><a href="#-english">↑ Back to Top</a></div>
<br>

---

<div id="-chinese"></div>

## 🇨🇳 中文

> **📦 代码变更**: [查看 Diff](https://github.com/gjwang/zero_x_infinity/compare/v0.0B-a-transfer...v0.0C-a-trade-fee) *(实现后)*

---

## 设计摘要

完整设计详见英文部分。以下是核心要点：

### 1. 费率模型

```
最终费率 = Symbol.base_fee × VipDiscount[vip_level] / 100
```

- **Layer 1**: Symbol 基础费率 (10^6 精度)
- **Layer 2**: VIP 折扣系数 (从数据库加载)

### 2. 核心设计原则

| 设计点 | 说明 |
|--------|------|
| **从 Gain 扣费** | 无需预留，不可能欠费 |
| **UBSCore 计费** | 余额权威，内存费率 O(1) |
| **Per-User Event** | 每用户一个事件，解耦隐私 |
| **BalanceEventBatch** | 原子整体 (buyer + seller + revenue) |
| **Event Sourcing** | TDengine 存储，聚合衍生 |

### 3. 数据流

```
ME → Trade{role} → UBSCore(fee计算) → BalanceEventBatch → Settlement → TDengine
                                           │
                                           ├── TradeSettled{buyer}
                                           ├── TradeSettled{seller}
                                           └── FeeReceived{REVENUE} ×2
```

### 4. 资产守恒

```
buyer.debit(quote)  + buyer.credit(base - fee)   = 0  ✓
seller.debit(base)  + seller.credit(quote - fee) = 0  ✓
revenue.credit(buyer_fee + seller_fee)           = fee_total ✓

Σ 变动 = 0 (可审计)
```

---

<br>
<div align="right"><a href="#-chinese">↑ Back to Top</a></div>
<br>
