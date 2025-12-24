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

In **0x0B**, we built the FSM mechanism for fund transfers between Funding and Spot accounts. Once funds enter the Spot account, the exchange needs a revenue source.

This is the topic of this chapter: **Trade Fee**.

Whenever buyers and sellers execute trades, the exchange collects a percentage fee. This is the core business model of exchanges and the foundation for sustainable operations.

> **Design Philosophy**: Fee implementation seems simple (just deducting a percentage, right?), but involves multiple key decisions:
> - Where to configure fee rates? (Symbol level vs Global)
> - Which asset to deduct from? (Paid vs Received)
> - When to deduct? (In ME vs In Settlement)
> - How to ensure precision? (u64 * bps / 10000 overflow issues)

### 1.2 Goal

Implement **Maker/Taker fee model** for trade execution. Fees are the primary revenue source for exchanges

### 1.3 Key Concepts

| Term | Definition |
|------|------------|
| **Maker** | Order that adds liquidity (resting on orderbook) |
| **Taker** | Order that removes liquidity (matches immediately) |
| **Fee Rate** | Percentage of trade value charged |
| **bps** | Basis points (1 bps = 0.01% = 0.0001) |

### 1.4 Architecture Overview

```
┌─────────── Fee Model ────────────┐
│                                  │
│  Final Rate = Symbol.base_fee    │
│             × VipDiscount / 100  │
└──────────────────────────────────┘

┌─────────── Data Flow ─────────────────────────────────────────────────────┐
│                                                                           │
│  ME ────▶ Trade{role} ────▶ UBSCore ────▶ BalanceEventBatch ────▶ TDengine
│              │                  │              │                          │
│              │           Memory: VIP/Fees      ├── buyer event            │
│              │           O(1) fee calc         ├── seller event           │
│              │                                 └── revenue event ×2       │
│              │                                                            │
└──────────────┴────────────────────────────────────────────────────────────┘

┌─────────── Core Design ───────────┐
│ ✅ Fee from Gain → No reservation │
│ ✅ UBSCore billing → Balance auth │
│ ✅ Per-User Event → Decoupled     │
│ ✅ Event Sourcing → Conservation  │
└───────────────────────────────────┘
```

---

## 2. Fee Model Design

### 2.1 Why Maker/Taker Model?

Traditional stock exchanges use fixed rates, but crypto exchanges universally adopt the **Maker/Taker** model. This is not arbitrary:

| Problem | How Maker/Taker Solves |
|---------|------------------------|
| Low liquidity | Low Maker fees encourage limit orders |
| Price discovery | Deeper orderbook, narrower spreads |
| Fairness | Liquidity takers pay more |

> **Industry Practice**: Binance, OKX, Bybit all use this model.

### 2.2 Fee Rate Architecture

**Two-Layer System**: Symbol base rate × VIP discount coefficient

```
Final Rate = Symbol.base_fee × VipDiscountTable[user.vip_level] / 100
```

#### Layer 1: Symbol Base Rate

Each trading pair defines its own base rate:

| Field | Precision | Default | Description |
|-------|-----------|---------|-------------|
| `base_maker_fee` | 10^6 | 1000 | 0.10% |
| `base_taker_fee` | 10^6 | 2000 | 0.20% |

#### Layer 2: VIP Discount Coefficient

VIP levels and discounts are configured from database (not hardcoded).

**VIP Level Table Design**:

| Field | Type | Description |
|-------|------|-------------|
| `level` | SMALLINT PK | VIP level (0, 1, 2, ...) |
| `discount_percent` | SMALLINT | Discount % (100=no discount, 50=50% off) |
| `min_volume` | DECIMAL | Trading volume for upgrade (optional) |
| `description` | VARCHAR | Level description (optional) |

**Example Data**:

| level | discount_percent | description |
|-------|-----------------|-------------|
| 0 | 100 | Normal |
| 1 | 90 | VIP 1 |
| 2 | 80 | VIP 2 |
| 3 | 70 | VIP 3 |
| ... | ... | ... |

> Operations can configure any number of VIP levels; code loads from database.

**Example Calculation**:
```
BTC_USDT: base_taker_fee = 2000 (0.20%)
User VIP 5: discount = 50%
Final Rate = 2000 × 50 / 100 = 1000 (0.10%)
```

> **Why 10^6 Precision?**
> - 10^4 (bps) only represents down to 0.01%, not fine enough
> - 10^6 can represent 0.0001%, sufficient for VIP discounts and rebates
> - Safe with u128 intermediate: `(amount as u128 * rate as u128 / 10^6) as u64`

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

> **Why deduct from received asset?**
> 1. **Simplify user mental accounting**: User pays 100 USDT, it's exactly 100 USDT
> 2. **Avoid budget overrun**: Buying 1 BTC won't require 100,020 USDT due to fees
> 3. **Industry practice**: Binance, Coinbase all do this

### 2.4 Why No Lock Reservation Needed

Since fees are deducted from **received asset**, **no fee reservation needed**:

```
┌─────────────────────────────────────────────────────────────────────┐
│ Benefits of Fee from Gain (Received Asset)                         │
├─────────────────────────────────────────────────────────────────────┤
│ User receives 1 BTC → Deduct 0.002 BTC fee → Net credit 0.998 BTC │
│                                                                     │
│ ✅ Never "insufficient balance for fee"                            │
│ ✅ Pay amount = Actual pay amount (exact)                          │
│ ✅ No complex reservation/refund logic                             │
└─────────────────────────────────────────────────────────────────────┘
```

**Compare with deducting from paid asset**:

| Approach | Lock Amount | Issue |
|----------|-------------|-------|
| From Gain | `base_cost` | No extra reservation ✅ |
| From Pay | `base_cost + max_fee` | May insufficient, need reservation ❌ |

> **Design Decision**: Use "fee from gain" mode, simplify lock logic.
> - Buy order locks USDT, fee deducted from received BTC
> - Sell order locks BTC, fee deducted from received USDT

### 2.5 Fee Responsibility: UBSCore (First Principles)

**Core Question**: Who is responsible for fee calculation?

```
Fee deduction = Balance change = Must be executed by UBSCore
```

| Question | Answer |
|----------|--------|
| Who knows trade occurred? | ME |
| Who manages balances? | **UBSCore** |
| Who can execute deductions? | **UBSCore** |
| Who is responsible for fees? | **UBSCore** |

**Data Flow**:
```
ME ──▶ Trade{role} ──▶ UBSCore ──▶ BalanceEvent{fee} ──▶ Settlement ──▶ TDengine
                          │
                     ① Get VIP level (memory)
                     ② Get Symbol fee rate (memory)
                     ③ Calculate fee = received × rate
                     ④ credit(net_amount)
```

### 2.6 High Performance Design

**Key to efficiency**: All config in UBSCore memory

```
UBSCore Memory Structure (loaded at startup):
├── user_vip_levels: HashMap<UserId, u8>
├── vip_discounts: HashMap<u8, u8>  // level → discount%
└── symbol_fees: HashMap<SymbolId, (u64, u64)>  // (maker, taker)

Fee calculation = Pure memory operation, O(1)
```

| Component | Responsibility | Blocking? |
|-----------|----------------|-----------|
| UBSCore | Calculate fee, update balance | ❌ Pure memory |
| BalanceEvent | Pass fee info | ❌ Async channel |
| Settlement | Write to TDengine | ❌ Separate thread |

> **Why efficient?**
> - No I/O on critical path
> - All data in memory
> - Output reuses existing BalanceEvent channel

### 2.7 Per-User BalanceEvent Design

**Core Insight**: One Trade produces two users' balance changes → Two BalanceEvents

```
Trade ──▶ UBSCore ──┬──▶ BalanceEvent{user: buyer}  ──▶ WS + TDengine
                    │
                    └──▶ BalanceEvent{user: seller} ──▶ WS + TDengine
```

**Per-User Event Structure**:

| Field | Type | Description |
|-------|------|-------------|
| `trade_id` | u64 | Links to original Trade |
| `user_id` | u64 | Who this event belongs to |
| `debit_asset` | u32 | Asset paid |
| `debit_amount` | u64 | Amount paid |
| `credit_asset` | u32 | Asset received |
| `credit_amount` | u64 | Net amount (after fee) |
| `fee` | u64 | Fee charged |
| `is_maker` | bool | Is Maker role |

**Example Code (Pseudocode, for reference only)**:
```rust
// ⚠️ Pseudocode - may change during implementation
BalanceEvent::TradeSettled {
    trade_id: u64,         // Links to original Trade
    user_id: u64,          // Who this event belongs to
    
    debit_asset: u32,      // Paid
    debit_amount: u64,
    credit_asset: u32,     // Received (net)
    credit_amount: u64,
    
    fee: u64,              // Fee
    is_maker: bool,        // Role
}
```

> **Why Per-User Design?**
> - **Single responsibility**: One event = One user's balance change
> - **Decoupled**: User doesn't need to know counterparty
> - **WebSocket friendly**: Route directly by user_id
> - **Query friendly**: TDengine partitioned by user_id
> - **Privacy safe**: User only sees own data

---

## 3. Data Model

### 3.1 Symbol Base Fee Configuration

```sql
-- Symbol base fee (10^6 precision: 1000 = 0.10%)
ALTER TABLE symbols_tb ADD COLUMN base_maker_fee INTEGER NOT NULL DEFAULT 1000;
ALTER TABLE symbols_tb ADD COLUMN base_taker_fee INTEGER NOT NULL DEFAULT 2000;
```

### 3.2 User VIP Level

```sql
-- User VIP level (0-9, 0=normal user, 9=top tier)
ALTER TABLE users_tb ADD COLUMN vip_level SMALLINT NOT NULL DEFAULT 0;
```

### 3.3 Trade Record Enhancement

Existing `Trade` struct already has:
- `fee: u64` - Amount of fee charged (in received asset's scaled units)
- `role: u8` - 0=Maker, 1=Taker

### 3.4 Fee Record Storage

Fee info **is already included in Trade record**:

| Storage | Content |
|---------|---------|
| `trades_tb` (TDengine) | `fee`, `fee_asset`, `role` fields |
| Trade Event | Real-time push to downstream (WS, Kafka) |

### 3.5 Event Sourcing: BalanceEventBatch (Full Traceability)

**Core Design**: One Trade produces a group of BalanceEvents as **atomic unit**

```
Trade ──▶ UBSCore ──▶ BalanceEventBatch{trade_id, events: [...]}
                              │
                              ├── TradeSettled{user: buyer}   // Buyer
                              ├── TradeSettled{user: seller}  // Seller
                              ├── FeeReceived{account: REVENUE, from: buyer}
                              └── FeeReceived{account: REVENUE, from: seller}
```

**Example Structure (Pseudocode)**:
```rust
// ⚠️ Pseudocode - may change during implementation
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

**Atomic Unit Properties**:

| Property | Description |
|----------|-------------|
| Generated together | Same trade_id |
| Persisted together | Single batch write to TDengine |
| Traced together | All events linked by trade_id |

**Asset Conservation Verification**:
```
buyer.debit(quote)  + buyer.credit(base - fee)   = 0  ✓
seller.debit(base)  + seller.credit(quote - fee) = 0  ✓
revenue.credit(buyer_fee + seller_fee)           = fee_total ✓

Σ changes = 0 (Asset conservation, auditable)
```

**TDengine Storage (Event Sourcing)**:

| Table | Content |
|-------|---------|
| `balance_events_tb` | All BalanceEvents (TradeSettled + FeeReceived) |

> **Why Event Sourcing?**
> - **Full traceability**: Any fee can be traced to trade_id + user_id
> - **Asset conservation**: Conservation verifiable within event batch
> - **Aggregation is derived**: Balance = SUM(events), computed on demand

---

## 4. Implementation Architecture

### 4.1 Complete Data Flow

```
┌───────────┐    ┌───────────┐    ┌─────────────────────────────────────────┐
│    ME     │───▶│  UBSCore  │───▶│         BalanceEventBatch               │
│  (Match)  │    │ (Fee calc)│    │  ┌─ TradeSettled{buyer}                 │
└───────────┘    └───────────┘    │  ├─ TradeSettled{seller}                │
                      │           │  ├─ FeeReceived{REVENUE, from:buyer}    │
                      │           │  └─ FeeReceived{REVENUE, from:seller}   │
          Memory: VIP/Fee rates   └───────────────┬─────────────────────────┘
                                                  │
                                                  ▼
                              ┌──────────────────────────────────────────────┐
                              │              Settlement Service              │
                              │  ① Batch write to TDengine                   │
                              │  ② WebSocket push (routed by user_id)       │
                              │  ③ Kafka publish (optional)                 │
                              └──────────────────────────────────────────────┘
```

### 4.2 TDengine Schema Design

**balance_events Super Table**:
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
    from_user   BIGINT         -- FeeReceived: source user
) TAGS (
    account_id  BIGINT         -- user_id or REVENUE_ID
);

-- One subtable per account
CREATE TABLE user_1001_events USING balance_events TAGS (1001);
CREATE TABLE user_1002_events USING balance_events TAGS (1002);
CREATE TABLE revenue_events   USING balance_events TAGS (0);  -- REVENUE_ID=0
```

**Design Points**:

| Design | Rationale |
|--------|-----------|
| Partition by account_id | User queries scan only their table |
| Timestamp index | TDengine native optimization |
| event_type field | Distinguish event types |

### 4.3 Query Patterns

**User query fee history**:
```sql
SELECT ts, trade_id, fee, fee_asset, is_maker
FROM user_1001_events
WHERE event_type = 1  -- TradeSettled
  AND ts > NOW() - 30d
ORDER BY ts DESC
LIMIT 100;
```

**Platform fee income stats**:
```sql
SELECT fee_asset, SUM(credit_amt) as total_fee
FROM revenue_events
WHERE ts > NOW() - 1d
GROUP BY fee_asset;
```

**Trace all events for a trade**:
```sql
SELECT * FROM balance_events
WHERE trade_id = 12345
ORDER BY ts;
```

### 4.4 Consumer Architecture

```
BalanceEventBatch
       │
       ├──▶ TDengine Writer (batch write, high throughput)
       │       └── Route to subtable by account_id
       │
       ├──▶ WebSocket Router (real-time push)
       │       └── Route to WS connection by user_id
       │
       └──▶ Kafka Publisher (optional, downstream subscription)
               └── Topic: balance_events
```

### 4.5 Performance Considerations

| Optimization | Strategy |
|--------------|----------|
| **Batch write** | BalanceEventBatch writes at once |
| **Partition strategy** | Partition by user_id, avoid hotspots |
| **Time partition** | TDengine auto partitions by time |
| **Async processing** | UBSCore doesn't wait after send |

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

## 1. 概述

### 1.1 从资金划转到交易

在 **0x0B** 章节中，我们建立了资金划转机制。本章的主题是**交易手续费**——交易所最核心的商业模式。

### 1.2 目标

实现 **Maker/Taker 手续费模型**。

### 1.3 核心概念

| 术语 | 定义 |
|------|------|
| **Maker** | 挂单方 (订单在盘口等待成交) |
| **Taker** | 吃单方 (订单立即匹配成交) |
| **费率** | 交易额的百分比 |
| **bps** | 基点 (1 bps = 0.01%) |

### 1.4 架构总览

```
┌─────────── 费率模型 ────────────┐
│  最终费率 = Symbol.base_fee    │
│           × VipDiscount / 100  │
└────────────────────────────────┘

┌─────────── 数据流 ─────────────────────────────────────────────────────┐
│  ME ────▶ Trade{role} ────▶ UBSCore ────▶ BalanceEventBatch ────▶ TDengine
│              │                  │              │                       │
│              │           内存: VIP/费率        ├── buyer event         │
│              │           O(1) fee 计算         ├── seller event        │
│              │                                 └── revenue event ×2    │
└──────────────┴─────────────────────────────────────────────────────────┘

┌─────────── 核心设计 ───────────┐
│ ✅ 从 Gain 扣费 → 无需预留     │
│ ✅ UBSCore 计费 → 余额权威     │
│ ✅ Per-User Event → 解耦隐私   │
│ ✅ Event Sourcing → 资产守恒   │
└────────────────────────────────┘
```

---

## 2. 费率模型设计

### 2.1 为什么选择 Maker/Taker?

| 问题 | 解决方案 |
|------|---------|
| 流动性不足 | 低 Maker 费率鼓励挂单 |
| 价格发现 | 盘口深度越深，价差越小 |
| 公平性 | 消耗流动性者多付费 |

### 2.2 两层费率体系

```
最终费率 = Symbol.base_fee × VipDiscount[vip_level] / 100
```

**Layer 1: Symbol 基础费率**

| 字段 | 精度 | 默认值 | 说明 |
|------|-----|-------|------|
| `base_maker_fee` | 10^6 | 1000 | 0.10% |
| `base_taker_fee` | 10^6 | 2000 | 0.20% |

**Layer 2: VIP 折扣系数**

| 字段 | 类型 | 说明 |
|------|------|------|
| `level` | SMALLINT PK | VIP 等级 |
| `discount_percent` | SMALLINT | 折扣百分比 |

### 2.3 手续费扣除点

**规则**: 手续费从**收到的资产**扣除，不是支付的资产。

```
Alice (Taker, BUY) 以 100,000 USDT 购买 1 BTC

Before: Alice -100,000 USDT, +1 BTC
After:  Alice -100,000 USDT, +0.998 BTC (手续费 0.002 BTC)
```

### 2.4 无需预留手续费

从 Gain 扣费的好处：
- ✅ 永远不会"余额不足付手续费"
- ✅ 支付金额 = 实际支付金额
- ✅ 无需复杂的预留/退还逻辑

### 2.5 计费责任: UBSCore (第一性原理)

```
费用扣除 = 余额变动 = 必须由 UBSCore 执行
```

| 问题 | 答案 |
|------|------|
| 谁管理余额？ | **UBSCore** |
| 谁能执行扣款？ | **UBSCore** |
| 谁负责计费？ | **UBSCore** |

### 2.6 高性能设计

```
UBSCore 内存结构 (启动时加载):
├── user_vip_levels: HashMap<UserId, u8>
├── vip_discounts: HashMap<u8, u8>
└── symbol_fees: HashMap<SymbolId, (u64, u64)>

费用计算 = 纯内存操作, O(1)
```

### 2.7 Per-User BalanceEvent

一个 Trade → 两个用户事件

```
Trade ──▶ UBSCore ──┬──▶ BalanceEvent{user: buyer}
                    └──▶ BalanceEvent{user: seller}
```

---

## 3. 数据模型

### 3.1 Symbol 费率配置

```sql
ALTER TABLE symbols_tb ADD COLUMN base_maker_fee INTEGER NOT NULL DEFAULT 1000;
ALTER TABLE symbols_tb ADD COLUMN base_taker_fee INTEGER NOT NULL DEFAULT 2000;
```

### 3.2 User VIP 等级

```sql
ALTER TABLE users_tb ADD COLUMN vip_level SMALLINT NOT NULL DEFAULT 0;
```

### 3.3 Event Sourcing: BalanceEventBatch

一个 Trade 产生一组 BalanceEvent 作为**原子整体**：

```
BalanceEventBatch{trade_id}
├── TradeSettled{user: buyer}
├── TradeSettled{user: seller}
├── FeeReceived{REVENUE, from: buyer}
└── FeeReceived{REVENUE, from: seller}
```

**资产守恒验证**:
```
buyer.debit(quote)  + buyer.credit(base - fee)   = 0  ✓
seller.debit(base)  + seller.credit(quote - fee) = 0  ✓
revenue.credit(buyer_fee + seller_fee)           = fee_total ✓

Σ 变动 = 0 (可审计)
```

---

## 4. 实现架构

### 4.1 TDengine Schema

```sql
CREATE STABLE balance_events (
    ts          TIMESTAMP,
    event_type  TINYINT,
    trade_id    BIGINT,
    debit_asset INT,
    debit_amt   BIGINT,
    credit_asset INT,
    credit_amt  BIGINT,
    fee         BIGINT,
    is_maker    BOOL
) TAGS (account_id BIGINT);
```

### 4.2 查询模式

```sql
-- 用户手续费历史
SELECT ts, trade_id, fee FROM user_1001_events WHERE event_type = 1;

-- 平台收入统计
SELECT fee_asset, SUM(credit_amt) FROM revenue_events GROUP BY fee_asset;
```

### 4.3 消费者架构

```
BalanceEventBatch
├──▶ TDengine Writer (批量写入)
├──▶ WebSocket Router (按 user_id 推送)
└──▶ Kafka Publisher (可选)
```

---

## 5. API 变更

### 5.1 Trade 响应

```json
{
  "trade_id": "12345",
  "fee": "0.002",
  "fee_asset": "BTC",
  "role": "TAKER"
}
```

### 5.2 WebSocket 推送

```json
{
  "e": "trade.update",
  "data": {"trade_id": "12345", "fee": "0.002", "is_maker": false}
}
```

---

## 6. 边界情况

| 情况 | 处理 |
|------|------|
| Fee 四舍五入为 0 | 最小 fee = 1 |
| 零费率交易对 | 允许 `maker_fee = 0` |

---

## 7. 验证计划

- [ ] 手续费计算准确性测试
- [ ] E2E 交易手续费扣除
- [ ] API/WS 返回手续费信息
- [ ] 资产守恒审计

---

<br>
<div align="right"><a href="#-chinese">↑ 返回顶部</a></div>
<br>
