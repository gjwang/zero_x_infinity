# 0x0A 账户体系: 安全鉴权 (Account & Auth)

> **📅 状态**: 架构设计完成，待 Gateway 集成  
> **分支**: `0x10-productization-core`  
> **日期**: 2025-12-22

---

## 1. 概述

Phase 0x0A 建立了基于 PostgreSQL 的账户管理系统，为交易系统提供用户、资产和交易对的配置管理。这是 Part II 产品化阶段的第一步，为后续的鉴权、资金管理和手续费系统奠定基础。

---

## 2. 技术选型

| 组件 | 选型 | 用途 |
|------|------|------|
| **PostgreSQL 18** | 关系型数据库 | 用户、资产、交易对配置 |
| **sqlx** | Rust 异步驱动 | 编译时 SQL 检查 + 异步查询 |
| **Docker Compose** | 容器编排 | PostgreSQL + TDengine 统一管理 |

---

## 3. 数据库表结构

### 3.1 users 表

用户账户信息表。

| 列 | 类型 | 说明 |
|---|---|---|
| `user_id` | BIGSERIAL | 主键，自增 |
| `username` | VARCHAR(64) | 唯一用户名 |
| `email` | VARCHAR(128) | 邮箱（可选）|
| `status` | SMALLINT | 0=disabled, 1=active |
| `user_flags` | INT | 权限位标志 |
| `created_at` | TIMESTAMPTZ | 创建时间 |
| `updated_at` | TIMESTAMPTZ | 更新时间 |

**user_flags 位定义**:
```
0x01 = can_login       # 允许登录
0x02 = can_trade       # 允许交易
0x04 = can_withdraw    # 允许提现
0x08 = can_api_access  # 允许 API 访问
0x10 = is_vip          # VIP 用户
0x20 = is_kyc_verified # KYC 认证
```

默认值: `15` (0x0F) = login + trade + withdraw + api

### 3.2 assets 表

资产配置表（BTC, USDT, ETH 等）。

| 列 | 类型 | 说明 |
|---|---|---|
| `asset_id` | SERIAL | 主键，自增 |
| `asset` | VARCHAR(16) | 资产代码（唯一）|
| `name` | VARCHAR(64) | 全称 |
| `decimals` | SMALLINT | 精度（8 for BTC, 6 for USDT）|
| `status` | SMALLINT | 0=disabled, 1=active |
| `asset_flags` | INT | 权限位标志 |
| `created_at` | TIMESTAMPTZ | 创建时间 |

**asset_flags 位定义**:
```
0x01 = can_deposit     # 允许充值
0x02 = can_withdraw    # 允许提现
0x04 = can_trade       # 允许交易
0x08 = is_stable_coin  # 稳定币标记
```

默认值: `7` (0x07) = deposit + withdraw + trade

### 3.3 symbols 表

交易对配置表（BTC_USDT 等）。

| 列 | 类型 | 说明 |
|---|---|---|
| `symbol_id` | SERIAL | 主键，自增 |
| `symbol` | VARCHAR(32) | 交易对名称（唯一）|
| `base_asset_id` | INT | 基础资产 ID（外键）|
| `quote_asset_id` | INT | 计价资产 ID（外键）|
| `price_decimals` | SMALLINT | 价格精度 |
| `qty_decimals` | SMALLINT | 数量精度 |
| `min_qty` | BIGINT | 最小下单量（scaled）|
| `status` | SMALLINT | 0=offline, 1=online, 2=maintenance |
| `symbol_flags` | INT | 权限位标志 |
| `created_at` | TIMESTAMPTZ | 创建时间 |

**symbol_flags 位定义**:
```
0x01 = is_tradable        # 可交易
0x02 = is_visible         # 可见
0x04 = allow_market_order # 允许市价单
0x08 = allow_limit_order  # 允许限价单
```

默认值: `15` (0x0F) = 全部功能

---

## 4. 代码结构

### 4.1 模块组织

```
src/account/
├── mod.rs           # 模块导出
├── db.rs            # PostgreSQL 连接池
├── models.rs        # User, Asset, Symbol 数据模型
└── repository.rs    # CRUD 操作
```

### 4.2 核心类型

详见源代码：
- `Database` - PostgreSQL 连接池管理
- `User` - 用户模型（含权限检查方法）
- `Asset` - 资产模型（含权限检查方法）
- `Symbol` - 交易对模型（含状态检查方法）

### 4.3 Repository 层

- `UserRepository` - 用户 CRUD 操作
- `AssetManager` - 资产加载和查询
- `SymbolManager` - 交易对加载和查询

---

## 5. 种子数据

系统初始化时自动创建：

- **资产**: BTC, USDT, ETH
- **交易对**: BTC_USDT
- **系统用户**: user_id=1 (system)

---

## 6. 待集成任务

- [ ] 在 `src/lib.rs` 添加 `pub mod account;`
- [ ] Gateway 启动时加载 assets/symbols 到内存
- [ ] 配置文件添加 `postgres_url`
- [ ] 创建 `/api/v1/assets` 端点
- [ ] 创建 `/api/v1/symbols` 端点

---

## 7. 验收标准

```bash
# 启动服务
docker-compose up -d
cargo build
cargo test

# 验证 API
curl http://localhost:8080/api/v1/symbols
curl http://localhost:8080/api/v1/assets
```

---

## 8. 下一步: 0x0A-b API Key 鉴权

实现基于 HMAC-SHA256 的 API Key 鉴权机制。

---

## 9. 相关文档

- [Part II 导读](./Part-II-Introduction.md)
- [ID 规范](./0x0A-a-id-specification.md)
- [API 规范](../standards/api-conventions.md)

---

**最后更新**: 2025-12-22
