# 数据库选型分析: TDengine vs 其他方案

> **场景**: 交易所 Settlement Persistence - 存储订单、成交、余额

---

## 📊 方案对比

### 候选数据库

| 数据库 | 类型 | 适用场景 |
|--------|------|----------|
| **TDengine** | 时序数据库 | IoT, 金融数据, 高频写入 |
| PostgreSQL | 关系型数据库 | 通用 OLTP |
| TimescaleDB | PostgreSQL扩展 | 时序数据 (基于PG) |
| ClickHouse | 列式分析数据库 | OLAP, 大规模聚合 |

---

## 🎯 为什么选择 TDengine

### 1. 性能优势 (基于 TSBS 基准测试)

| 指标 | TDengine vs TimescaleDB | TDengine vs PostgreSQL |
|------|-------------------------|------------------------|
| **写入速度** | 1.5-6.7x 更快 | 10x+ 更快 |
| **查询速度** | 1.2-24.6x 更快 | 10x+ 更快 |
| **存储空间** | 1/12 - 1/27 | 极大节省 |

### 2. 交易所场景完美匹配

| 需求 | TDengine 解决方案 |
|------|-------------------|
| **高频写入** | 百万/秒级写入能力 |
| **时间戳索引** | 原生时序设计，毫秒级查询 |
| **高基数支持** | 亿级数据点，Super Table |
| **实时分析** | 内置流计算引擎 |
| **数据订阅** | 类 Kafka 的实时推送 |
| **自动分区** | 按时间自动分片 |
| **高压缩率** | 1/10 存储空间 |

### 3. 简化架构

```
传统方案:
┌─────────────┬────────────────┐
│ PostgreSQL  │     Kafka      │
│  (持久化)   │   (消息队列)    │
└─────────────┴────────────────┘

TDengine 方案:
┌─────────────────────────────────────────────┐
│                  TDengine                    │
│      持久化 + 流计算 + 数据订阅              │
└─────────────────────────────────────────────┘
```

**减少组件 = 减少运维复杂度 + 减少延迟**

### 4. Rust 生态支持

```toml
# Cargo.toml
[dependencies]
taos = { version = "0.12", features = ["ws", "ws-rustls"] }
```

- ✅ 官方 Rust 客户端 `taos`
- ✅ 异步支持 (tokio 兼容)
- ✅ 连接池 (r2d2)
- ✅ WebSocket 连接 (适合云部署)

---

## ❌ 为什么不选其他方案

### PostgreSQL
- ❌ 通用数据库，时序性能差
- ❌ 高频写入会成为瓶颈
- ❌ 需要额外优化 (分区表、索引调优)
- ❌ 存储空间消耗大

### TimescaleDB
- ⚠️ 基于 PostgreSQL，继承其限制
- ⚠️ 比 TDengine 慢 1.5-6.7x
- ⚠️ 存储空间是 TDengine 的 12-27x
- ✅ 如果已有 PostgreSQL 生态可考虑

### ClickHouse
- ✅ 分析查询极快
- ❌ 实时写入不如 TDengine
- ❌ 更适合批量导入 + OLAP
- ❌ 运维复杂度高

---

## 📋 交易所数据模型设计

### TDengine Super Table 设计

```sql
-- 订单表 (Super Table)
CREATE STABLE orders (
    ts TIMESTAMP,           -- 订单时间戳 (主键)
    order_id BIGINT,
    user_id BIGINT,
    side TINYINT,           -- 0=BUY, 1=SELL
    order_type TINYINT,     -- 0=LIMIT, 1=MARKET
    price BIGINT,           -- 价格 (整数表示)
    qty BIGINT,             -- 数量 (整数表示)
    filled_qty BIGINT,
    status TINYINT          -- 0=NEW, 1=FILLED, 2=CANCELED
) TAGS (
    symbol_id INT           -- 交易对 ID
);

-- 成交表 (Super Table)
CREATE STABLE trades (
    ts TIMESTAMP,           -- 成交时间戳
    trade_id BIGINT,
    order_id BIGINT,
    user_id BIGINT,
    side TINYINT,
    price BIGINT,
    qty BIGINT,
    fee BIGINT
) TAGS (
    symbol_id INT
);

-- 余额快照表 (Super Table)  
CREATE STABLE balances (
    ts TIMESTAMP,           -- 快照时间
    avail BIGINT,           -- 可用余额
    frozen BIGINT,          -- 冻结余额
    version BIGINT          -- 版本号
) TAGS (
    user_id BIGINT,
    asset_id INT
);

-- 查询示例
-- 查询用户最新余额
SELECT LAST_ROW(avail, frozen, version) FROM balances 
WHERE user_id = 1001 AND asset_id = 1;

-- 查询用户订单历史
SELECT * FROM orders WHERE user_id = 1001 
ORDER BY ts DESC LIMIT 100;

-- 查询成交历史
SELECT * FROM trades WHERE user_id = 1001
AND ts >= NOW() - INTERVAL 1 DAY;
```

### Super Table 优势

```
┌─────────────────────────────────────────────────────────┐
│                Super Table: orders                       │
│  (统一 schema，自动按 symbol_id 分表)                    │
├─────────────────┬─────────────────┬────────────────────┤
│ orders_BTC_USDT │ orders_ETH_USDT │ orders_ETH_BTC ... │
│   (子表 1)      │    (子表 2)     │     (子表 N)       │
└─────────────────┴─────────────────┴────────────────────┘
```

- ✅ 自动按 TAG 分表
- ✅ 查询时自动聚合
- ✅ Schema 统一管理

---

## 🏗️ 架构集成方案

```
┌──────────────────────────────────────────────────────────────────┐
│                         Gateway (HTTP)                            │
└────────────────────────────────┬─────────────────────────────────┘
                                 │
                    ┌────────────▼────────────┐
                    │      Order Queue        │
                    │   (Ring Buffer)         │
                    └────────────┬────────────┘
                                 │
┌────────────────────────────────▼────────────────────────────────┐
│                      Trading Core                                │
│   Ingestion → UBSCore → ME → Settlement                          │
└────────────────────────────────┬────────────────────────────────┘
                                 │
              ┌──────────────────┼──────────────────┐
              │                  │                  │
    ┌─────────▼──────┐  ┌───────▼───────┐  ┌──────▼──────┐
    │  Order Events  │  │ Trade Events  │  │Balance Events│
    └─────────┬──────┘  └───────┬───────┘  └──────┬──────┘
              │                 │                  │
              └─────────────────┼──────────────────┘
                                │
                    ┌───────────▼───────────┐
                    │      TDengine         │
                    │  orders | trades | bal │
                    └───────────────────────┘
```

---

## ✅ 最终推荐

**主存储**: TDengine
- 订单、成交、余额历史
- 高性能写入和查询
- 自动数据分区和压缩

## 📊 预期性能

| 指标 | 预期值 |
|------|--------|
| 写入延迟 | < 1ms |
| 查询延迟 (最新余额) | < 5ms |
| 历史查询 (100条) | < 10ms |
| 存储压缩率 | 10:1 |
| 支持 TPS | 100,000+ |

---

## 🔗 参考资料

- [TDengine 官网](https://tdengine.com/)
- [TDengine Rust Client (taos)](https://github.com/taosdata/taos-connector-rust)
- [TDengine vs TimescaleDB Benchmark](https://tdengine.com/tdengine-vs-timescaledb/)
