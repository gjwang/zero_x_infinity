# Database Selection: TDengine vs Others

<h3>
  <a href="#-english">🇺🇸 English</a>
  &nbsp;&nbsp;&nbsp;|&nbsp;&nbsp;&nbsp;
  <a href="#-chinese">🇨🇳 中文</a>
</h3>

<div id="-english"></div>

## 🇺🇸 English

> **Scenario**: Settlement Persistence - Storing orders, trades, and balances.

---

## 📊 Comparison

### Candidates

| Database | Type | Use Case |
|----------|------|----------|
| **TDengine** | Time-Series | IoT, Financial Data, High-Frequency Write |
| PostgreSQL | Relational | General OLTP |
| TimescaleDB | PG Extension | Time-Series (PG based) |
| ClickHouse | Columnar | OLAP, Analytics |

---

## 🎯 Why TDengine?

### 1. Performance (Based on TSBS)

| Metric | TDengine vs TimescaleDB | TDengine vs PostgreSQL |
|--------|-------------------------|------------------------|
| **Write Speed** | 1.5-6.7x Faster | 10x+ Faster |
| **Query Speed** | 1.2-24.6x Faster | 10x+ Faster |
| **Storage** | 1/12 - 1/27 Space | Huge Saving |

### 2. Matching Exchange Requirements

| Requirement | TDengine Solution |
|-------------|-------------------|
| **High Frequency Write** | Million/sec write capacity |
| **Timestamp Index** | Native time-series design |
| **High Cardinality** | High data points, Super Tables |
| **Real-time Stream** | Built-in Stream Computing |
| **Data Subscription** | Kafka-like real-time push |
| **Auto Partitioning** | Auto-sharding by time |

### 3. Simplified Architecture

```
TDengine Solution:
┌─────────────────────────────────────────────┐
│                  TDengine                    │
│      Persistence + Stream + Subscription     │
55 └─────────────────────────────────────────────┘
```

**Fewer Components = Lower Ops Complexity + Lower Latency**

### 4. Rust Ecosystem

*   ✅ Official Rust Client `taos`
*   ✅ Async (tokio)
*   ✅ Connection Pool (r2d2)
*   ✅ WebSocket (Cloud friendly)

---

## ❌ Why Not Others?

### PostgreSQL
*   ❌ Poor time-series performance.
*   ❌ High-frequency write bottleneck.
*   ❌ Large storage consumption.

### TimescaleDB
*   ⚠️ Slower than TDengine.
*   ⚠️ Much larger storage footprint.

### ClickHouse
*   ✅ Fast analytics.
*   ❌ Real-time row-by-row write is weak (prefers batch).
*   ❌ High Ops complexity.

---

## 📋 Data Model

### TDengine Super Table

```sql
-- Orders Super Table
CREATE STABLE orders (
    ts TIMESTAMP,           -- PK
    order_id BIGINT,
    user_id BIGINT,
    side TINYINT,
    order_type TINYINT,
    price BIGINT,
    qty BIGINT,
    filled_qty BIGINT,
    status TINYINT
) TAGS (
    symbol_id INT           -- Partition Tag
);

-- Trades
CREATE STABLE trades (...) TAGS (symbol_id INT);

-- Balances
CREATE STABLE balances (...) TAGS (user_id BIGINT, asset_id INT);
```

### Advantages

*   ✅ Auto-partition by TAG.
*   ✅ Auto-aggregation query.
*   ✅ Unified Schema.

---

## 🏗️ Architecture Integration

```
Gateway -> Order Queue -> Trading Core -> Events -> TDengine
```

---

## ✅ Final Recommendation

**Primary Storage**: TDengine
*   Orders, Trades, Balances History.
*   High performance write/read.

## 📊 Expected Performance

*   Write Latency: < 1ms
*   Query Latency: < 5ms
*   Storage Compression: 10:1
*   Supported TPS: 100,000+

<br>
<div align="right"><a href="#-english">↑ Back to Top</a></div>
<br>

---

<div id="-chinese"></div>

## 🇨🇳 中文

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
TDengine 方案:
┌─────────────────────────────────────────────┐
│                  TDengine                    │
│      持久化 + 流计算 + 数据订阅              │
└─────────────────────────────────────────────┘
```

**减少组件 = 减少运维复杂度 + 减少延迟**

### 4. Rust 生态支持

- ✅ 官方 Rust 客户端 `taos`
- ✅ 异步支持 (tokio 兼容)
- ✅ 连接池 (r2d2)
- ✅ WebSocket 连接 (适合云部署)

---

## ❌ 为什么不选其他方案

### PostgreSQL
- ❌ 通用数据库，时序性能差
- ❌ 高频写入会成为瓶颈
- ❌ 存储空间消耗大

### TimescaleDB
- ⚠️ 基于 PostgreSQL，继承其限制
- ⚠️ 比 TDengine 慢 1.5-6.7x
- ⚠️ 存储空间是 TDengine 的 12-27x

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
CREATE STABLE orders (...) TAGS (symbol_id INT);

-- 成交表 (Super Table)
CREATE STABLE trades (...) TAGS (symbol_id INT);

-- 余额快照表 (Super Table)  
CREATE STABLE balances (...) TAGS (user_id BIGINT, asset_id INT);
```

### Super Table 优势

- ✅ 自动按 TAG 分表
- ✅ 查询时自动聚合
- ✅ Schema 统一管理

---

## 🏗️ 架构集成方案

```
Gateway -> Order Queue -> Trading Core -> Events -> TDengine
```

---

## ✅ 最终推荐

**主存储**: TDengine
- 订单、成交、余额历史
- 高性能写入和查询
- 自动数据分区和压缩

## 📊 预期性能

*   写入延迟: < 1ms
*   查询延迟: < 5ms
*   存储压缩率: 10:1
*   支持 TPS: 100,000+
