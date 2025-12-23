# 0x09-b Settlement Persistence: TDengine Integration

<h3>
  <a href="#-english">🇺🇸 English</a>
  &nbsp;&nbsp;&nbsp;|&nbsp;&nbsp;&nbsp;
  <a href="#-chinese">🇨🇳 中文</a>
</h3>

<div id="-english"></div>

## 🇺🇸 English

> **📦 Code Changes**: [View Diff](https://github.com/gjwang/zero_x_infinity/compare/v0.9-a-gateway...v0.9-b-settlement-persistence)

> **Core Objective**: Persist trade data to TDengine and implement Order Query & History APIs.

---

## Background: From Memory to Persistence

In Gateway Phase 1 (0x09-a), we completed:
*   ✅ HTTP API (create_order, cancel_order)
*   ✅ Order Validation
*   ✅ Ring Buffer Integration
*   ⏳ **Data Persistence** ← This Chapter

Current System Issue:

```
┌─────────────────────────────────────────────────────────────────┐
│                    Trading Core (In-Memory)                      │
│                                                                  │
│    Orders → Match → Trades → Settle → Balance Update             │
│       ↓         ↓           ↓                                   │
│      ❌         ❌           ❌    ← Data LOST on restart!       │
└─────────────────────────────────────────────────────────────────┘
```

This Chapter's Solution:

```
┌─────────────────────────────────────────────────────────────────┐
│                    Trading Core                                  │
│                                                                  │
│    Orders → Match → Trades → Settle → Balance Update             │
│       ↓         ↓           ↓                                   │
│    ┌─────────────────────────────────────────────────┐          │
│    │              TDengine (Persistence)              │          │
│    │    orders | trades | balances                   │          │
│    └─────────────────────────────────────────────────┘          │
└─────────────────────────────────────────────────────────────────┘
```

---

## 1. Why TDengine?

Detailed comparison: [Database Selection Analysis](./database-selection-tdengine.md)

### Core Advantages

| Feature | TDengine | PostgreSQL |
|---------|----------|------------|
| Write Speed | 1M/sec | 10k/sec |
| Time-Series | Native Support | Index Optimization Needed |
| Storage | 1/10 | 1x |
| Real-time Analytics | Built-in Stream | External Tools Needed |
| Rust Client | ✅ Official `taos` | ✅ `tokio-postgres` |

---

## 2. Schema Design

### 2.1 Super Table Architecture

TDengine uses the **Super Table** concept:

```
┌─────────────────────────────────────────────────────────┐
│              Super Table: orders                         │
│    (Unified schema, auto-create sub-table per symbol)    │
├─────────────────┬─────────────────┬────────────────────┤
│ orders_1        │ orders_2        │ orders_N           │
│ (BTC_USDT)      │ (ETH_USDT)      │ (...)              │
└─────────────────┴─────────────────┴────────────────────┘
```

### 2.2 DDL Definitions

```sql
-- Database Setup
CREATE DATABASE IF NOT EXISTS trading 
    KEEP 365d              -- Retain data for 1 year
    DURATION 10d           -- Partition every 10 days
    BUFFER 256             -- 256MB Write Buffer
    WAL_LEVEL 2            -- WAL Persistence Level
    PRECISION 'us';        -- Microsecond Precision

USE trading;

-- Orders Super Table
CREATE STABLE IF NOT EXISTS orders (
    ts TIMESTAMP,               -- Timestamp (PK)
    order_id BIGINT UNSIGNED,
    user_id BIGINT UNSIGNED,
    side TINYINT UNSIGNED,      -- 0=BUY, 1=SELL
    order_type TINYINT UNSIGNED,-- 0=LIMIT, 1=MARKET
    price BIGINT UNSIGNED,      -- Integer representation
    qty BIGINT UNSIGNED,
    filled_qty BIGINT UNSIGNED,
    status TINYINT UNSIGNED,
    cid NCHAR(64)               -- Client Order ID
) TAGS (
    symbol_id INT UNSIGNED      -- Partition Key
);

-- Trades Super Table
CREATE STABLE IF NOT EXISTS trades (
    ts TIMESTAMP,
    trade_id BIGINT UNSIGNED,
    order_id BIGINT UNSIGNED,
    user_id BIGINT UNSIGNED,
    side TINYINT UNSIGNED,
    price BIGINT UNSIGNED,
    qty BIGINT UNSIGNED,
    fee BIGINT UNSIGNED,
    role TINYINT UNSIGNED       -- 0=MAKER, 1=TAKER
) TAGS (
    symbol_id INT UNSIGNED
);

-- Balances Super Table
CREATE STABLE IF NOT EXISTS balances (
    ts TIMESTAMP,
    avail BIGINT UNSIGNED,
    frozen BIGINT UNSIGNED,
    lock_version BIGINT UNSIGNED,
    settle_version BIGINT UNSIGNED
) TAGS (
    user_id BIGINT UNSIGNED,
    asset_id INT UNSIGNED
);
```

### 2.3 Status Enums

```rust
// New Enum
pub enum TradeRole {
    Maker = 0,
    Taker = 1,
}
```

---

## 3. API Design

### 3.1 Query Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/v1/order/{order_id}` | GET | Query single order |
| `/api/v1/orders` | GET | Query order list |
| `/api/v1/trades` | GET | Query trade history |
| `/api/v1/balances` | GET | Query user balances |

### 3.2 Request/Response Format

**GET /api/v1/order/{order_id}**:

```json
{
    "code": 0,
    "msg": "ok",
    "data": {
        "order_id": 1001,
        "symbol": "BTC_USDT",
        "status": "PARTIALLY_FILLED",
        "filled_qty": "0.0005",
        "created_at": 1734533784000
    }
}
```

**GET /api/v1/balances**:

```json
{
    "code": 0,
    "msg": "ok",
    "data": {
        "balances": [
             { "asset": "BTC", "avail": "1.50000000", "frozen": "0.10000000" }
        ]
    }
}
```

---

## 4. Implementation Architecture

### 4.1 Module Structure

```
src/
├── persistence/
│   ├── mod.rs              // Entry
│   ├── tdengine.rs         // Connection Manager
│   ├── orders.rs           // Order Persistence
│   ├── trades.rs           // Trade Persistence
│   └── balances.rs         // Balance Persistence
```

### 4.2 Data Flow

```
┌─────────────────────────────────────────────────────────────────┐
│                      Settlement Thread                           │
│                                                                  │
│    trade_queue.pop() ──┬── Update In-Memory Balance              │
│                        │                                         │
│                        └── Write to TDengine                     │
│                             ├── INSERT trades                    │
│                             ├── INSERT order_events              │
│                             └── INSERT balances (Snapshot)       │
└─────────────────────────────────────────────────────────────────┘
```

### 4.3 Batch Write Optimization

```rust
// Batch write to reduce I/O overhead
const BATCH_SIZE: usize = 1000;

async fn flush_trades(trades: Vec<Trade>) {
    let mut sql = String::from("INSERT INTO ");
    // Construct bulk insert SQL...
    client.exec(&sql).await;
}
```

---

## 5. Implementation Plan

### Phase 1: Basic Persistence (This Chapter)
*   [ ] TDengine Connection
*   [ ] Schema Initialization
*   [ ] Trade/Order/Balance Writes

### Phase 2: Query APIs
*   [ ] Implement GET Endpoints

### Phase 3: Optimization
*   [ ] Batch Writes
*   [ ] Connection Pool
*   [ ] Redis Cache

---

## 6. Verification Plan

### 6.1 Integration Test

```bash
# 1. Start TDengine
docker run -d -p 6030:6030 -p 6041:6041 tdengine/tdengine:latest

# 2. Run Gateway
cargo run --release -- --gateway --port 8080

# 3. Submit Order
curl -X POST http://localhost:8080/api/v1/create_order ...

# 4. Query Order (Verify Persistence)
curl http://localhost:8080/api/v1/order/1
```

---

## Summary

This chapter implements Settlement Persistence.

**Core Philosophy**:
> Persistence is a **side-channel operation**, not blocking the main trading flow. The Settlement thread writes to TDengine asynchronously.

<br>
<div align="right"><a href="#-english">↑ Back to Top</a></div>
<br>

---

<div id="-chinese"></div>

## 🇨🇳 中文

> **📦 代码变更**: [查看 Diff](https://github.com/gjwang/zero_x_infinity/compare/v0.9-a-gateway...v0.9-b-settlement-persistence)

> **本节核心目标**：将成交数据持久化到 TDengine，实现订单查询和历史记录 API。

---

## 背景：从内存到持久化

在 Gateway Phase 1 (0x09-a) 中，我们完成了：
- ✅ HTTP API (create_order, cancel_order)
- ✅ 订单验证和转换
- ✅ Ring Buffer 队列集成
- ⏳ **数据持久化** ← 本章

当前系统的问题：

```
┌─────────────────────────────────────────────────────────────────┐
│                    Trading Core (内存中)                         │
│                                                                  │
│    Orders → 匹配 → Trades → 结算 → 余额更新                      │
│       ↓         ↓           ↓                                   │
│      ❌         ❌           ❌    ← 重启后数据丢失！              │
└─────────────────────────────────────────────────────────────────┘
```

本章解决方案：

```
┌─────────────────────────────────────────────────────────────────┐
│                    Trading Core                                  │
│                                                                  │
│    Orders → 匹配 → Trades → 结算 → 余额更新                      │
│       ↓         ↓           ↓                                   │
│    ┌─────────────────────────────────────────────────┐          │
│    │              TDengine (持久化)                   │          │
│    │    orders | trades | balances                   │          │
│    └─────────────────────────────────────────────────┘          │
└─────────────────────────────────────────────────────────────────┘
```

---

## 1. 为什么选择 TDengine

详细对比见: [数据库选型分析](./database-selection-tdengine.md)

### 核心优势

| 特性 | TDengine | PostgreSQL |
|------|----------|------------|
| 写入速度 | 100万/秒 | 1万/秒 |
| 时序查询 | 原生支持 | 需要索引优化 |
| 存储空间 | 1/10 | 1x |
| 实时分析 | 内置流计算 | 需要额外工具 |
| Rust 客户端 | ✅ 官方 `taos` | ✅ `tokio-postgres` |

---

## 2. Schema 设计

### 2.1 Super Table 架构

TDengine 使用 **Super Table** 概念：

```
┌─────────────────────────────────────────────────────────┐
│              Super Table: orders                         │
│    (统一 schema，自动按 symbol_id 创建子表)               │
├─────────────────┬─────────────────┬────────────────────┤
│ orders_1        │ orders_2        │ orders_N           │
│ (BTC_USDT)      │ (ETH_USDT)      │ (...)              │
└─────────────────┴─────────────────┴────────────────────┘
```

### 2.2 DDL 定义

```sql
-- Database Setup
CREATE DATABASE IF NOT EXISTS trading 
    KEEP 365d              -- 数据保留 1 年
    DURATION 10d           -- 每 10 天一个分区
    BUFFER 256             -- 写缓冲 256MB
    WAL_LEVEL 2            -- WAL 持久化级别
    PRECISION 'us';        -- 微秒精度

USE trading;

-- Orders Super Table
CREATE STABLE IF NOT EXISTS orders (
    ts TIMESTAMP,               -- 订单时间戳 (主键)
    order_id BIGINT UNSIGNED,   -- 订单 ID
    user_id BIGINT UNSIGNED,    -- 用户 ID
    side TINYINT UNSIGNED,      -- 0=BUY, 1=SELL
    order_type TINYINT UNSIGNED,-- 0=LIMIT, 1=MARKET
    price BIGINT UNSIGNED,      -- 价格 (整数)
    qty BIGINT UNSIGNED,        -- 原始数量
    filled_qty BIGINT UNSIGNED, -- 已成交数量
    status TINYINT UNSIGNED,    -- 订单状态
    cid NCHAR(64)               -- 客户端订单 ID
) TAGS (
    symbol_id INT UNSIGNED      -- 交易对 ID (分区键)
);

-- Trades Super Table
CREATE STABLE IF NOT EXISTS trades (
    ts TIMESTAMP,               -- 成交时间戳
    trade_id BIGINT UNSIGNED,   -- 成交 ID
    order_id BIGINT UNSIGNED,   -- 订单 ID
    user_id BIGINT UNSIGNED,    -- 用户 ID
    side TINYINT UNSIGNED,      -- 0=BUY, 1=SELL
    price BIGINT UNSIGNED,      -- 成交价格
    qty BIGINT UNSIGNED,        -- 成交数量
    fee BIGINT UNSIGNED,        -- 手续费
    role TINYINT UNSIGNED       -- 0=MAKER, 1=TAKER
) TAGS (
    symbol_id INT UNSIGNED
);

-- Balances Super Table
CREATE STABLE IF NOT EXISTS balances (
    ts TIMESTAMP,               -- 快照时间
    avail BIGINT UNSIGNED,      -- 可用余额
    frozen BIGINT UNSIGNED,     -- 冻结余额
    lock_version BIGINT UNSIGNED,   -- 锁定版本
    settle_version BIGINT UNSIGNED  -- 结算版本
) TAGS (
    user_id BIGINT UNSIGNED,    -- 用户 ID
    asset_id INT UNSIGNED       -- 资产 ID
);
```

### 2.3 状态枚举

```rust
// 新增
pub enum TradeRole {
    Maker = 0,
    Taker = 1,
}
```

---

## 3. API 设计

### 3.1 查询端点

| 端点 | 方法 | 描述 |
|------|------|------|
| `/api/v1/order/{order_id}` | GET | 查询单个订单 |
| `/api/v1/orders` | GET | 查询订单列表 |
| `/api/v1/trades` | GET | 查询成交历史 |
| `/api/v1/balances` | GET | 查询用户余额 |

### 3.2 请求/响应格式

**GET /api/v1/order/{order_id}**:

```json
{
    "code": 0,
    "msg": "ok",
    "data": {
        "order_id": 1001,
        "symbol": "BTC_USDT",
        "status": "PARTIALLY_FILLED",
        "filled_qty": "0.0005",
        "created_at": 1734533784000
    }
}
```

---

## 4. 实现架构

### 4.1 模块结构

```
src/
├── persistence/
│   ├── mod.rs              // 模块入口
│   ├── tdengine.rs         // TDengine 连接管理
│   ├── orders.rs           // 订单持久化
│   ├── trades.rs           // 成交持久化
│   └── balances.rs         // 余额持久化
```

### 4.2 数据流

```
┌─────────────────────────────────────────────────────────────────┐
│                      Settlement 线程                             │
│                                                                  │
│    trade_queue.pop() ──┬── 更新内存余额                          │
│                        │                                         │
│                        └── 写入 TDengine                         │
│                             ├── INSERT trades                    │
│                             ├── INSERT order_events              │
│                             └── INSERT balances (快照)           │
└─────────────────────────────────────────────────────────────────┘
```

### 4.3 批量写入优化

```rust
// 批量写入，减少 I/O 开销
const BATCH_SIZE: usize = 1000;

async fn flush_trades(trades: Vec<Trade>) {
    let mut sql = String::from("INSERT INTO ");
    // ... 构建批量插入 SQL
    client.exec(&sql).await;
}
```

---

## 5. 实现计划

### Phase 1: 基础持久化 (本次)
- [ ] TDengine 连接管理
- [ ] Schema 初始化
- [ ] 成交/订单/余额写入

### Phase 2: 查询接口
- [ ] 实现 GET 端点

### Phase 3: 优化
- [ ] 批量写入
- [ ] 连接池
- [ ] Redis 缓存

---

## 6. 验证计划

### 6.1 集成测试

```bash
# 1. 启动 TDengine
docker run -d -p 6030:6030 -p 6041:6041 tdengine/tdengine:latest

# 2. 运行 Gateway
cargo run --release -- --gateway --port 8080

# 3. 提交订单
curl -X POST http://localhost:8080/api/v1/create_order ...

# 4. 查询订单 (验证持久化)
curl http://localhost:8080/api/v1/order/1
```

---

## Summary

本章实现 Settlement Persistence：

**核心理念**：
> 持久化是**旁路操作**，不阻塞主交易流程。Trading Core 保持高性能，Settlement 线程异步写入 TDengine。

下一章 (0x09-c) 将实现 WebSocket 实时推送。
