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

### 2.4 Lock Amount Must Include Max Fee

**关键设计**: 下单锁定金额必须包含最大潜在手续费，避免成交后余额不足。

```rust
// 下单时计算锁定金额
let base_cost = match side {
    Buy => price * qty / qty_unit,  // USDT
    Sell => qty,                     // BTC
};

// 获取用户最大费率 (Taker 费率)
let max_fee_rate = get_user_fee_rate(user_id, symbol_id, is_taker: true);
let max_fee = base_cost * max_fee_rate / 1_000_000;

// 实际锁定 = 基础成本 + 最大潜在费用
let lock_amount = base_cost + max_fee;
user.lock(lock_amount);
```

**成交后处理**:
- 实际费用 <= 预留费用
- 多余预留金额解冻退回用户可用余额

> **Why 必须预留？**
> - 避免成交后付不起手续费
> - 无需产生欠债记录
> - 用户体验更好

### 2.5 Fee Calculation Timing

关键问题：费用在哪里计算和扣除？

```
ME (Match) --> Trade{role} --> Settlement (Calculate Fee, Deduct, Credit Net)
```

> **Why 在 Settlement 层计算**（而不是 ME）？
> 1. **ME 保持高性能**: 撮合引擎只关注 price-time priority
> 2. **费用可配置性**: 不同用户可能有不同 VIP 等级或折扣
> 3. **复杂场景扩展**: BNB 抵扣、返佣等逻辑不影响 ME

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

### 3.4 Fee Ledger (New Table)

```sql
CREATE TABLE fee_ledger_tb (
    id BIGSERIAL PRIMARY KEY,
    trade_id BIGINT NOT NULL,
    user_id BIGINT NOT NULL,
    symbol_id INTEGER NOT NULL,
    asset_id INTEGER NOT NULL,      -- Asset in which fee was collected
    fee_amount DECIMAL(36,18) NOT NULL,
    role SMALLINT NOT NULL,         -- 0=Maker, 1=Taker
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_fee_ledger_user ON fee_ledger_tb(user_id);
CREATE INDEX idx_fee_ledger_symbol ON fee_ledger_tb(symbol_id);
```

### 3.5 Double-Entry Fee Architecture (Future: TigerBeetle)

从 `indexer-blockdata-rs` 的 UBSCORE_TIGERBEETLE.md 借鉴的账户体系：

```
┌─────────────────────────────────────────────────────────────┐
│                    ACCOUNT HIERARCHY                        │
├─────────────────────────────────────────────────────────────┤
│ User Account     │  UserID | AssetID  │ 用户余额           │
│ Omnibus Account  │  0xFF.. | AssetID  │ 交易所冷钱包(负债) │
│ Holding Account  │  0xFE.. | AssetID  │ 订单冻结中间账户   │
│ Revenue Account  │  0xEE.. | AssetID  │ 手续费收入(权益)   │
└─────────────────────────────────────────────────────────────┘
```

**Atomic Settlement Batch** (TigerBeetle LINKED flag):

| Idx | Operation | From | To | Asset | Description |
|-----|-----------|------|----|-------|-------------|
| 1 | POST Buyer | - | - | USDT | 解冻买方资金 |
| 2 | POST Seller | - | - | BTC | 解冻卖方资金 |
| 3 | Principal | Seller → Buyer | - | BTC | 转移基础资产 |
| 4 | Principal | Buyer → Seller | - | USDT | 转移报价资产 |
| 5 | **Fee** | Buyer → Revenue | - | BTC | 买方手续费 |
| 6 | **Fee** | Seller → Revenue | - | USDT | 卖方手续费 |

> **Why Double-Entry?**
> - **审计性**: `Σ(User Balances) + Σ(Revenue) == Omnibus Balance`
> - **透明性**: 费用是显式转账，不是隐式扣除
> - **原子性**: TigerBeetle LINKED flag 确保要么全成功要么全回滚

## 4. Implementation Architecture

### 4.1 Data Flow Diagram

```
┌──────────────────────────────────────────────────────────────────┐
│                        MATCHING ENGINE                           │
│                                                                  │
│  Order A (Taker) ──┐                                             │
│                    ├──▶ Match ──▶ Trade{fee, role} ──┬──▶ ME Result
│  Order B (Maker) ──┘                                 │           │
│                                                      │           │
│             SymbolInfo.taker_fee_bps ───────────────▶│           │
│             SymbolInfo.maker_fee_bps ───────────────▶│           │
└──────────────────────────────────────────────────────────────────┘
                                                       │
                                                       ▼
┌──────────────────────────────────────────────────────────────────┐
│                        SETTLEMENT                                │
│                                                                  │
│  Trade.fee ──▶ Calculate net_amount ──▶ Credit to user          │
│             ──▶ Record fee in fee_ledger_tb                     │
└──────────────────────────────────────────────────────────────────┘
```

### 4.2 SymbolInfo Enhancement

**File**: `src/symbol_manager.rs`

```rust
#[derive(Debug, Clone)]
pub struct SymbolInfo {
    pub symbol: String,
    pub symbol_id: u32,
    pub base_asset_id: u32,
    pub quote_asset_id: u32,
    pub price_decimal: u32,
    pub price_display_decimal: u32,
    pub base_decimals: u32,
    // NEW: Fee configuration
    pub maker_fee_bps: u16,  // e.g., 10 = 0.10%
    pub taker_fee_bps: u16,  // e.g., 20 = 0.20%
}
```

### 4.3 Trade Struct (Existing, Use Placeholder)

**File**: `src/models.rs`

```rust
// Already exists - just populate during matching:
pub struct Trade {
    // ... existing fields ...
    pub fee: u64,   // Amount of fee (in received asset's scaled units)
    pub role: u8,   // 0=Maker, 1=Taker
}
```

### 4.4 Fee Calculation Function

**File**: `src/engine.rs` (or new `src/fee.rs`)

```rust
/// Calculate fee amount from gross amount
/// 
/// # Arguments
/// - `amount`: Gross amount in scaled units
/// - `fee_bps`: Fee rate in basis points (10000 = 100%)
///
/// # Returns
/// Fee amount in same scaled units
#[inline]
pub fn calculate_fee(amount: u64, fee_bps: u16) -> u64 {
    // Use u128 to prevent overflow
    let fee = (amount as u128) * (fee_bps as u128) / 10000;
    fee as u64
}

/// Calculate fee with minimum (avoid 0 fee on small trades)
#[inline]
pub fn calculate_fee_with_min(amount: u64, fee_bps: u16, min_fee: u64) -> u64 {
    let fee = calculate_fee(amount, fee_bps);
    fee.max(min_fee)
}
```

### 4.5 Config Loading

**File**: `src/csv_io.rs` (add fee columns to fixtures)

**fixtures/symbols_config.csv** (add columns):
```csv
symbol_id,symbol,base_asset_id,quote_asset_id,price_decimal,price_display_decimal,maker_fee_bps,taker_fee_bps
1,BTC_USDT,1,2,6,2,10,20
```

### 4.6 PostgreSQL Migration

**File**: `migrations/006_add_fee_config.sql`

```sql
-- Add fee columns to symbols_tb
ALTER TABLE symbols_tb ADD COLUMN maker_fee_bps SMALLINT NOT NULL DEFAULT 10;
ALTER TABLE symbols_tb ADD COLUMN taker_fee_bps SMALLINT NOT NULL DEFAULT 20;

-- Fee ledger table
CREATE TABLE fee_ledger_tb (
    id BIGSERIAL PRIMARY KEY,
    trade_id BIGINT NOT NULL,
    user_id BIGINT NOT NULL,
    symbol_id INTEGER NOT NULL,
    asset_id INTEGER NOT NULL,
    fee_amount DECIMAL(30,8) NOT NULL,
    role SMALLINT NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_fee_ledger_user ON fee_ledger_tb(user_id, created_at DESC);
CREATE INDEX idx_fee_ledger_symbol ON fee_ledger_tb(symbol_id);
```

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

### 1.1 目标

实现 **Maker/Taker 手续费模型**。手续费是交易所的主要收入来源。

### 1.2 核心概念

| 术语 | 定义 |
|------|------|
| **Maker** | 挂单方 (订单在盘口等待成交) |
| **Taker** | 吃单方 (订单立即匹配成交) |
| **费率** | 交易额的百分比 |
| **bps** | 基点 (1 bps = 0.01% = 0.0001) |

---

## 2. 费率模型设计

### 2.1 标准费率

| 角色 | 费率 (bps) | 费率 (%) | 示例: 100 USDT 交易 |
|------|-----------|----------|-------------------|
| **Maker** | 10 | 0.10% | 0.10 USDT |
| **Taker** | 20 | 0.20% | 0.20 USDT |

### 2.2 手续费扣除规则

**规则**: 手续费从 **收到的资产** 中扣除，而不是支付的资产。

---

## 3. 数据模型

### 3.1 Symbol 费率配置

```sql
ALTER TABLE symbols_tb ADD COLUMN maker_fee_bps SMALLINT NOT NULL DEFAULT 10;
ALTER TABLE symbols_tb ADD COLUMN taker_fee_bps SMALLINT NOT NULL DEFAULT 20;
```

### 3.2 手续费账本

```sql
CREATE TABLE fee_ledger_tb (
    id BIGSERIAL PRIMARY KEY,
    trade_id BIGINT NOT NULL,
    user_id BIGINT NOT NULL,
    symbol_id INTEGER NOT NULL,
    asset_id INTEGER NOT NULL,
    fee_amount DECIMAL(36,18) NOT NULL,
    role SMALLINT NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW()
);
```

---

## 4. 实现要点

### 4.1 费率计算

```rust
fn calculate_fee(amount: u64, fee_bps: u16) -> u64 {
    (amount as u128 * fee_bps as u128 / 10000) as u64
}
```

### 4.2 结算调整

```rust
let net_amount = received_amount - fee;
// 记账: 实际到账 = 毛收入 - 手续费
```

---

## 5. 验证计划

- [ ] 手续费计算准确性测试
- [ ] E2E 交易手续费扣除测试
- [ ] 手续费账本对账
- [ ] API/WS 返回手续费信息

---
