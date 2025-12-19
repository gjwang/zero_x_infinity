# 0x09-d K-Line Aggregation: K线聚合服务

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

pub enum KLineInterval {
    M1,   // 1 minute
    M5,   // 5 minutes
    M15,  // 15 minutes
    M30,  // 30 minutes
    H1,   // 1 hour
    D1,   // 1 day
}
```

> [!WARNING]
> **quote_volume 精度问题**: `price * qty` 可能导致 u64 溢出
>
> ```sql
> -- ❌ 错误方案 (可能溢出)
> SUM(price * qty) AS quote_volume
>
> -- ✅ 正确方案 (使用 DOUBLE)
> SUM(CAST(price AS DOUBLE) * CAST(qty AS DOUBLE)) AS quote_volume
> ```


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
                     ├─── kline_15m_stream ──► klines_15m 表
                     ├─── kline_30m_stream ──► klines_30m 表
                     ├─── kline_1h_stream  ──► klines_1h 表
                     └─── kline_1d_stream  ──► klines_1d 表
                                                   │
                           ┌───────────────────────┴───────────────────────┐
                           ▼                                               ▼
                    HTTP API                                       WebSocket Push
               GET /api/v1/klines                               kline.update (可选)
```

### 2.3 TDengine Stream 示例

```sql
-- 创建 1 分钟 K-Line 流计算
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
-- 不使用 FILL: 空窗口不产生 K-Line
```

### 2.4 时间窗口 & Stream

| Interval | TDengine INTERVAL | Stream 名称 |
|----------|-------------------|-------------|
| 1m | INTERVAL(1m) | kline_1m_stream |
| 5m | INTERVAL(5m) | kline_5m_stream |
| 15m | INTERVAL(15m) | kline_15m_stream |
| 30m | INTERVAL(30m) | kline_30m_stream |
| 1h | INTERVAL(1h) | kline_1h_stream |
| 1d | INTERVAL(1d) | kline_1d_stream |

---

## 3. API 设计

### 3.1 HTTP 端点

| 端点 | 描述 |
|------|------|
| `GET /api/v1/klines?symbol=BTC_USDT&interval=1m&limit=100` | 获取历史 K 线 |

### 3.2 WebSocket 推送

```json
// K 线更新推送
{
    "type": "kline.update",
    "data": {
        "symbol": "BTC_USDT",
        "interval": "1m",
        "open_time": 1734533760000,
        "open": "30000.00",
        "high": "30100.00",
        "low": "29900.00",
        "close": "30050.00",
        "volume": "0.700000",
        "is_final": false
    }
}

// is_final = true 表示该 K 线已完结，不会再更新
```

---

## 4. 模块结构

```
src/
├── persistence/
│   ├── klines.rs           # 创建 Stream, 查询 K-Line (新增)
│   ├── schema.rs           # 添加 klines 超级表
│   └── queries.rs          # 添加 query_klines()
├── gateway/
│   ├── handlers.rs         # 添加 get_klines
│   └── mod.rs              # 添加路由
└── websocket/
    └── messages.rs         # 添加 KLineUpdate (可选)
```

> [!TIP]
> 无需 `src/kline/` 目录，TDengine 流计算替代了手动聚合逻辑

---

## 5. 实现计划

### Phase 1: Schema
- [ ] 添加 `klines` 超级表到 `schema.rs`
- [ ] 在 `init_schema()` 中创建表

### Phase 2: Stream Computing
- [ ] 创建 `persistence/klines.rs` 模块
- [ ] 实现 `create_kline_streams()` (6 个周期)
- [ ] Gateway 初始化时调用

### Phase 3: HTTP API
- [ ] 实现 `query_klines()` 查询函数
- [ ] 添加 `GET /api/v1/klines` 端点
- [ ] 格式化响应 (display_decimals)

### Phase 4: 验证
- [ ] 验证 Schema 创建
- [ ] 验证 Stream 自动聚合
- [ ] E2E 测试 API

### (可选) Phase 5: WebSocket Push
- [ ] 研究 TDengine TMQ 订阅
- [ ] 实现 kline.update 推送

---

## 6. 验证计划

### 6.1 单元测试

```rust
#[test]
fn test_kline_aggregation() {
    let mut agg = Aggregator::new(KLineInterval::M1);
    
    agg.add_trade(30000, 100000);  // price, qty
    agg.add_trade(30100, 200000);
    agg.add_trade(29900, 100000);
    agg.add_trade(30050, 300000);
    
    let kline = agg.current();
    assert_eq!(kline.open, 30000);
    assert_eq!(kline.high, 30100);
    assert_eq!(kline.low, 29900);
    assert_eq!(kline.close, 30050);
    assert_eq!(kline.volume, 700000);
}
```

### 6.2 E2E 测试方案

#### 前置条件

1. TDengine 运行中：`docker ps | grep tdengine`
2. Gateway 运行中：`cargo run --release -- --gateway --port 8080`

#### 测试脚本

```bash
./scripts/test_kline_e2e.sh
```

该脚本执行以下步骤：

| 步骤 | 操作 | 验证点 |
|------|------|--------|
| 1 | 检查 API 连通性 | `/api/v1/klines` 可访问 |
| 2 | 记录初始 K-Line 数量 | 基准值 |
| 3 | 创建匹配订单 (Buy + Sell) | 订单成功创建 |
| 4 | 等待 Stream 处理 (5s) | TDengine Stream 聚合 |
| 5 | 查询 K-Line API | 返回 OHLCV 数据 |
| 6 | 验证响应结构 | code=0, symbol 正确 |

#### 手动验证

```bash
# 1. 查看 TDengine trades 表
docker exec tdengine taos -s "USE trading; SELECT * FROM trades ORDER BY ts DESC LIMIT 5;"

# 2. 查看 K-Line streams 状态
docker exec tdengine taos -s "USE trading; SHOW STREAMS;"

# 3. 查看 K-Line 数据
docker exec tdengine taos -s "USE trading; SELECT * FROM klines_1m LIMIT 5;"

# 4. 测试 API
curl "http://localhost:8080/api/v1/klines?interval=1m&limit=10" | jq .
```

#### 预期 API 响应

```json
{
  "code": 0,
  "msg": "ok",
  "data": [
    {
      "symbol": "BTC_USDT",
      "interval": "1m",
      "open_time": 1734611580000,
      "open": "37000.00",
      "high": "37000.00",
      "low": "37000.00",
      "close": "37000.00",
      "volume": "0.400000",
      "quote_volume": "14800.00",
      "trade_count": 8
    }
  ]
}
```

> [!WARNING]
> **待修复 (P0)**: K-Line API 需对齐 Binance 行业标准
>
> | # | 问题 | 当前 | Binance 标准 |
> |---|------|------|--------------|
> | 1 | `open_time` | ISO 8601 字符串 | **Unix 毫秒** (Number) |
> | 2 | `close_time` | 缺失 | **Unix 毫秒** (Number) |
>
> ```rust
> // 当前: "open_time": "2025-12-19T19:33:00+08:00"
> // 应为: "open_time": 1734611580000, "close_time": 1734611639999
> ```

> [!NOTE]
> **可选 (P2)**: Binance 额外字段
> - `taker_buy_base_volume` - Taker 买入基础资产量
> - `taker_buy_quote_volume` - Taker 买入计价资产量
> (需要 Settlement 额外记录 Taker 方向)

> [!TIP]
> `quote_volume` = volume × price = 0.4 BTC × 37000 = 14800 USDT

> [!NOTE]
> K-Line Stream 是增量处理的。如果 API 返回空数据，可能需要等待时间窗口关闭（1分钟后）。

---

## Summary

本章实现 K-Line 聚合服务：

| 设计点 | 方案 |
|--------|------|
| 数据结构 | OHLCV + trade_count |
| 时间周期 | 1m, 5m, 15m, 30m, 1h, 1d |
| 数据源 | 从成交事件实时聚合 |
| 存储 | TDengine (klines Super Table) |
| 推送 | WebSocket kline.update |

**核心理念**：

> K-Line 是**衍生数据**：从成交事件实时计算，而非存储原始数据。

---

## 未来优化

### Binance Array 格式 (P3)

当前使用 **对象格式**，Binance 使用 **数组格式** 更高效：

```json
// 当前 (对象格式)
{
  "symbol": "BTC_USDT",
  "interval": "1m",
  "open_time": 1734611580000,
  "close_time": 1734611639999,
  "open": "37000.00",
  ...
}

// Binance (数组格式 - 更高效)
[
  1734611580000,    // open_time
  "37000.00",       // open
  "37500.00",       // high
  "36800.00",       // low
  "37200.00",       // close
  "0.400000",       // volume
  1734611639999,    // close_time
  "14800.00",       // quote_volume
  8,                // trade_count
  "0.200000",       // taker_buy_base_volume
  "7400.00",        // taker_buy_quote_volume
  "0"               // ignore
]
```

**优势**:
- 更小的 JSON 体积 (无字段名)
- 更快的解析速度
- 完全兼容 Binance 客户端

**实施时机**: 当 API 客户端数量增加，带宽成为瓶颈时
