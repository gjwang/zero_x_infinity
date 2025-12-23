# 0x0B Funding & Transfer: Fund System

<h3>
  <a href="#-english">🇺🇸 English</a>
  &nbsp;&nbsp;&nbsp;|&nbsp;&nbsp;&nbsp;
  <a href="#-chinese">🇨🇳 中文</a>
</h3>

<div id="-english"></div>

## 🇺🇸 English

> **📅 Status**: 📝 **Draft**
> **Branch**: `0x0B-funding-transfer`
> **Date**: 2025-12-23

---

## 1. Overview

### 1.1 Objectives

Build a complete fund management system supporting:
*   **Deposit**: External funds entering the exchange.
*   **Withdraw**: Funds leaving the exchange.
*   **Transfer**: Internal fund movement between accounts.

### 1.2 Design Principles

| Principle | Description |
|-----------|-------------|
| **Integrity** | Complete audit log for every change |
| **Double Entry** | Debits = Credits, funds conserved |
| **Async** | Deposits/Withdrawals are async, Transfers sync |
| **Idempotency** | No duplicate execution |
| **Auditability** | All actions traceable |

---

## 2. Account Model

### 2.1 Account Types

```
┌─────────────────────────────────────────────────────────────┐
│                    User Account System                      │
├─────────────────────────────────────────────────────────────┤
│  Main Account                                               │
│  ├── Spot Account (Matching)                                │
│  ├── Funding Account (Deposit/Withdraw)                     │
│  └── Margin Account (Future: Leverage)                      │
└─────────────────────────────────────────────────────────────┘
```

### 2.2 Schema (PostgreSQL)

```sql
CREATE TYPE account_type AS ENUM ('spot', 'funding', 'margin');

CREATE TABLE sub_accounts_tb (
    sub_account_id  BIGSERIAL PRIMARY KEY,
    user_id         BIGINT NOT NULL REFERENCES users_tb(user_id),
    account_type    account_type NOT NULL,
    created_at      TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(user_id, account_type)
);

CREATE TABLE sub_balances_tb (
    balance_id      BIGSERIAL PRIMARY KEY,
    sub_account_id  BIGINT NOT NULL REFERENCES sub_accounts_tb(sub_account_id),
    asset_id        INTEGER NOT NULL REFERENCES assets_tb(asset_id),
    available       BIGINT NOT NULL DEFAULT 0,
    frozen          BIGINT NOT NULL DEFAULT 0,
    UNIQUE(sub_account_id, asset_id)
);
```

---

## 3. Deposit Flow

1.  User gets address.
2.  User transfers funds to exchange address.
3.  **Indexer** monitors chain.
4.  Wait for **Confirmations**.
5.  Credit to **Funding Account**.

### 3.1 Deposit Table

```sql
CREATE TYPE deposit_status AS ENUM ('pending', 'confirming', 'completed', 'failed');

CREATE TABLE deposits_tb (
    deposit_id      BIGSERIAL PRIMARY KEY,
    user_id         BIGINT NOT NULL REFERENCES users_tb(user_id),
    asset_id        INTEGER NOT NULL REFERENCES assets_tb(asset_id),
    amount          BIGINT NOT NULL,
    tx_hash         VARCHAR(128) UNIQUE,
    status          deposit_status NOT NULL DEFAULT 'pending',
    ...
);
```

---

## 4. Withdrawal Flow

1.  User Request -> Review -> Sign -> Broadcast -> Complete.

### 4.1 Withdrawal Table

```sql
CREATE TYPE withdraw_status AS ENUM ('pending', 'risk_review', 'processing', 'completed', ...);

CREATE TABLE withdrawals_tb (
    withdrawal_id   BIGSERIAL PRIMARY KEY,
    user_id         BIGINT NOT NULL,
    amount          BIGINT NOT NULL,
    fee             BIGINT NOT NULL,
    net_amount      BIGINT NOT NULL,
    status          withdraw_status NOT NULL DEFAULT 'pending',
    ...
);
```

### 4.2 Risk Rules

*   Small Amount: Auto-approve (< 500 USDT).
*   Large Amount: Manual Review (>= 10000 USDT).
*   New Address: 24h Delay.

---

## 5. Transfer

### 5.1 Types

*   `funding → spot`: Available for trading.
*   `spot → funding`: Available for withdrawal.
*   `user → user`: Internal transfer.

### 5.2 API Design

`POST /api/v1/private/transfer`

```json
{
    "from_account": "funding",
    "to_account": "spot",
    "asset": "USDT",
    "amount": "100.00"
}
```

---

## 6. Ledger

Complete record of all fund movements.

```sql
CREATE TYPE ledger_type AS ENUM ('deposit', 'withdraw', 'transfer_in', 'trade_buy', ...);

CREATE TABLE ledger_tb (
    ledger_id       BIGSERIAL PRIMARY KEY,
    user_id         BIGINT NOT NULL,
    ledger_type     ledger_type NOT NULL,
    amount          BIGINT NOT NULL,
    balance_after   BIGINT NOT NULL,
    ref_id          BIGINT,
    ...
);
```

---

## 7. Implementation Plan

*   [ ] **Phase 1: DB**: Migrations for sub_accounts, funding, ledger.
*   [ ] **Phase 2: Transfer**: Model + API (Sync).
*   [ ] **Phase 3: Deposit**: Model + Address logic.
*   [ ] **Phase 4: Withdraw**: Model + Risk logic.

---

## 8. Design Decisions

| Decision | Choice | Reason |
|----------|--------|--------|
| Account Model | Sub-accounts | Isolate trading risks |
| Storage | PostgreSQL | ACID Requirement |
| Transfer | Synchronous | User Experience |
| Deposit | Asynchronous | Chain dependency |

<br>
<div align="right"><a href="#-english">↑ Back to Top</a></div>
<br>

---

<div id="-chinese"></div>

## 🇨🇳 中文

> **📅 状态**: 📝 **草稿**
> **分支**: `0x0B-funding-transfer`

---

## 1. 概述

构建完整的资金管理体系，支持充值、提现、划转。

### 1.2 设计原则

账本完整性、双重记账、异步处理、幂等性、可审计。

---

## 2. 账户模型

### 2.1 账户类型

*   **Main Account**: 主账户
*   **Spot Account**: 现货账户 (撮合)
*   **Funding Account**: 资金账户 (充提)

### 2.2 账户结构 (PostgreSQL)

采用 `sub_accounts_tb` 和 `sub_balances_tb` 设计，支持扩展多种账户类型 (margin, futures)。

---

## 3. 充值流程 (Deposit)

监听链上交易 -> 等待确认数 -> 入账 Funding 账户。

### 3.3 确认数规则

BTC 3个确认 (~30min)，ETH 12个确认 (~3min)。

---

## 4. 提现流程 (Withdraw)

用户申请 -> 风控审核 -> 签名广播 -> 完成。

### 4.3 风控规则

小额免审，大额人工复核，新地址延迟提现。

---

## 5. 划转 (Transfer)

### 5.1 划转类型

支持 `funding <-> spot` 互转，及内部用户转账。

### 5.3 API 设计

`POST /api/v1/private/transfer`，需要 Ed25519 签名鉴权。

---

## 6. 资金流水 (Ledger)

记录每一笔资金变动 (`deposit`, `withdraw`, `trade`, `fee`, etc.)，确保可追溯。

---

## 7. 实现计划

*   Phase 1: 数据库 Migration
*   Phase 2: Transfer 功能 (优先)
*   Phase 3: Deposit (P2)
*   Phase 4: Withdraw (P2)

---

## 8. 设计决策

| 决策 | 选择 | 理由 |
|------|------|------|
| 账户模型 | 子账户 | 隔离交易和充提资金 |
| 充提存储 | PostgreSQL | 需要事务 ACID |
| 划转 | 同步 | 低延迟，用户体验好 |
| 充提 | 异步 | 依赖链上确认 |
