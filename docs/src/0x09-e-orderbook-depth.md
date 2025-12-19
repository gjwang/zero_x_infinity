# 0x09-e Order Book Depth: 盘口深度

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
        ["29980.00", "0.800000"],  // [price, qty]
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

```
GET /api/v1/depth?symbol=BTC_USDT&limit=20
```

| 参数 | 类型 | 描述 |
|------|------|------|
| symbol | String | 交易对 |
| limit | u32 | 档位数量 (5, 10, 20, 50, 100) |

### 2.2 WebSocket 推送

```json
// 订阅
{"type": "subscribe", "channel": "depth", "symbol": "BTC_USDT"}

// 推送 (增量更新)
{
    "type": "depth.update",
    "symbol": "BTC_USDT",
    "bids": [["29980.00", "0.800000"]],
    "asks": [["30020.00", "0.000000"]],  // qty=0 表示删除
    "last_update_id": 12346
}
```

> [!NOTE]
> 增量更新模式：`qty=0` 表示该价位已无挂单

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
                                             │
                                             ▼
                                  ┌─────────────────────┐
                                  │   DepthService      │
                                  │   (tokio async)     │
                                  ├─────────────────────┤
                                  │ ● HTTP 快照查询     │
                                  │ ● WS 增量推送       │
                                  └─────────────────────┘
```

### 3.3 DepthEvent

ME 在三个时机发送事件：

```rust
pub enum DepthEvent {
    OrderRested { price: u64, qty: u64, side: Side },     // 挂单
    TradeFilled { price: u64, qty: u64, side: Side },     // 成交 (maker 侧)
    OrderCancelled { price: u64, qty: u64, side: Side },  // 取消
}
```

**发送方式**（非阻塞）：
```rust
let _ = depth_event_queue.push(event);  // 满就丢，ME 不等待
```

> [!IMPORTANT]
> **Market Data 特性**：最新数据最重要，丢几条事件不影响最终一致性。

## 4. 模块结构

```
src/
├── gateway/
│   ├── handlers.rs     # 添加 get_depth
│   └── mod.rs          # 添加路由
├── engine.rs           # 添加 get_depth() 方法
└── websocket/
    └── messages.rs     # 添加 DepthUpdate
```

---

## 5. 实现计划

### Phase 1: HTTP API
- [ ] OrderBook 添加 `get_depth(limit)` 方法
- [ ] 添加 `GET /api/v1/depth` 端点
- [ ] 格式化输出 (display_decimals)

### Phase 2: WebSocket 推送 (可选)
- [ ] depth.update 消息类型
- [ ] 订阅机制
- [ ] 增量更新触发

---

## 6. 验证计划

### 6.1 单元测试

```rust
#[test]
fn test_get_depth() {
    let mut book = OrderBook::new();
    // 添加订单
    book.add_order(...);
    
    let depth = book.get_depth(5);
    assert_eq!(depth.bids.len(), 5);
    assert_eq!(depth.asks.len(), 5);
}
```

### 6.2 E2E 测试

```bash
# 1. 启动 Gateway
cargo run --release -- --gateway --port 8080

# 2. 提交订单创建盘口
./scripts/test_depth.sh

# 3. 查询盘口
curl "http://localhost:8080/api/v1/depth?symbol=BTC_USDT&limit=5" | jq .

# 预期响应
{
  "code": 0,
  "msg": "ok",
  "data": {
    "symbol": "BTC_USDT",
    "bids": [["30000.00", "0.100000"]],
    "asks": [],
    "last_update_id": 1
  }
}
```

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
