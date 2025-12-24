# 0x0A-b: ID Specification & Account Structure

<h3>
  <a href="#-english">🇺🇸 English</a>
  &nbsp;&nbsp;&nbsp;|&nbsp;&nbsp;&nbsp;
  <a href="#-chinese">🇨🇳 中文</a>
</h3>

<div id="-english"></div>

## 🇺🇸 English

> **📅 Status**: Design Phase
> **Core Objective**: Define ID generation rules and account data structures.

---

## 1. ID Generation Rules

### 1.1 User ID (`u64`)
- **Semantics**: Global unique user identifier.
- **Strategy**: Auto-increment or Snowflake/ULID (for future distributed support).
- **Initial Value**: `1001` (0-1000 reserved for system accounts).

### 1.2 Asset ID (`u32`)
- **Semantics**: Asset identifier (e.g., BTC=1, USDT=2).
- **Strategy**: Sequential allocation starting from `1`.
- **Purpose**: Maintain O(1) array indexing performance.

### 1.3 Symbol ID (`u32`)
- **Semantics**: Trading Pair identifier (e.g., BTC_USDT=1).
- **Strategy**: Sequential allocation starting from `1`.

### 1.4 Account ID (`u64`)
- **Semantics**: User's sub-account identifier (distinguishing Funding vs Spot).
- **Strategy**: Composite ID (High bits for User, Low bits for Type).
  ```
  Account ID = (user_id << 8) | account_type
  ```
  - `account_type = 0x01` -> Funding
  - `account_type = 0x02` -> Spot

### 1.5 Order ID / Trade ID (`u64`)
- **Semantics**: Unique identifier for orders/trades within the Matching Engine.
- **Strategy**: Global atomic increment.

---

## 2. Core Data Structures

### 2.1 `AccountType` Enum
```rust
#[repr(u8)]
pub enum AccountType {
    Funding = 0x01,
    Spot    = 0x02,
}
```

### 2.2 `Account` Struct (Conceptual)
```rust
pub struct Account {
    pub account_id: u64,      // Composite ID
    pub user_id: u64,
    pub account_type: AccountType,
    pub balances: HashMap<AssetId, Balance>,
    pub created_at: u64,
    pub status: AccountStatus,
}
```

---

## 3. System Reserved Accounts

| User ID | Purpose | Description |
| :--- | :--- | :--- |
| `0` | REVENUE | Platform fee income account |
| `1` | INSURANCE | Insurance fund (future) |
| `2-999` | Reserved | For future system use |
| `1000` | Reserved | Boundary marker |

---

> This design will be updated to `src/core_types.rs` and `src/account/mod.rs` upon confirmation.

<br>
<div align="right"><a href="#-english">↑ Back to Top</a></div>
<br>

---

<div id="-chinese"></div>

## 🇨🇳 中文

> **📅 状态**: 设计中
> **核心目标**: 定义系统中所有关键 ID 的生成规则和账户的基础数据结构。

---

## 1. ID 生成规则

### 1.1 User ID (`u64`)
- **语义**: 全局唯一的用户标识符。
- **生成策略**: 自增序列 或 Snowflake/ULID (未来支持分布式)。
- **初始值**: `1001` (0-1000 保留给系统账户)。

### 1.2 Asset ID (`u32`)
- **语义**: 资产标识符（如 BTC=1, USDT=2）。
- **生成策略**: 顺序分配，从 `1` 开始。
- **目的**: 保持 O(1) 数组索引性能。

### 1.3 Symbol ID (`u32`)
- **语义**: 交易对标识符（如 BTC/USDT=1）。
- **生成策略**: 顺序分配，从 `1` 开始。

### 1.4 Account ID (`u64`)
- **语义**: 用户的子账户标识（区分 Funding 与 Spot）。
- **生成策略**: 复合 ID，高位用户，低位类型。
  ```
  Account ID = (user_id << 8) | account_type
  ```
  - `account_type = 0x01` -> Funding
  - `account_type = 0x02` -> Spot

### 1.5 Order ID / Trade ID (`u64`)
- **语义**: 撮合引擎内的订单/成交唯一标识。
- **生成策略**: 全局原子递增。

---

## 2. 核心数据结构

### 2.1 `AccountType` 枚举
```rust
#[repr(u8)]
pub enum AccountType {
    Funding = 0x01,
    Spot    = 0x02,
}
```

### 2.2 `Account` 结构体 (概念)
```rust
pub struct Account {
    pub account_id: u64,      // 复合 ID
    pub user_id: u64,
    pub account_type: AccountType,
    pub balances: HashMap<AssetId, Balance>,
    pub created_at: u64,
    pub status: AccountStatus,
}
```

---

## 3. 系统保留账户

| User ID | 用途 | 说明 |
| :--- | :--- | :--- |
| `0` | REVENUE | 平台手续费收入账户 |
| `1` | INSURANCE | 保险基金 (未来) |
| `2-999` | 保留 | 未来系统用途 |
| `1000` | 保留 | 边界标记 |

---

> 此设计待确认后，将同步更新至 `src/core_types.rs` 与 `src/account/mod.rs`。
