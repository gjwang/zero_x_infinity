# 0x09-b Settlement Persistence: TDengine 集成

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

详细对比见: [数据库选型分析](./database_selection_tdengine.md)

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
-- ============================================================================
-- Database
-- ============================================================================
CREATE DATABASE IF NOT EXISTS trading 
    KEEP 365d              -- 数据保留 1 年
    DURATION 10d           -- 每 10 天一个分区
    BUFFER 256             -- 写缓冲 256MB
    WAL_LEVEL 2            -- WAL 持久化级别
    PRECISION 'us';        -- 微秒精度

USE trading;

-- ============================================================================
-- Orders Super Table
-- ============================================================================
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

-- ============================================================================
-- Trades Super Table
-- ============================================================================
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

-- ============================================================================
-- Balances Super Table
-- ============================================================================
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

-- ============================================================================
-- Order Events Super Table (审计日志)
-- ============================================================================
CREATE STABLE IF NOT EXISTS order_events (
    ts TIMESTAMP,
    order_id BIGINT UNSIGNED,
    event_type TINYINT UNSIGNED,-- 0=CREATED, 1=FILLED, 2=PARTIALLY_FILLED, 3=CANCELED
    prev_status TINYINT UNSIGNED,
    new_status TINYINT UNSIGNED,
    filled_qty BIGINT UNSIGNED,
    remaining_qty BIGINT UNSIGNED
) TAGS (
    symbol_id INT UNSIGNED
);
```

### 2.3 状态枚举

```rust
// src/models.rs (已有)
pub enum OrderStatus {
    NEW = 0,
    PARTIALLY_FILLED = 1,
    FILLED = 2,
    CANCELED = 3,
    REJECTED = 4,
}

pub enum Side {
    Buy = 0,
    Sell = 1,
}

pub enum OrderType {
    Limit = 0,
    Market = 1,
}

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

#### GET /api/v1/order/{order_id}

```json
// Response
{
    "code": 0,
    "msg": "ok",
    "data": {
        "order_id": 1001,
        "cid": "my-order-001",
        "symbol": "BTC_USDT",
        "side": "BUY",
        "order_type": "LIMIT",
        "price": "85000.00",
        "qty": "0.001",
        "filled_qty": "0.0005",
        "status": "PARTIALLY_FILLED",
        "created_at": 1734533784000,
        "updated_at": 1734533790000
    }
}
```

#### GET /api/v1/orders

```json
// Request Query Params
// ?symbol=BTC_USDT&status=NEW&limit=100&start_time=1734533784000

// Response
{
    "code": 0,
    "msg": "ok",
    "data": {
        "orders": [...],
        "total": 150,
        "has_more": true
    }
}
```

#### GET /api/v1/balances

```json
// Response
{
    "code": 0,
    "msg": "ok",
    "data": {
        "balances": [
            {
                "asset": "BTC",
                "avail": "1.50000000",
                "frozen": "0.10000000"
            },
            {
                "asset": "USDT",
                "avail": "50000.0000",
                "frozen": "8500.0000"
            }
        ]
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
├── gateway/
│   ├── handlers.rs         // 现有 + 查询端点
│   └── ...
└── ...
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
    
    for (i, trade) in trades.iter().enumerate() {
        sql.push_str(&format!(
            "trades_{} VALUES ({}, {}, {}, {}, {}, {}) ",
            trade.symbol_id,
            trade.ts,
            trade.trade_id,
            trade.order_id,
            trade.price,
            trade.qty,
            trade.role
        ));
    }
    
    client.exec(&sql).await;
}
```

---

## 5. 实现计划

### Phase 1: 基础持久化 (本次)

- [ ] TDengine 连接管理
- [ ] Schema 初始化
- [ ] 成交写入
- [ ] 订单状态更新写入
- [ ] 余额快照写入

### Phase 2: 查询接口

- [ ] GET /api/v1/order/{order_id}
- [ ] GET /api/v1/orders
- [ ] GET /api/v1/trades
- [ ] GET /api/v1/balances

### Phase 3: 优化

- [ ] 批量写入优化
- [ ] 连接池
- [ ] 缓存层 (Redis)

---

## 6. 验证计划

### 6.1 单元测试

```rust
#[tokio::test]
async fn test_insert_trade() {
    let client = TDengineClient::connect("localhost:6041").await;
    let trade = Trade { ... };
    assert!(client.insert_trade(trade).await.is_ok());
}
```

### 6.2 集成测试

```bash
# 1. 启动 TDengine
docker run -d -p 6030:6030 -p 6041:6041 tdengine/tdengine:latest

# 2. 运行 Gateway + Trading Core
cargo run --release -- --gateway --port 8080

# 3. 提交订单
curl -X POST http://localhost:8080/api/v1/create_order ...

# 4. 查询订单 (验证持久化)
curl http://localhost:8080/api/v1/order/1

# 5. 查询余额
curl http://localhost:8080/api/v1/balances
```

---

## Summary

本章实现 Settlement Persistence：

| 设计点 | 方案 |
|--------|------|
| 数据库 | TDengine (时序数据库) |
| Schema | Super Table (按 symbol 分表) |
| 写入 | 批量异步写入 |
| 查询 | REST API (GET endpoints) |

**核心理念**：

> 持久化是**旁路操作**，不阻塞主交易流程。Trading Core 保持高性能，Settlement 线程异步写入 TDengine。

下一章 (0x09-c) 将实现 WebSocket 实时推送。
