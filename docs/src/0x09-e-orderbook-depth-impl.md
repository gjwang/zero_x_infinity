# 0x09-e Order Book Depth - 实现任务

> 详细设计见 [0x09-e-orderbook-depth.md](./0x09-e-orderbook-depth.md)

---

## Task 1: 添加 DepthEvent 和队列

**文件**: `src/pipeline.rs`

```rust
pub const DEPTH_EVENT_QUEUE_CAPACITY: usize = 1024;

pub enum DepthEvent {
    OrderRested { price: u64, qty: u64, side: Side },
    TradeFilled { price: u64, qty: u64, side: Side },
    OrderCancelled { price: u64, qty: u64, side: Side },
}

// MultiThreadQueues 添加
pub depth_event_queue: Arc<ArrayQueue<DepthEvent>>,
```

---

## Task 2: ME 发送事件

**文件**: `src/pipeline_services.rs`

在 `MatchingService::run()` 中：

| 触发点 | 事件 |
|--------|------|
| `rest_order()` 后 | `OrderRested` |
| trade 循环内 | `TradeFilled` (maker 侧) |
| `remove_order_by_id()` 后 | `OrderCancelled` |

```rust
// 非阻塞发送
let _ = self.queues.depth_event_queue.push(DepthEvent::OrderRested {
    price: order.price,
    qty: order.remaining_qty(),
    side: order.side,
});
```

---

## Task 3: 创建 DepthService

**文件**: `src/market/depth_service.rs` [NEW]

```rust
pub struct DepthService {
    bids: BTreeMap<u64, u64>,  // price → total_qty
    asks: BTreeMap<u64, u64>,
    update_id: u64,
    depth_queue: Arc<ArrayQueue<DepthEvent>>,
}

impl DepthService {
    pub fn get_snapshot(&self, limit: usize) -> DepthSnapshot { ... }
    pub async fn run_consumer_loop(&mut self) { ... }
}
```

---

## Task 4: HTTP 端点

**文件**: `src/gateway/handlers.rs`, `src/gateway/mod.rs`

```rust
// GET /api/v1/depth?symbol=BTC_USDT&limit=20
pub async fn get_depth(...) -> Result<Json<ApiResponse<DepthApiData>>, ...>
```

---

## 验证

```bash
# 1. 启动
cargo run -- --gateway -e dev

# 2. 提交订单
curl -X POST http://localhost:8080/api/v1/create_order \
  -H "Content-Type: application/json" \
  -H "X-User-ID: 1001" \
  -d '{"symbol":"BTC_USDT","side":"BUY","order_type":"LIMIT","price":"30000.00","qty":"0.1"}'

# 3. 查询深度
curl "http://localhost:8080/api/v1/depth?symbol=BTC_USDT&limit=5"
```

预期响应:
```json
{"code":0,"msg":"ok","data":{"symbol":"BTC_USDT","bids":[["30000.00","0.100000"]],"asks":[],"last_update_id":1}}
```
