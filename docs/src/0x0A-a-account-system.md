# 0x0A-a: Account System

<h3>
  <a href="#-english">🇺🇸 English</a>
  &nbsp;&nbsp;&nbsp;|&nbsp;&nbsp;&nbsp;
  <a href="#-chinese">🇨🇳 中文</a>
</h3>

<div id="-english"></div>

## 🇺🇸 English

> **📦 Code Changes**: [View Diff](https://github.com/gjwang/zero_x_infinity/compare/v0.9-f-integration-test...0x10-productization-core)

This chapter establishes the account infrastructure for the trading system: `exchange_info` module, naming conventions, and database management.

---

## 1. Core Module: exchange_info

### 1.1 Module Structure

```
src/exchange_info/
├── mod.rs           # Module entry
├── validation.rs    # AssetName/SymbolName validation
├── asset/
│   ├── mod.rs
│   ├── models.rs    # Asset struct + asset_flags
│   └── manager.rs   # AssetManager
└── symbol/
    ├── mod.rs
    ├── models.rs    # Symbol struct + symbol_flags
    └── manager.rs   # SymbolManager
```

### 1.2 Core Types

```rust
// Asset
pub struct Asset {
    pub asset_id: i32,
    pub asset: String,     // "BTC", "USDT" (UPPERCASE)
    pub name: String,      // "Bitcoin", "Tether USD"
    pub decimals: i16,     // 8 for BTC, 6 for USDT
    pub status: i16,
    pub asset_flags: i32,  // Permission bits
}

// Symbol
pub struct Symbol {
    pub symbol_id: i32,
    pub symbol: String,    // "BTC_USDT" (UPPERCASE)
    pub base_asset_id: i32,
    pub quote_asset_id: i32,
    pub price_decimals: i16,
    pub qty_decimals: i16,
    pub symbol_flags: i32,
}
```

---

## 2. Naming Convention

| Category | Standard | Example |
|----------|----------|---------|
| Database Name | `_db` suffix | `exchange_info_db` |
| Table Name | `_tb` suffix | `assets_tb`, `symbols_tb` |
| Flags Module | Table name prefix | `asset_flags::`, `symbol_flags::` |
| Codes | UPPERCASE | `BTC`, `BTC_USDT` |

See [Naming Convention Document](../standards/naming-convention.md).

---

## 3. Database Management

### 3.1 Management Script

```bash
# Full Init (Reset + Seed)
python3 scripts/db/manage_db.py init

# Reset Schema Only
python3 scripts/db/manage_db.py reset

# Seed Data Only
python3 scripts/db/manage_db.py seed

# Check Status
python3 scripts/db/manage_db.py status
```

### 3.2 Database Constraints

```sql
-- Enforce UPPERCASE Asset
CONSTRAINT chk_asset_uppercase CHECK (asset = UPPER(asset))

-- Enforce UPPERCASE Symbol
CONSTRAINT chk_symbol_uppercase CHECK (symbol = UPPER(symbol))
```

---

## 4. API Endpoints

### 4.1 GET /api/v1/exchange_info

Returns full exchange information:

```json
{
  "code": 0,
  "data": {
    "assets": [
      {
        "asset_id": 1,
        "asset": "BTC",
        "name": "Bitcoin",
        "decimals": 8,
        "can_deposit": true,
        "can_withdraw": true,
        "can_trade": true
      }
    ],
    "symbols": [
      {
        "symbol_id": 1,
        "symbol": "BTC_USDT",
        "base_asset": "BTC",
        "quote_asset": "USDT",
        "price_decimals": 2,
        "qty_decimals": 8,
        "is_tradable": true,
        "is_visible": true
      }
    ],
    "server_time": 1734897000000
  }
}
```

### 4.2 Other Endpoints

| Endpoint | Description |
|----------|-------------|
| `GET /api/v1/assets` | Asset list only |
| `GET /api/v1/symbols` | Symbol list only |

---

## 5. Verification

### 5.1 Integration Test

```bash
./scripts/test_account_integration.sh
```

Scope:
*   ✅ DB Initialization (Auto reset + seed)
*   ✅ Assets/Symbols/ExchangeInfo API
*   ✅ DB Constraints (Lowercase rejected)
*   ✅ Idempotency

### 5.2 Unit Test

```bash
cargo test --lib
# 150 passed, 0 failed
```

---

## 6. Next Steps

*   [0x0A-b: ID Specification](./0x0A-a-id-specification.md)
*   [0x0A-c: Authentication](./0x0A-b-api-auth.md)

<br>
<div align="right"><a href="#-english">↑ Back to Top</a></div>
<br>

---

<div id="-chinese"></div>

## 🇨🇳 中文

> **📦 代码变更**: [查看 Diff](https://github.com/gjwang/zero_x_infinity/compare/v0.9-f-integration-test...0x10-productization-core)

本章建立交易系统的账户基础设施：exchange_info 模块、命名规范、数据库管理。

---

## 1. 核心模块：exchange_info

### 1.1 模块结构

```
src/exchange_info/
├── mod.rs           # 模块入口
├── validation.rs    # AssetName/SymbolName 验证
├── asset/
│   ├── mod.rs
│   ├── models.rs    # Asset 结构 + asset_flags
│   └── manager.rs   # AssetManager
└── symbol/
    ├── mod.rs
    ├── models.rs    # Symbol 结构 + symbol_flags
    └── manager.rs   # SymbolManager
```

### 1.2 核心类型

```rust
// Asset (资产)
pub struct Asset {
    pub asset_id: i32,
    pub asset: String,     // "BTC", "USDT" (强制大写)
    pub name: String,      // "Bitcoin", "Tether USD"
    pub decimals: i16,     // 8 for BTC, 6 for USDT
    pub status: i16,
    pub asset_flags: i32,  // 权限位
}

// Symbol (交易对)
pub struct Symbol {
    pub symbol_id: i32,
    pub symbol: String,    // "BTC_USDT" (强制大写)
    pub base_asset_id: i32,
    pub quote_asset_id: i32,
    pub price_decimals: i16,
    pub qty_decimals: i16,
    pub symbol_flags: i32,
}
```

---

## 2. 命名规范

| 类别 | 规范 | 示例 |
|------|------|------|
| 数据库名 | `_db` 后缀 | `exchange_info_db` |
| 表名 | `_tb` 后缀 | `assets_tb`, `symbols_tb` |
| Flags 模块 | 表名前缀 | `asset_flags::`, `symbol_flags::` |
| Asset/Symbol 代码 | 强制大写 | `BTC`, `BTC_USDT` |

详见 [命名规范文档](../standards/naming-convention.md)

---

## 3. 数据库管理

### 3.1 Python 管理脚本

```bash
# 完整初始化（重置 + 种子数据）
python3 scripts/db/manage_db.py init

# 只重置 schema（无数据）
python3 scripts/db/manage_db.py reset

# 只添加种子数据
python3 scripts/db/manage_db.py seed

# 查看当前状态
python3 scripts/db/manage_db.py status
```

### 3.2 数据库约束

```sql
-- Asset 强制大写
CONSTRAINT chk_asset_uppercase CHECK (asset = UPPER(asset))

-- Symbol 强制大写
CONSTRAINT chk_symbol_uppercase CHECK (symbol = UPPER(symbol))
```

---

## 4. API 端点

### 4.1 GET /api/v1/exchange_info

返回完整的交易所信息：

```json
{
  "code": 0,
  "data": {
    "assets": [
      {
        "asset_id": 1,
        "asset": "BTC",
        "name": "Bitcoin",
        "decimals": 8,
        "can_deposit": true,
        "can_withdraw": true,
        "can_trade": true
      }
    ],
    "symbols": [
      {
        "symbol_id": 1,
        "symbol": "BTC_USDT",
        "..."
      }
    ],
    "server_time": 1734897000000
  }
}
```

### 4.2 其他端点

| 端点 | 说明 |
|------|------|
| `GET /api/v1/assets` | 仅返回资产列表 |
| `GET /api/v1/symbols` | 仅返回交易对列表 |

---

## 5. 测试验证

### 5.1 集成测试

```bash
./scripts/test_account_integration.sh
```

### 5.2 单元测试

```bash
cargo test --lib
```

---

## 6. 下一步

- [0x0A-b: ID 规范](./0x0A-a-id-specification.md)
- [0x0A-c: 安全鉴权](./0x0A-b-api-auth.md)
