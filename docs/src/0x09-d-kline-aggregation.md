# 0x09-d K-Line Aggregation Service

<h3>
  <a href="#-english">🇺🇸 English</a>
  &nbsp;&nbsp;&nbsp;|&nbsp;&nbsp;&nbsp;
  <a href="#-chinese">🇨🇳 中文</a>
</h3>

<div id="-english"></div>

## 🇺🇸 English

> **📦 Code Changes**: [View Diff](https://github.com/gjwang/zero_x_infinity/compare/v0.9-c-websocket-push...v0.9-d-kline-aggregation)

> **Core Objective**: Implement real-time K-Line (Candlestick) aggregation service, supporting multiple intervals (1m, 5m, 15m, 30m, 1h, 1d).

---

## Background: Market Data Aggregation

The exchange needs to provide standardized market data:

```
Trades                            K-Line (OHLCV)
  │                                    │
  ├── Trade 1: price=30000, qty=0.1    │
  ├── Trade 2: price=30100, qty=0.2  ──▶ 1-Min K-Line:
  ├── Trade 3: price=29900, qty=0.1    │   Open:  30000
  └── Trade 4: price=30050, qty=0.3    │   High:  30100
                                       │   Low:   29900
                                       │   Close: 30050
                                       │   Volume: 0.7
```

---

## 1. K-Line Data Structure

### 1.1 OHLCV

```rust
pub struct KLine {
    pub symbol_id: u32,
    pub interval: KLineInterval,
    pub open_time: u64,      // Unix timestamp (ms)
    pub close_time: u64,
    pub open: u64,
    pub high: u64,
    pub low: u64,
    pub close: u64,
    pub volume: u64,         // Base asset volume
    pub quote_volume: u64,   // Quote asset volume (price * qty)
    pub trade_count: u32,
}
```

> [!WARNING]
> **quote_volume Overflow**: `price * qty` might overflow `u64`.
>
> Correct SQL: `SUM(CAST(price AS DOUBLE) * CAST(qty AS DOUBLE)) AS quote_volume`

### 1.2 API Response Format

```json
{
    "symbol": "BTC_USDT",
    "interval": "1m",
    "open_time": 1734533760000,
    "close_time": 1734533819999,
    "open": "30000.00",
    "high": "30100.00",
    "low": "29900.00",
    "close": "30050.00",
    "volume": "0.700000",
    "quote_volume": "21035.00",
    "trade_count": 4
}
```

---

## 2. Architecture: TDengine Stream Computing

### 2.1 Core Concept

**Leverage TDengine built-in Stream Computing** for auto-aggregation. No manual aggregator implementation needed:

1.  Settlement writes to `trades` table.
2.  TDengine automatically triggers stream computing.
3.  Results are written to `klines` tables.
4.  HTTP API queries `klines` tables directly.

### 2.2 Data Flow

```
   Settlement ──▶ trades table (TDengine)
                      │
                      │ TDengine Stream Computing (Auto)
                      │
                      ├─── kline_1m_stream  ──► klines_1m table
                      ├─── kline_5m_stream  ──► klines_5m table
                      └─── ...
                                                    │
                           ┌────────────────────────┴───────────────────────┐
                           ▼                                                ▼
                    HTTP API                                        WebSocket Push
               GET /api/v1/klines                                kline.update (Optional)
```

### 2.3 TDengine Stream Example

```sql
CREATE STREAM IF NOT EXISTS kline_1m_stream
INTO klines_1m SUBTABLE(CONCAT('kl_1m_', CAST(symbol_id AS NCHAR(10))))
AS SELECT
    _wstart AS ts,
    FIRST(price) AS open,
    MAX(price) AS high,
    MIN(price) AS low,
    LAST(price) AS close,
    SUM(qty) AS volume,
    SUM(CAST(price AS DOUBLE) * CAST(qty AS DOUBLE)) AS quote_volume,
    COUNT(*) AS trade_count
FROM trades
PARTITION BY symbol_id
INTERVAL(1m);
```

---

## 3. API Design

### 3.1 HTTP Endpoint

`GET /api/v1/klines?symbol=BTC_USDT&interval=1m&limit=100`

### 3.2 WebSocket Push

```json
{
    "type": "kline.update",
    "data": {
        "symbol": "BTC_USDT",
        "interval": "1m",
        "open": "30000.00",
        "close": "30050.00",
        "is_final": false
    }
}
```

---

## 4. Module Structure

```
src/
├── persistence/
│   ├── klines.rs           # Create Streams, Query K-Lines
│   ├── schema.rs           # Add klines Super Table
│   └── queries.rs          # Add query_klines()
├── gateway/
│   ├── handlers.rs         # Add get_klines
│   └── ...
```

> [!TIP]
> No need for `src/kline/` logic directory, TDengine handles it.

---

## 5. Implementation Plan

*   [x] **Phase 1: Schema**: Add `klines` super table.
*   [x] **Phase 2: Stream Computing**: Implement `create_kline_streams()`.
*   [x] **Phase 3: HTTP API**: Implement `query_klines()` and API endpoint.
*   [x] **Phase 4: Verification**: E2E test.

---

## 6. Verification

### 6.1 E2E Test Scenarios

Script: `./scripts/test_kline_e2e.sh`

1.  Check API connectivity.
2.  Record initial K-Line count.
3.  Create matched orders.
4.  Wait for Stream processing (5s).
5.  Query K-Line API and verify data structure.

### 6.2 Binance Standard Alignment

> [!WARNING]
> **P0 Fix**: Ensure time fields align with Binance standard (Unix Milliseconds Number).
>
> *   `open_time`: 1734611580000 (was ISO 8601 string)
> *   `close_time`: 1734611639999 (was missing)

---

## Summary

This chapter implements K-Line aggregation service leveraging TDengine's Stream Computing.

**Key Concept**:
> K-Line is **derived data**. We calculate it from trades in real-time, rather than storing original raw data.

Next Chapter: **0x09-e OrderBook Depth**.

<br>
<div align="right"><a href="#-english">↑ Back to Top</a></div>
<br>

---

<div id="-chinese"></div>

## 🇨🇳 中文

> **📦 代码变更**: [查看 Diff](https://github.com/gjwang/zero_x_infinity/compare/v0.9-c-websocket-push...v0.9-d-kline-aggregation)

> **本节核心目标**：实现 K-Line (蜡烛图) 实时聚合服务，支持多时间周期 (1m, 5m, 15m, 30m, 1h, 1d)。

---

## 背景：行情数据聚合

交易所需要提供标准化的行情数据：

```
每笔成交                          K-Line (OHLCV)
  │                                    │
  ├── Trade 1: price=30000, qty=0.1    │
  ├── Trade 2: price=30100, qty=0.2  ──▶ 1分钟 K-Line:
  ├── Trade 3: price=29900, qty=0.1    │   Open:  30000
  └── Trade 4: price=30050, qty=0.3    │   High:  30100
                                       │   Low:   29900
                                       │   Close: 30050
                                       │   Volume: 0.7
```

---

## 1. K-Line 数据结构

### 1.1 OHLCV

```rust
pub struct KLine {
    pub symbol_id: u32,
    pub interval: KLineInterval,
    pub open_time: u64,      // 时间戳 (毫秒)
    pub close_time: u64,
    pub open: u64,           // 开盘价
    pub high: u64,           // 最高价
    pub low: u64,            // 最低价
    pub close: u64,          // 收盘价
    pub volume: u64,         // 成交量 (base asset)
    pub quote_volume: u64,   // 成交额 (quote asset)
    pub trade_count: u32,    // 成交笔数
}
```

> [!WARNING]
> **quote_volume 精度问题**: `price * qty` 可能导致 u64 溢出，需使用 DOUBLE 计算。

### 1.2 API 响应格式

```json
{
    "symbol": "BTC_USDT",
    "interval": "1m",
    "open_time": 1734533760000,
    "close_time": 1734533819999,
    "open": "30000.00",
    "high": "30100.00",
    "low": "29900.00",
    "close": "30050.00",
    "volume": "0.700000",
    "quote_volume": "21035.00",
    "trade_count": 4
}
```

---

## 2. 架构设计：TDengine Stream Computing

### 2.1 核心思路

**利用 TDengine 内置流计算自动聚合 K-Line**，无需手动实现聚合器：

- Settlement 写入 `trades` 表后，TDengine 自动触发流计算
- 流计算结果自动写入 `klines` 表
- HTTP API 直接查询 `klines` 表返回结果

### 2.2 数据流

```
   Settlement ──▶ trades 表 (TDengine)
                      │
                      │ TDengine Stream Computing (自动)
                      │
                      ├─── kline_1m_stream  ──► klines_1m 表
                      ├─── kline_5m_stream  ──► klines_5m 表
                      └─── ...
```

### 2.3 TDengine Stream 示例

```sql
CREATE STREAM IF NOT EXISTS kline_1m_stream
INTO klines_1m SUBTABLE(...)
AS SELECT
    _wstart AS ts,
    FIRST(price) AS open,
    MAX(price) AS high,
    MIN(price) AS low,
    LAST(price) AS close,
    SUM(qty) AS volume,
    SUM(CAST(price AS DOUBLE) * CAST(qty AS DOUBLE)) AS quote_volume,
    COUNT(*) AS trade_count
FROM trades
PARTITION BY symbol_id
INTERVAL(1m);
```

---

## 3. API 设计

HTTP 端点: `GET /api/v1/klines?symbol=BTC_USDT&interval=1m&limit=100`

---

## 4. 模块结构

```
src/
├── persistence/
│   ├── klines.rs           # Create Stream, Query K-Line
│   ├── schema.rs           # Add klines table
│   └── queries.rs          # Add query_klines()
├── gateway/
│   ├── handlers.rs         # Add get_klines
```

> [!TIP]
> 无需 `src/kline/` 目录，TDengine 流计算替代了手动聚合逻辑

---

## 5. 实现计划

- [x] **Phase 1: Schema**: 添加 `klines` 超级表。
- [x] **Phase 2: Stream Computing**: 实现 `create_kline_streams()`。
- [x] **Phase 3: HTTP API**: 实现查询函数和 API 端点。
- [x] **Phase 4: 验证**: E2E 测试。

---

## 6. 验证计划

运行脚本 `./scripts/test_kline_e2e.sh` 验证：
1.  API 连通性
2.  K-Line 数据生成 (Stream 处理)
3.  响应结构正确性 (对齐 Binance 标准)

---

## Summary

本章实现 K-Line 聚合服务。

**核心理念**：
> K-Line 是**衍生数据**：从成交事件实时计算，而非存储原始数据。

下一章 (0x09-e) 将实现 OrderBook Depth 聚合。
