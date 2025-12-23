# 0x0B-a Internal Transfer

<h3>
  <a href="#-english">🇺🇸 English</a>
  &nbsp;&nbsp;&nbsp;|&nbsp;&nbsp;&nbsp;
  <a href="#-chinese">🇨🇳 中文</a>
</h3>

<div id="-english"></div>

## 🇺🇸 English

> **📅 Status**: 🔵 **Designing**
> **Branch**: `0x0B-a-transfer`
> **Date**: 2025-12-23

---

## 1. Overview

### 1.1 Objectives

Implement internal fund transfers between user accounts:
*   **Funding → Spot**: Transfer from Funding Account to Spot Account (for trading).
*   **Spot → Funding**: Transfer from Spot Account to Funding Account (for withdrawal).

### 1.2 Scope

| Feature | Phase | Description |
|---------|-------|-------------|
| Funding ↔ Spot | ✅ P1 | Same user, internal transfer |
| User ↔ User | ❌ P2 | Transfer between different users |
| Sub-accounts | ❌ P2 | Multiple sub-accounts |

### 1.3 Design Principles

*   **Atomicity**: All or nothing.
*   **Synchronous**: Immediate execution.
*   **Balance Check**: Pre-check availability.
*   **Audit**: Complete ledger history.

---

## 2. Data Model

### 2.1 Account Type

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountType {
    Spot = 1,
    Funding = 2,
}
```

### 2.2 Transfer Record

```sql
CREATE TABLE transfers_tb (
    transfer_id     BIGSERIAL PRIMARY KEY,
    user_id         BIGINT NOT NULL REFERENCES users_tb(user_id),
    asset_id        INTEGER NOT NULL REFERENCES assets_tb(asset_id),
    from_account    SMALLINT NOT NULL,  -- 1=Spot, 2=Funding
    to_account      SMALLINT NOT NULL,
    amount          BIGINT NOT NULL,
    created_at      TIMESTAMPTZ DEFAULT NOW(),
    
    CHECK (amount > 0),
    CHECK (from_account != to_account)
);
```

---

## 3. Balance Model Extension

**Option A: Add `account_type` column (Selected)**

```sql
ALTER TABLE balances_tb ADD COLUMN account_type SMALLINT NOT NULL DEFAULT 1;
ALTER TABLE balances_tb ADD CONSTRAINT balances_tb_unique 
    UNIQUE(user_id, asset_id, account_type);
```

---

## 4. API Design

### 4.1 Transfer Endpoint

`POST /api/v1/private/transfer`

```json
Request:
{
    "from": "funding",
    "to": "spot",
    "asset": "USDT",
    "amount": "100.00"
}

Response:
{
    "code": 0,
    "data": {
        "transfer_id": "12345678",
        "status": "completed"
    }
}
```

### 4.2 Error Codes

| Code | Name | Description |
|------|------|-------------|
| 5001 | InsufficientBalance | Balance not enough |
| 5002 | InvalidAccount | Invalid type |
| 5003 | SameAccount | Source == Target |

---

## 5. Business Logic

1.  Validate Params.
2.  Start Transaction.
3.  **Lock Source Balance** (`SELECT FOR UPDATE`).
4.  Check Balance >= Amount.
5.  Debit Source, Credit Target.
6.  Insert Transfer Record.
7.  Commit.

<br>
<div align="right"><a href="#-english">↑ Back to Top</a></div>
<br>

---

<div id="-chinese"></div>

## 🇨🇳 中文

> **📅 状态**: 🔵 **架构设计中**
> **分支**: `0x0B-a-transfer`

---

## 1. 概述

### 1.1 目标

实现用户账户间的内部资金划转功能：
*   **Funding → Spot**: 资金账户转入现货账户。
*   **Spot → Funding**: 现货账户转回资金账户。

### 1.3 设计原则

原子性、同步执行、余额验证、流水记录。

---

## 2. 数据模型

### 2.2 划转记录

```sql
CREATE TABLE transfers_tb (
    transfer_id     BIGSERIAL PRIMARY KEY,
    user_id         BIGINT NOT NULL,
    from_account    SMALLINT NOT NULL,  -- 1=Spot, 2=Funding
    to_account      SMALLINT NOT NULL,
    amount          BIGINT NOT NULL
);
```

---

## 3. 余额模型扩展

**方案 A**: 在现有 `balances_tb` 中添加 `account_type` 列，复用现有逻辑。

---

## 4. API 设计

`POST /api/v1/private/transfer`，需要签名鉴权。

---

## 5. 业务流程

开启事务 -> 锁定源余额 -> 检查余额 -> 扣减源/增加目标 -> 记录流水 -> 提交。
