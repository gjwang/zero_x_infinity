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

### 1.1 Goal

Implement **Maker/Taker fee model** for trade execution. Fees are the primary revenue source for exchanges.

### 1.2 Key Concepts

| Term | Definition |
|------|------------|
| **Maker** | Order that adds liquidity (resting on orderbook) |
| **Taker** | Order that removes liquidity (matches immediately) |
| **Fee Rate** | Percentage of trade value charged |
| **bps** | Basis points (1 bps = 0.01% = 0.0001) |

---

## 2. Fee Model Design

### 2.1 Standard Rates

| Role | Rate (bps) | Rate (%) | Example: 100 USDT trade |
|------|-----------|----------|------------------------|
| **Maker** | 10 | 0.10% | 0.10 USDT |
| **Taker** | 20 | 0.20% | 0.20 USDT |

> **Industry Reference**: Binance Spot (VIP 0): Maker 0.10%, Taker 0.10%

### 2.2 Fee Collection Point

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

---

## 3. Data Model

### 3.1 Symbol Fee Configuration

```sql
ALTER TABLE symbols_tb ADD COLUMN maker_fee_bps SMALLINT NOT NULL DEFAULT 10;
ALTER TABLE symbols_tb ADD COLUMN taker_fee_bps SMALLINT NOT NULL DEFAULT 20;
```

### 3.2 Trade Record Enhancement

Existing `Trade` struct already has:
- `fee: u64` - Amount of fee charged (in received asset's scaled units)
- `role: u8` - 0=Maker, 1=Taker

### 3.3 Fee Ledger (New Table)

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

---

## 4. Implementation Points

### 4.1 Symbol Configuration

**File**: `src/exchange_info/symbol/models.rs`

```rust
pub struct Symbol {
    // ... existing fields ...
    pub maker_fee_bps: u16,  // e.g., 10 = 0.10%
    pub taker_fee_bps: u16,  // e.g., 20 = 0.20%
}
```

### 4.2 Fee Calculation

**File**: `src/matching.rs` (in `process_match()`)

```rust
fn calculate_fee(amount: u64, fee_bps: u16) -> u64 {
    // amount * fee_bps / 10000, with rounding
    (amount as u128 * fee_bps as u128 / 10000) as u64
}
```

### 4.3 Settlement Adjustment

**File**: `src/pipeline_services.rs`

When crediting received asset:
```rust
let received_amount = trade_qty_or_value;
let fee = calculate_fee(received_amount, fee_bps);
let net_amount = received_amount - fee;

// Credit net_amount to user
// Record fee in fee_ledger
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
