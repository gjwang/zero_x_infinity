# 0x10-a: ID 规范与账户结构设计

> **📅 状态**: 设计中
> **核心目标**: 定义系统中所有关键 ID 的生成规则和账户的基础数据结构。

---

## 1. ID 生成规则

### 1.1 User ID (`u64`)
- **语义**: 全局唯一的用户标识符。
- **生成策略**: 自增序列 或 Snowflake/ULID (未来支持分布式)。
- **初始值**: `1001` (保留 1-1000 给系统账户)。

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

| Account ID | User ID | Type | 用途 |
| :--- | :--- | :--- | :--- |
| `0x0101` | `1` | Funding | 系统手续费收入账户 |
| `0x0102` | `1` | Spot | 系统 Spot 账户 (可选) |

---

> 此设计待确认后，将同步更新至 `src/core_types.rs` 与 `src/account/mod.rs`。
