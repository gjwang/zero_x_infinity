# 0x0B 资金体系: 充提与划转 (Funding & Transfer)

> **📅 状态**: � **草稿**  
> **分支**: `0x0B-funding-transfer`  
> **日期**: 2025-12-23

---

## 1. 概述

### 1.1 目标

构建完整的资金管理体系，支持：
- **充值 (Deposit)**: 外部资金进入交易所
- **提现 (Withdraw)**: 资金从交易所转出
- **划转 (Transfer)**: 账户间内部资金转移

### 1.2 设计原则

| 原则 | 说明 |
|------|------|
| **账本完整性** | 每笔资金变动都有完整的流水记录 |
| **双重记账** | 借贷平衡，任何时刻资金守恒 |
| **异步处理** | 充提为异步，划转可同步 |
| **幂等性** | 重复请求不会重复执行 |
| **可审计** | 所有操作可溯源 |

---

## 2. 账户模型

### 2.1 账户类型

```
┌─────────────────────────────────────────────────────────────┐
│                    用户账户体系                              │
├─────────────────────────────────────────────────────────────┤
│  Main Account (主账户)                                       │
│  ├── Spot Account (现货账户) - 用于撮合                      │
│  ├── Funding Account (资金账户) - 用于充提                   │
│  └── Margin Account (保证金账户) - 未来: 杠杆交易             │
└─────────────────────────────────────────────────────────────┘
```

### 2.2 账户结构 (PostgreSQL)

```sql
-- 子账户类型
CREATE TYPE account_type AS ENUM ('spot', 'funding', 'margin');

-- 子账户表
CREATE TABLE sub_accounts_tb (
    sub_account_id  BIGSERIAL PRIMARY KEY,
    user_id         BIGINT NOT NULL REFERENCES users_tb(user_id),
    account_type    account_type NOT NULL,
    created_at      TIMESTAMPTZ DEFAULT NOW(),
    
    UNIQUE(user_id, account_type)
);

-- 子账户余额 (扩展现有 balances_tb)
CREATE TABLE sub_balances_tb (
    balance_id      BIGSERIAL PRIMARY KEY,
    sub_account_id  BIGINT NOT NULL REFERENCES sub_accounts_tb(sub_account_id),
    asset_id        INTEGER NOT NULL REFERENCES assets_tb(asset_id),
    available       BIGINT NOT NULL DEFAULT 0,
    frozen          BIGINT NOT NULL DEFAULT 0,
    updated_at      TIMESTAMPTZ DEFAULT NOW(),
    version         INTEGER NOT NULL DEFAULT 0,
    
    UNIQUE(sub_account_id, asset_id),
    CHECK (available >= 0),
    CHECK (frozen >= 0)
);
```

---

## 3. 充值流程 (Deposit)

### 3.1 流程图

```
┌─────────┐     ┌─────────┐     ┌─────────┐     ┌─────────┐
│ 用户    │────>│ 获取    │────>│ 转账到  │────>│ 确认    │
│         │     │ 充值地址│     │ 交易所  │     │ 到账    │
└─────────┘     └─────────┘     └─────────┘     └─────────┘
                                     │
                                     ▼
                              ┌─────────────┐
                              │ 链上监控    │
                              │ (Indexer)   │
                              └──────┬──────┘
                                     │
                                     ▼
                              ┌─────────────┐
                              │ 确认数检查  │
                              └──────┬──────┘
                                     │
                                     ▼
                              ┌─────────────┐
                              │ 入账到      │
                              │ Funding账户 │
                              └─────────────┘
```

### 3.2 充值记录表

```sql
CREATE TYPE deposit_status AS ENUM (
    'pending',      -- 等待确认
    'confirming',   -- 确认中 (等待 N 个区块)
    'completed',    -- 已完成
    'failed'        -- 失败
);

CREATE TABLE deposits_tb (
    deposit_id      BIGSERIAL PRIMARY KEY,
    user_id         BIGINT NOT NULL REFERENCES users_tb(user_id),
    asset_id        INTEGER NOT NULL REFERENCES assets_tb(asset_id),
    amount          BIGINT NOT NULL,
    tx_hash         VARCHAR(128) UNIQUE,  -- 链上交易哈希
    from_address    VARCHAR(128),
    to_address      VARCHAR(128) NOT NULL,
    confirmations   INTEGER DEFAULT 0,
    required_confs  INTEGER NOT NULL,     -- 所需确认数
    status          deposit_status NOT NULL DEFAULT 'pending',
    created_at      TIMESTAMPTZ DEFAULT NOW(),
    completed_at    TIMESTAMPTZ,
    
    CHECK (amount > 0)
);

CREATE INDEX idx_deposits_user ON deposits_tb(user_id);
CREATE INDEX idx_deposits_status ON deposits_tb(status);
CREATE INDEX idx_deposits_tx_hash ON deposits_tb(tx_hash);
```

### 3.3 确认数规则

| 资产类型 | 所需确认数 | 预估时间 |
|----------|-----------|----------|
| BTC | 3 | ~30 min |
| ETH | 12 | ~3 min |
| USDT-ERC20 | 12 | ~3 min |
| USDT-TRC20 | 20 | ~1 min |

---

## 4. 提现流程 (Withdraw)

### 4.1 流程图

```
┌─────────┐     ┌─────────┐     ┌─────────┐     ┌─────────┐
│ 用户    │────>│ 提交    │────>│ 风控    │────>│ 审核    │
│ 发起    │     │ 申请    │     │ 检查    │     │ (可选)  │
└─────────┘     └─────────┘     └─────────┘     └─────────┘
                                                     │
                                                     ▼
┌─────────┐     ┌─────────┐     ┌─────────┐     ┌─────────┐
│ 完成    │<────│ 广播    │<────│ 签名    │<────│ 冻结    │
│         │     │ 交易    │     │ 交易    │     │ 资金    │
└─────────┘     └─────────┘     └─────────┘     └─────────┘
```

### 4.2 提现记录表

```sql
CREATE TYPE withdraw_status AS ENUM (
    'pending',       -- 待处理
    'risk_review',   -- 风控审核中
    'manual_review', -- 人工审核中
    'approved',      -- 已批准
    'processing',    -- 处理中 (已签名)
    'broadcasting',  -- 广播中
    'completed',     -- 已完成
    'rejected',      -- 已拒绝
    'failed'         -- 失败
);

CREATE TABLE withdrawals_tb (
    withdrawal_id   BIGSERIAL PRIMARY KEY,
    user_id         BIGINT NOT NULL REFERENCES users_tb(user_id),
    asset_id        INTEGER NOT NULL REFERENCES assets_tb(asset_id),
    amount          BIGINT NOT NULL,        -- 提现金额
    fee             BIGINT NOT NULL,        -- 手续费
    net_amount      BIGINT NOT NULL,        -- 实际到账 = amount - fee
    to_address      VARCHAR(128) NOT NULL,
    memo            VARCHAR(64),            -- 某些链需要 memo
    tx_hash         VARCHAR(128),
    status          withdraw_status NOT NULL DEFAULT 'pending',
    reject_reason   TEXT,
    created_at      TIMESTAMPTZ DEFAULT NOW(),
    completed_at    TIMESTAMPTZ,
    
    CHECK (amount > 0),
    CHECK (fee >= 0),
    CHECK (net_amount > 0),
    CHECK (net_amount = amount - fee)
);

CREATE INDEX idx_withdrawals_user ON withdrawals_tb(user_id);
CREATE INDEX idx_withdrawals_status ON withdrawals_tb(status);
```

### 4.3 风控规则

| 规则 | 条件 | 处理 |
|------|------|------|
| 小额免审 | amount < 500 USDT | 自动处理 |
| 大额审核 | amount >= 10000 USDT | 人工审核 |
| 新地址 | 首次提现到该地址 | 24h 延迟 |
| 频率限制 | 每日 > 5 笔 | 触发风控 |

---

## 5. 划转 (Transfer)

### 5.1 划转类型

| 类型 | 说明 | 同步/异步 |
|------|------|-----------|
| `funding → spot` | 资金账户到交易账户 | 同步 |
| `spot → funding` | 交易账户到资金账户 | 同步 |
| `user → user` | 用户间内部转账 | 同步 |

### 5.2 划转记录表

```sql
CREATE TYPE transfer_type AS ENUM (
    'funding_to_spot',
    'spot_to_funding',
    'user_to_user'
);

CREATE TABLE transfers_tb (
    transfer_id     BIGSERIAL PRIMARY KEY,
    transfer_type   transfer_type NOT NULL,
    from_user_id    BIGINT NOT NULL REFERENCES users_tb(user_id),
    to_user_id      BIGINT NOT NULL REFERENCES users_tb(user_id),
    from_account    account_type NOT NULL,
    to_account      account_type NOT NULL,
    asset_id        INTEGER NOT NULL REFERENCES assets_tb(asset_id),
    amount          BIGINT NOT NULL,
    created_at      TIMESTAMPTZ DEFAULT NOW(),
    
    CHECK (amount > 0)
);

CREATE INDEX idx_transfers_from ON transfers_tb(from_user_id);
CREATE INDEX idx_transfers_to ON transfers_tb(to_user_id);
```

### 5.3 API 设计

```
POST /api/v1/private/transfer
Authorization: ZXINF v1.<api_key>.<ts_nonce>.<signature>

Request:
{
    "from_account": "funding",
    "to_account": "spot",
    "asset": "USDT",
    "amount": "100.00"
}

Response:
{
    "code": 0,
    "data": {
        "transfer_id": "12345",
        "status": "completed"
    }
}
```

---

## 6. 资金流水 (Ledger)

### 6.1 流水类型

```sql
CREATE TYPE ledger_type AS ENUM (
    'deposit',       -- 充值
    'withdraw',      -- 提现
    'withdraw_fee',  -- 提现手续费
    'transfer_in',   -- 转入
    'transfer_out',  -- 转出
    'trade_buy',     -- 买入成交
    'trade_sell',    -- 卖出成交
    'trade_fee',     -- 交易手续费
    'rebate',        -- 返佣
    'adjustment'     -- 人工调账
);
```

### 6.2 流水表

```sql
CREATE TABLE ledger_tb (
    ledger_id       BIGSERIAL PRIMARY KEY,
    user_id         BIGINT NOT NULL REFERENCES users_tb(user_id),
    sub_account_id  BIGINT REFERENCES sub_accounts_tb(sub_account_id),
    asset_id        INTEGER NOT NULL REFERENCES assets_tb(asset_id),
    ledger_type     ledger_type NOT NULL,
    amount          BIGINT NOT NULL,       -- 正数: 增加, 负数: 减少
    balance_after   BIGINT NOT NULL,       -- 变动后余额
    ref_id          BIGINT,                -- 关联 ID (deposit_id, withdrawal_id, etc.)
    ref_type        VARCHAR(32),           -- 关联类型
    memo            TEXT,
    created_at      TIMESTAMPTZ DEFAULT NOW(),
    
    CHECK (balance_after >= 0)
);

CREATE INDEX idx_ledger_user ON ledger_tb(user_id);
CREATE INDEX idx_ledger_user_asset ON ledger_tb(user_id, asset_id);
CREATE INDEX idx_ledger_type ON ledger_tb(ledger_type);
CREATE INDEX idx_ledger_ref ON ledger_tb(ref_type, ref_id);
```

---

## 7. 实现计划

### 7.1 开发清单

#### Phase 1: 数据库层

| # | 任务 | 输出文件 | 验收标准 |
|---|------|----------|----------|
| 1.1 | 子账户表 migration | `migrations/003_sub_accounts.sql` | 表创建成功 |
| 1.2 | 充提划转表 migration | `migrations/004_funding.sql` | 表创建成功 |
| 1.3 | 流水表 migration | `migrations/005_ledger.sql` | 表创建成功 |

#### Phase 2: 划转功能 (同步)

| # | 任务 | 输出文件 | 验收标准 |
|---|------|----------|----------|
| 2.1 | Transfer 模型 | `src/funding/transfer.rs` | 划转成功 |
| 2.2 | Transfer API | `src/gateway/handlers.rs` | POST 可用 |
| 2.3 | 集成测试 | `scripts/test_transfer.py` | 测试通过 |

#### Phase 3: 充值功能 (P2)

| # | 任务 | 输出文件 | 验收标准 |
|---|------|----------|----------|
| 3.1 | Deposit 模型 | `src/funding/deposit.rs` | CRUD |
| 3.2 | 充值地址管理 | `src/funding/address.rs` | 地址分配 |

#### Phase 4: 提现功能 (P2)

| # | 任务 | 输出文件 | 验收标准 |
|---|------|----------|----------|
| 4.1 | Withdraw 模型 | `src/funding/withdraw.rs` | CRUD |
| 4.2 | 风控规则 | `src/funding/risk.rs` | 规则生效 |

---

## 8. 设计决策

| 决策 | 选择 | 理由 |
|------|------|------|
| 账户模型 | 子账户 | 隔离交易和充提资金 |
| 充提存储 | PostgreSQL | 需要事务 ACID |
| 流水存储 | PostgreSQL | 需要精确查询 |
| 划转 | 同步 | 低延迟，用户体验好 |
| 充提 | 异步 | 依赖链上确认 |

---

## 9. P2 未来工作

| 项目 | 优先级 | 说明 |
|------|--------|------|
| 链上 Indexer | P2 | 监控充值确认 |
| 冷热钱包 | P2 | 资金安全 |
| 多签提现 | P2 | 大额安全 |
| 提现白名单 | P2 | 地址管理 |

---

**状态**: 📝 草稿 - 先实现 Transfer
