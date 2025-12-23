# 0x0B-a 内部划转 (Internal Transfer)

> **📅 状态**: 🔵 **架构设计中**  
> **分支**: `0x0B-a-transfer`  
> **日期**: 2025-12-23

---

## 1. 概述

### 1.1 目标

实现用户账户间的内部资金划转功能：
- **Funding → Spot**: 从资金账户转入现货账户（用于交易）
- **Spot → Funding**: 从现货账户转回资金账户（用于提现）

### 1.2 范围

| 功能 | 本期 | 说明 |
|------|------|------|
| Funding ↔ Spot 划转 | ✅ P1 | 同一用户，账户间转移 |
| 用户间转账 | ❌ P2 | 不同用户间转账 |
| 子账户管理 | ❌ P2 | 创建多个子账户 |

### 1.3 设计原则

| 原则 | 说明 |
|------|------|
| **原子性** | 划转操作要么全部成功，要么全部失败 |
| **同步执行** | 划转立即完成，无需异步等待 |
| **余额验证** | 划转前检查可用余额 |
| **流水记录** | 每笔划转生成完整流水 |
| **幂等性** | 相同请求多次执行结果一致 |

---

## 2. 数据模型

### 2.1 账户类型

```rust
/// 账户类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountType {
    /// 现货账户 - 用于撮合交易
    Spot = 1,
    /// 资金账户 - 用于充提
    Funding = 2,
}
```

### 2.2 划转记录

```rust
/// 划转记录
pub struct Transfer {
    pub transfer_id: i64,
    pub user_id: i64,
    pub asset_id: i32,
    pub from_account: AccountType,
    pub to_account: AccountType,
    pub amount: i64,          // 划转金额 (最小单位)
    pub created_at: DateTime<Utc>,
}
```

### 2.3 数据库设计

```sql
-- 划转记录表
CREATE TABLE transfers_tb (
    transfer_id     BIGSERIAL PRIMARY KEY,
    user_id         BIGINT NOT NULL REFERENCES users_tb(user_id),
    asset_id        INTEGER NOT NULL REFERENCES assets_tb(asset_id),
    from_account    SMALLINT NOT NULL,  -- 1=Spot, 2=Funding
    to_account      SMALLINT NOT NULL,
    amount          BIGINT NOT NULL,
    created_at      TIMESTAMPTZ DEFAULT NOW(),
    
    CHECK (amount > 0),
    CHECK (from_account != to_account),
    CHECK (from_account IN (1, 2)),
    CHECK (to_account IN (1, 2))
);

CREATE INDEX idx_transfers_user ON transfers_tb(user_id);
CREATE INDEX idx_transfers_created ON transfers_tb(created_at);
```

---

## 3. 余额模型扩展

### 3.1 现有余额表扩展

当前 `balances_tb` 服务于 Spot 账户。需要扩展以支持多账户：

**方案 A: 添加 account_type 列** ✅ 选择

```sql
-- 添加账户类型列 (默认 Spot)
ALTER TABLE balances_tb ADD COLUMN account_type SMALLINT NOT NULL DEFAULT 1;

-- 更新唯一约束
ALTER TABLE balances_tb DROP CONSTRAINT balances_tb_user_id_asset_id_key;
ALTER TABLE balances_tb ADD CONSTRAINT balances_tb_unique 
    UNIQUE(user_id, asset_id, account_type);
```

**方案 B: 创建独立 funding_balances_tb** ❌

优先选择方案 A，复用现有逻辑。

### 3.2 余额结构

```rust
/// 账户余额
pub struct AccountBalance {
    pub user_id: i64,
    pub asset_id: i32,
    pub account_type: AccountType,
    pub available: i64,
    pub frozen: i64,
    pub version: i32,
}
```

---

## 4. API 设计

### 4.1 划转接口

```
POST /api/v1/private/transfer
Authorization: ZXINF v1.<api_key>.<ts_nonce>.<signature>
Content-Type: application/json

Request:
{
    "from": "funding",      // "spot" | "funding"
    "to": "spot",           // "spot" | "funding"
    "asset": "USDT",        // 资产名称
    "amount": "100.00"      // 划转金额 (字符串)
}

Response (成功):
{
    "code": 0,
    "data": {
        "transfer_id": "12345678",
        "from": "funding",
        "to": "spot",
        "asset": "USDT",
        "amount": "100.00",
        "timestamp": 1703318400000
    }
}

Response (失败):
{
    "code": 5001,
    "error": "INSUFFICIENT_BALANCE",
    "message": "Insufficient balance in funding account"
}
```

### 4.2 错误码

| 错误码 | 名称 | 说明 |
|--------|------|------|
| 5001 | InsufficientBalance | 余额不足 |
| 5002 | InvalidAccount | 无效的账户类型 |
| 5003 | SameAccount | 源和目标账户相同 |
| 5004 | InvalidAsset | 无效的资产 |
| 5005 | InvalidAmount | 无效的金额 |
| 5006 | TransferFailed | 划转失败 |

### 4.3 查询余额接口

```
GET /api/v1/private/balances?account=funding
Authorization: ZXINF v1.<api_key>.<ts_nonce>.<signature>

Response:
{
    "code": 0,
    "data": [
        {
            "asset": "USDT",
            "available": "1000.00",
            "frozen": "0.00",
            "account": "funding"
        }
    ]
}
```

---

## 5. 业务流程

### 5.1 划转流程

```
┌─────────────────────────────────────────────────────────────┐
│                     划转请求处理流程                          │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  1. 验证请求参数                                             │
│     ├── from/to 有效且不同                                   │
│     ├── asset 存在                                          │
│     └── amount > 0                                          │
│                                                             │
│  2. 开启数据库事务                                           │
│                                                             │
│  3. 锁定源账户余额 (SELECT FOR UPDATE)                       │
│                                                             │
│  4. 检查可用余额 >= amount                                   │
│                                                             │
│  5. 扣减源账户: available -= amount                          │
│                                                             │
│  6. 增加目标账户: available += amount                        │
│     (如不存在则创建)                                         │
│                                                             │
│  7. 插入划转记录                                             │
│                                                             │
│  8. 提交事务                                                 │
│                                                             │
│  9. 返回成功                                                 │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### 5.2 并发控制

使用 PostgreSQL 行级锁确保并发安全：

```sql
-- 锁定源账户余额行
SELECT available, version 
FROM balances_tb 
WHERE user_id = $1 AND asset_id = $2 AND account_type = $3
FOR UPDATE;
```

---

## 6. 服务端验证流程

```rust
/// 划转请求处理
pub async fn handle_transfer(
    db: &Database,
    user_id: i64,
    req: TransferRequest,
) -> Result<TransferResponse, TransferError> {
    // 1. 解析并验证参数
    let from_account = AccountType::from_str(&req.from)?;
    let to_account = AccountType::from_str(&req.to)?;
    
    if from_account == to_account {
        return Err(TransferError::SameAccount);
    }
    
    let asset = db.get_asset_by_name(&req.asset).await?
        .ok_or(TransferError::InvalidAsset)?;
    
    let amount = parse_amount(&req.amount, asset.decimals)?;
    if amount <= 0 {
        return Err(TransferError::InvalidAmount);
    }
    
    // 2. 执行划转 (带事务)
    let transfer = db.execute_transfer(
        user_id,
        asset.asset_id,
        from_account,
        to_account,
        amount,
    ).await?;
    
    // 3. 返回结果
    Ok(TransferResponse {
        transfer_id: transfer.transfer_id.to_string(),
        from: req.from,
        to: req.to,
        asset: req.asset,
        amount: req.amount,
        timestamp: transfer.created_at.timestamp_millis(),
    })
}
```

---

## 7. 实现计划

### 7.1 开发清单

#### Phase 1: 数据库层

| # | 任务 | 输出文件 | 验收标准 |
|---|------|----------|----------|
| 1.1 | 扩展 balances_tb | `migrations/003_account_type.sql` | account_type 列存在 |
| 1.2 | 创建 transfers_tb | `migrations/004_transfers.sql` | 表创建成功 |

#### Phase 2: 核心模块

| # | 任务 | 输出文件 | 验收标准 |
|---|------|----------|----------|
| 2.1 | AccountType 枚举 | `src/funding/types.rs` | 序列化正确 |
| 2.2 | Transfer 模型 | `src/funding/transfer.rs` | CRUD |
| 2.3 | 划转事务逻辑 | `src/funding/service.rs` | 原子执行 |
| 2.4 | 错误码定义 | `src/funding/error.rs` | 5001-5006 |

#### Phase 3: API 集成

| # | 任务 | 输出文件 | 验收标准 |
|---|------|----------|----------|
| 3.1 | Transfer 处理器 | `src/gateway/handlers.rs` | POST 可用 |
| 3.2 | Balances 查询扩展 | `src/gateway/handlers.rs` | account 参数 |
| 3.3 | 路由注册 | `src/gateway/mod.rs` | 路径正确 |

#### Phase 4: 测试验证

| # | 任务 | 输出文件 | 验收标准 |
|---|------|----------|----------|
| 4.1 | 单元测试 | `src/funding/tests.rs` | 覆盖主要场景 |
| 4.2 | 集成测试 | `scripts/test_transfer.py` | E2E 通过 |
| 4.3 | 并发测试 | 同上 | 无竞态条件 |

### 7.2 关键数据结构

```rust
// Request/Response
pub struct TransferRequest {
    pub from: String,       // "spot" | "funding"
    pub to: String,
    pub asset: String,
    pub amount: String,
}

pub struct TransferResponse {
    pub transfer_id: String,
    pub from: String,
    pub to: String,
    pub asset: String,
    pub amount: String,
    pub timestamp: i64,
}
```

### 7.3 验证 Checklist

- [ ] POST /api/v1/private/transfer 可用
- [ ] funding → spot 划转成功
- [ ] spot → funding 划转成功
- [ ] 余额不足返回 5001
- [ ] 同账户划转返回 5003
- [ ] 划转记录正确插入
- [ ] 并发划转无竞态

---

## 8. 设计决策

| 决策 | 选择 | 理由 |
|------|------|------|
| 余额表扩展 | 方案 A (添加列) | 复用现有逻辑，改动小 |
| 并发控制 | SELECT FOR UPDATE | PostgreSQL 行级锁，简单可靠 |
| 金额格式 | 字符串 | 避免浮点精度问题 |
| 执行方式 | 同步 | 低延迟，用户体验好 |

---

## 9. 安全考虑

| 风险 | 缓解措施 |
|------|----------|
| 并发竞态 | 行级锁 + 事务 |
| 余额溢出 | BIGINT + CHECK 约束 |
| 重复请求 | 幂等性设计 (考虑添加 request_id) |
| 未授权访问 | Ed25519 签名验证 |

---

**状态**: 等待架构审核
