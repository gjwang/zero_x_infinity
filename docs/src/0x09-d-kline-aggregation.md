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

## 2. 架构设计

### 2.1 数据流

```
┌───────────────────────────────────────────────────────────────────┐
│                        Trading Pipeline                            │
│                                                                     │
│  ME (MatchingEngine)  ──▶  trade_queue  ──▶  Settlement            │
│                                                  │                  │
│                                                  ├──▶ TDengine     │
│                                                  │                  │
│                                                  └──▶ KLineService │
└───────────────────────────────────────────────────────────────────┘
                                                          │
                                                          ▼
                                               ┌─────────────────────┐
                                               │   KLine Aggregator  │
                                               │                     │
                                               │  ┌───────────────┐  │
                                               │  │ 1m Aggregator │  │
                                               │  ├───────────────┤  │
                                               │  │ 5m Aggregator │  │
                                               │  ├───────────────┤  │
                                               │  │ 15m Aggregator│  │
                                               │  ├───────────────┤  │
                                               │  │ 1h Aggregator │  │
                                               │  ├───────────────┤  │
                                               │  │ 1d Aggregator │  │
                                               │  └───────────────┘  │
                                               └─────────────────────┘
                                                          │
                                          ┌───────────────┼───────────────┐
                                          ▼               ▼               ▼
                                      TDengine      WebSocket Push    HTTP API
```

### 2.2 时间窗口

| Interval | 窗口大小 (ms) | 说明 |
|----------|--------------|------|
| 1m | 60,000 | 每分钟一根 K 线 |
| 5m | 300,000 | 5分钟 |
| 15m | 900,000 | 15分钟 |
| 30m | 1,800,000 | 30分钟 |
| 1h | 3,600,000 | 1小时 |
| 1d | 86,400,000 | 1天 (UTC 00:00 切换) |

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
├── kline/
│   ├── mod.rs              # 模块入口
│   ├── aggregator.rs       # K-Line 聚合器
│   ├── interval.rs         # 时间间隔定义
│   ├── service.rs          # KLineService
│   └── storage.rs          # TDengine 存储
├── gateway/
│   ├── handlers.rs         # 添加 get_klines
│   └── mod.rs              # 添加路由
└── websocket/
    └── messages.rs         # 添加 KLineUpdate
```

---

## 5. 实现计划

### Phase 1: 基础聚合
- [ ] KLine 数据结构
- [ ] KLineInterval 枚举
- [ ] 单周期 Aggregator (1m)
- [ ] 成交驱动更新

### Phase 2: 多周期支持
- [ ] 5m, 15m, 30m, 1h, 1d Aggregators
- [ ] 时间窗口切换逻辑
- [ ] K 线完结事件

### Phase 3: 持久化
- [ ] TDengine 存储 (Super Table: klines)
- [ ] 历史 K 线查询

### Phase 4: 推送
- [ ] WebSocket kline.update 事件
- [ ] HTTP GET /api/v1/klines

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

### 6.2 集成测试

```bash
# 1. 启动 Gateway
cargo run --release -- --gateway --port 8080

# 2. 提交多笔订单
./scripts/test_kline.sh

# 3. 查询 K 线
curl "http://localhost:8080/api/v1/klines?symbol=BTC_USDT&interval=1m&limit=10"

# 4. 验证 WebSocket 推送
websocat ws://localhost:8080/ws?user_id=1001
# 观察 kline.update 事件
```

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
