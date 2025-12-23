# 0x09-e Order Book Depth

<h3>
  <a href="#-english">🇺🇸 English</a>
  &nbsp;&nbsp;&nbsp;|&nbsp;&nbsp;&nbsp;
  <a href="#-chinese">🇨🇳 中文</a>
</h3>

<div id="-english"></div>

## 🇺🇸 English

> **📦 Code Changes**: [View Diff](https://github.com/gjwang/zero_x_infinity/compare/v0.9-d-kline-aggregation...v0.9-e-orderbook-depth)

> **Core Objective**: Implement Order Book Depth push, allowing users to view the current buy/sell order distribution in real-time.

---

## Background: Depth Data

The Order Book Depth displays the current market's distribution of limit orders:

```
         Asks (Sells)                   
  ┌─────────────────────┐              
  │ 30100.00   0.3 BTC  │ ← Lowest Ask
  │ 30050.00   0.5 BTC  │              
  │ 30020.00   1.2 BTC  │              
  ├─────────────────────┤              
  │    Current: 30000   │              
  ├─────────────────────┤              
  │ 29980.00   0.8 BTC  │              
  │ 29950.00   1.5 BTC  │              
  │ 29900.00   2.0 BTC  │ ← Highest Bid
  └─────────────────────┘              
         Bids (Buys)                   
```

---

## 1. Data Structure

### 1.1 Depth Response Format

```json
{
    "symbol": "BTC_USDT",
    "bids": [
        ["29980.00", "0.800000"],
        ["29950.00", "1.500000"],
        ["29900.00", "2.000000"]
    ],
    "asks": [
        ["30020.00", "1.200000"],
        ["30050.00", "0.500000"],
        ["30100.00", "0.300000"]
    ],
    "last_update_id": 12345
}
```

### 1.2 Binance Format Comparison

| Field | Us | Binance |
|-------|----|---------|
| bids | `[["price", "qty"], ...]` | ✅ Match |
| asks | `[["price", "qty"], ...]` | ✅ Match |
| last_update_id | `12345` | ✅ Match |

---

## 2. API Design

### 2.1 HTTP Endpoint

`GET /api/v1/depth?symbol=BTC_USDT&limit=20`

| Parameter | Type | Description |
|-----------|------|-------------|
| symbol | String | Trading Pair |
| limit | u32 | Depth levels (5, 10, 20, 50, 100) |

### 2.2 WebSocket Push

```json
// Subscribe
{"type": "subscribe", "channel": "depth", "symbol": "BTC_USDT"}

// Push (Incremental)
{
    "type": "depth.update",
    "symbol": "BTC_USDT",
    "bids": [["29980.00", "0.800000"]],
    "asks": [["30020.00", "0.000000"]],  // qty=0 means removal
    "last_update_id": 12346
}
```

---

## 3. Architecture Design

### 3.1 Comparison with K-Line

| Data | Source | Latency | Method |
|------|--------|---------|--------|
| K-Line | Historical Trades | Minute-level | TDengine Stream |
| **Depth** | Current Orders | **Ms-level** | In-Memory |

Depth is too real-time for DB storage. We use **Ring Buffer + Independent Service**.

### 3.2 Event-Driven Architecture

Following the pattern: **Isolated service, Ring Buffer, Lock-Free**.

```
┌────────────┐                    ┌─────────────────────┐
│     ME     │ ──(non-blocking)─► │ depth_event_queue   │
│            │    drop if full    │ (capacity: 1024)    │
1└────────────┘                   └──────────┬──────────┘
                                             │
                                             ▼
                                  ┌─────────────────────┐
                                  │   DepthService      │
                                  │   (tokio async)     │
                                  ├─────────────────────┤
                                  │ ● HTTP Snapshot     │
                                  │ ● WS Incremental    │
                                  └─────────────────────┘
```

> [!IMPORTANT]
> **Market Data Characteristic**: Freshness is key. Dropping a few events is acceptable if the consumer is slow, as eventual consistency is restored by snapshots.

---

## 4. Module Structure

```
src/
├── gateway/
│   ├── handlers.rs     # Add get_depth
│   └── ...
├── engine.rs           # Add get_depth() method
└── websocket/
    └── messages.rs     # Add DepthUpdate
```

---

## 5. Implementation Plan

*   [x] **Phase 1: HTTP API**: Add `OrderBook::get_depth()`, API endpoint.
*   [ ] **Phase 2: WebSocket**: `depth.update` message, subscription Logic.

---

## 6. Verification

### 6.1 E2E Test Scenarios

Script: `scripts/test_depth.sh`

1.  Query empty depth.
2.  Submit Buy/Sell orders (creating depth).
3.  Wait for update (200ms).
4.  Query depth and verify bids/asks.
5.  Performance test (100 orders rapid fire).

Expected Result:
*   Depth reflects order book state.
*   Update latency ≤ 100ms.
*   High frequency updates are batched/throttled correctly.

---

## Summary

| Point | Implementation |
|-------|----------------|
| Structure | Compatible with Binance (Array format) |
| API | `GET /api/v1/depth` |
| WebSocket | `depth.update` (Future: Incremental) |
| Architecture | Event-driven, Ring Buffer |

**Core Concept**:
> **Service Isolation**: ME pushes via DepthEvent. DepthService maintains state. Lock-free.

Next Chapter: **0x09-f Integration Test**.

<br>
<div align="right"><a href="#-english">↑ Back to Top</a></div>
<br>

---

<div id="-chinese"></div>

## 🇨🇳 中文

> **📦 代码变更**: [查看 Diff](https://github.com/gjwang/zero_x_infinity/compare/v0.9-d-kline-aggregation...v0.9-e-orderbook-depth)

> **本节核心目标**：实现 Order Book 盘口深度推送，让用户实时看到买卖挂单分布。

---

## 背景：盘口数据

交易所盘口展示当前市场的买卖挂单分布：

```
         卖单 (Asks)                   
  ┌─────────────────────┐              
  │ 30100.00   0.3 BTC  │ ← 最低卖价   
  │ 30050.00   0.5 BTC  │              
  │ 30020.00   1.2 BTC  │              
  ├─────────────────────┤              
  │     当前价格: 30000 │              
  ├─────────────────────┤              
  │ 29980.00   0.8 BTC  │              
  │ 29950.00   1.5 BTC  │              
  │ 29900.00   2.0 BTC  │ ← 最高买价   
  └─────────────────────┘              
         买单 (Bids)                   
```

---

## 1. 数据结构

### 1.1 Depth 响应格式

```json
{
    "symbol": "BTC_USDT",
    "bids": [
        ["29980.00", "0.800000"],
        ["29950.00", "1.500000"],
        ["29900.00", "2.000000"]
    ],
    "asks": [
        ["30020.00", "1.200000"],
        ["30050.00", "0.500000"],
        ["30100.00", "0.300000"]
    ],
    "last_update_id": 12345
}
```

### 1.2 Binance 格式对比

| 字段 | 我们 | Binance |
|------|------|---------|
| bids | `[["price", "qty"], ...]` | ✅ 相同 |
| asks | `[["price", "qty"], ...]` | ✅ 相同 |
| last_update_id | `12345` | ✅ 相同 |

---

## 2. API 设计

### 2.1 HTTP 端点

`GET /api/v1/depth?symbol=BTC_USDT&limit=20`

| 参数 | 类型 | 描述 |
|------|------|------|
| symbol | String | 交易对 |
| limit | u32 | 档位数量 (5, 10, 20, 50, 100) |

### 2.2 WebSocket 推送

`depth.update` (增量更新)，`qty=0` 表示删除。

---

## 3. 架构设计

### 3.1 与 K-Line 的对比

| 数据 | 来源 | 时效性 | 处理方式 |
|------|------|--------|----------|
| K-Line | 历史成交 | 分钟级别 | TDengine 流计算 |
| **Depth** | 当前挂单 | **毫秒级** | 内存状态 |

Depth 太实时，不适合存数据库——使用 **ring buffer + 独立服务** 模式。

### 3.2 事件驱动架构

延续项目一贯的设计：**服务独立，通过 ring buffer 通信，lock-free**。

```
┌────────────┐                    ┌─────────────────────┐
│     ME     │ ──(non-blocking)─► │ depth_event_queue   │
│            │    drop if full    │ (capacity: 1024)    │
└────────────┘                    └──────────┬──────────┘
```

---

## 4. 模块结构

```
src/
├── gateway/
│   ├── handlers.rs     # 添加 get_depth
├── engine.rs           # 添加 get_depth()
└── websocket/
    └── messages.rs     # 添加 DepthUpdate
```

---

## 5. 实现计划

- [x] **Phase 1: HTTP API**: 实现 `OrderBook::get_depth` 和 API。
- [ ] **Phase 2: WebSocket**: 增量推送 (可选)。

---

## 6. 验证计划

运行 `scripts/test_depth.sh`:
1.  查询空盘口
2.  提交买卖单
3.  验证盘口数据更新
4.  性能验证 (100ms 更新频率)

---

## Summary

| 设计点 | 方案 |
|--------|------|
| 数据结构 | bids/asks 数组，Binance 兼容 |
| HTTP API | `GET /api/v1/depth` |
| WebSocket | `depth.update` (增量) |
| 架构 | 事件驱动，Ring Buffer 通信 |

**核心理念**：
> **服务隔离**：ME 通过 DepthEvent 推送，DepthService 维护独立状态，lock-free。

下一章 (0x09-f) 将进行集成测试。
