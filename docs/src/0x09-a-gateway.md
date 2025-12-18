# 0x09-a Gateway: 客户端接入层 (Client Access Layer)

> **📦 代码变更**: [查看 Diff](https://github.com/gjwang/zero_x_infinity/compare/v0.8-h-performance-monitoring...v0.9-a-gateway)

> **本节核心目标**：实现一个**轻量级**的 HTTP Gateway，连接客户端与交易核心系统。

---

## 背景：从核心到完整 MVP

在前面的章节中，我们已经构建了一个功能完整的**交易核心系统**：

| 组件 | 状态 | 章节 |
|------|------|------|
| OrderBook (BTreeMap) | ✅ | 0x04 |
| Balance Management | ✅ | 0x05-0x06 |
| Matching Engine | ✅ | 0x08 |
| Multi-Thread Pipeline | ✅ | 0x08-f/g |
| Performance Monitoring | ✅ | 0x08-h |

但要成为一个可用的 **MVP (Minimum Viable Product)**，还需要以下辅助系统：

```
┌─────────────────────────────────────────────────────────────────────────┐
│                        Complete Trading System MVP                       │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│  Client (Web/Mobile/API)                                                 │
│       │                                                                  │
│       ▼                                                                  │
│  ┌─────────────────┐                                                     │
│  │   0x09-a        │  ← 本章：接收订单，返回响应                           │
│  │   Gateway       │                                                     │
│  └────────┬────────┘                                                     │
│           │                                                              │
│           ▼                                                              │
│  ┌─────────────────────────────────────────────────────────────────┐     │
│  │              Trading Core (已完成)                               │     │
│  │  Ingestion → UBSCore → ME → Settlement                          │     │
│  └─────────────────────────────────────────────────────────────────┘     │
│           │                                                              │
│           ├──────────────────────────────────────────────────────────────│
│           │                                                              │
│  ┌────────▼────────┐  ┌────────────────┐  ┌────────────────┐             │
│  │   0x09-b        │  │   0x09-c       │  │   0x09-d       │             │
│  │   Settlement    │  │   K-Line       │  │   WebSocket    │             │
│  │   Persistence   │  │   Aggregation  │  │   Push         │             │
│  │   (DB Write)    │  │   (Candles)    │  │   (Real-time)  │             │
│  └─────────────────┘  └────────────────┘  └────────────────┘             │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

### 0x09 系列章节规划

| 章节 | 主题 | 核心功能 |
|------|------|----------|
| **0x09-a** | Gateway | HTTP/WS 订单接入、Pre-Check |
| 0x09-b | Settlement Persistence | 用户余额、订单、成交入库 |
| 0x09-c | K-Line Aggregation | 实时 K 线聚合 |
| 0x09-d | WebSocket Push | 实时行情推送 |

---

## 1. Gateway 设计

### 1.1 职责

Gateway 是**客户端与交易系统的唯一入口**：

| 职责 | 说明 |
|------|------|
| **协议转换** | HTTP/WebSocket → 内部消息格式 |
| **身份验证** | API Key / JWT 认证 |
| **Pre-Check** | 快速余额校验，过滤无效请求 |
| **限流** | Rate Limiting，防止 DDoS |
| **响应** | 同步返回订单接收确认 |

### 1.2 为什么 Gateway + Trading Core 分离？

| 架构 | 问题 | 分离后优势 |
|------|------|-----------|
| Gateway 直接处理订单 | 网络 I/O 阻塞撮合 | 网络处理与撮合解耦 |
| 单点架构 | 无法水平扩展 | Gateway 可多实例部署 |
| 同步处理 | 延迟不可控 | 异步队列，延迟可预测 |

### 1.3 技术选型

| 组件 | 选择 | 理由 |
|------|------|------|
| HTTP Framework | `axum` | 高性能、tokio 原生、类型安全 |
| WebSocket | `tokio-tungstenite` | 成熟稳定 |
| Serialization | `serde` + JSON | 标准、调试友好 |
| Rate Limiting | `tower` middleware | 可组合、生产级 |

---

## 2. 核心数据流

### 2.1 订单提交流程

```
┌──────────┐    HTTP POST    ┌──────────┐    Ring Buffer   ┌──────────┐
│  Client  │ ───────────────▶│ Gateway  │ ─────────────────▶│ Ingestion│
│          │                 │          │                   │  Stage   │
│          │◀─────────────── │          │                   │          │
└──────────┘  202 Accepted   └──────────┘                   └──────────┘
                   +                                              │
              order_id                                            ▼
              seq_id                                        Trading Core
```

### 2.2 Pre-Check 流程

```rust
async fn submit_order(order: OrderRequest) -> Result<OrderResponse, ApiError> {
    // 1. 参数校验
    validate_order(&order)?;
    
    // 2. 身份验证 (从 Header 获取)
    let user_id = authenticate(&headers)?;
    
    // 3. Pre-Check: 余额是否足够 (只读查询)
    let balance = ubscore.query_balance(user_id, order.asset_id).await?;
    let required = calculate_required(&order);
    if balance.avail < required {
        return Err(ApiError::InsufficientBalance);
    }
    
    // 4. 分配 order_id 和 client_order_id
    let order_id = id_generator.next();
    
    // 5. 发送到 Ring Buffer
    order_queue.push(SequencedOrder {
        order_id,
        user_id,
        ...order,
    })?;
    
    // 6. 返回接收确认 (异步处理)
    Ok(OrderResponse {
        order_id,
        status: "PENDING",
        accepted_at: now(),
    })
}
```

**关键点**：
- Pre-Check 是**尽力而为**的检查，不保证 100% 准确
- 最终的余额锁定在 UBSCore 执行
- Gateway 返回 `202 Accepted` 表示"已接收，异步处理中"

---

## 3. API 设计

### 3.1 RESTful Endpoints

| Method | Path | 描述 |
|--------|------|------|
| `POST` | `/api/v1/create_order` | 提交订单 |
| `POST` | `/api/v1/cancel_order` | 取消订单 |
| `GET` | `/api/v1/order/{order_id}` | 查询订单状态 |
| `GET` | `/api/v1/order_history` | 查询用户订单列表 |
| `GET` | `/api/v1/trade_history` | 查询成交历史 |
| `GET` | `/api/v1/balances` | 查询用户余额 |

### 3.2 请求/响应格式

#### 提交订单

```json
// POST /api/v1/create_order
// Request
{
    "cid": "my-order-001",  // client_order_id (可选)
    "symbol": "BTC_USDT",
    "side": "BUY",          // BUY | SELL (SCREAMING_CASE)
    "type": "LIMIT",        // LIMIT | MARKET (SCREAMING_CASE)
    "price": "85000.00",    // LIMIT 订单必填
    "qty": "0.001"          // 数量 (统一使用 qty)
}

// Response (202 Accepted)
{
    "code": 0,              // 0 = 成功, 非0 = 错误码
    "msg": "ok",
    "data": {
        "order_id": 1001,
        "cid": "my-order-001",
        "order_status": "ACCEPTED",  // ACCEPTED | REJECTED
        "accepted_at": 1734533784000
    }
}
```

#### 取消订单

```json
// POST /api/v1/cancel_order
// Request
{
    "order_id": 1001
}

// Response (200 OK)
{
    "code": 0,
    "msg": "ok",
    "data": {
        "order_id": 1001,
        "order_status": "CANCEL_PENDING"
    }
}
```

### 3.3 统一响应格式

**所有 API 响应统一使用以下格式**:

```json
{
    "code": 0,          // 0 = 成功, 非0 = 错误码
    "msg": "ok",        // 消息描述 (简短)
    "data": {}          // 实际数据 (成功时) 或 null (失败时)
}
```

**设计原则**:
- `code` 而非 `status`: 避免与 HTTP status 混淆
- `msg` 而非 `message`: 简短明确，减少流量
- `data`: 统一的数据容器

#### 成功响应

```json
// 成功示例
{
    "code": 0,
    "msg": "ok",
    "data": {
        "order_id": 1001,
        "cid": "my-order-001",
        "order_status": "ACCEPTED",
        "accepted_at": 1734533784000
    }
}
```

#### 错误响应

```json
// 错误示例 (400 Bad Request)
{
    "code": 1001,       // 业务错误码
    "msg": "Invalid parameter: price must be greater than zero",
    "data": null
}

// 错误示例 (401 Unauthorized)
{
    "code": 2001,
    "msg": "Missing X-User-ID header",
    "data": null
}
```

### 3.4 错误码设计

**简化的错误码方案** (不使用 HTTP*100):

| Code | 说明 | HTTP Status |
|------|------|-------------|
| 0 | 成功 | 200/202 |
| 1001 | 参数格式错误 | 400 |
| 1002 | 余额不足 | 400 |
| 1003 | 价格/数量无效 | 400 |
| 2001 | 缺少认证信息 | 401 |
| 2002 | 认证失败 | 401 |
| 4001 | 订单不存在 | 404 |
| 4291 | 请求过于频繁 | 429 |
| 5001 | 服务不可用 (队列满) | 503 |

### 3.4 API 规范遵循

> **重要**: Gateway API 必须遵循 [API Conventions](./api-conventions.md) 规范

**关键规则**:

1. **枚举值使用 SCREAMING_CASE**
   - `side`: `"BUY"` | `"SELL"` (不是 `"buy"` 或 `"Buy"`)
   - `type`: `"LIMIT"` | `"MARKET"`
   - `status`: `"ACCEPTED"` | `"REJECTED"` | `"CANCEL_PENDING"`

2. **字段命名一致性**
   - 使用 `qty` 而不是 `quantity` (与内部 `InternalOrder` 一致)
   - 使用 `cid` 作为 `client_order_id` 的简写

3. **错误码使用 SCREAMING_SNAKE_CASE**
   - `INVALID_PARAMETER`, `INSUFFICIENT_BALANCE`, `RATE_LIMITED`

**参考**: 与 Binance/FTX/OKX 等主流交易所 API 保持一致


---

## 4. WebSocket 实时推送

### 4.1 连接流程

```
┌──────────┐    WS Connect    ┌──────────┐
│  Client  │ ─────────────────▶│ Gateway  │
│          │                   │          │
│          │    Auth Token     │          │
│          │ ─────────────────▶│          │
│          │                   │          │
│          │◀───────────────── │          │
│          │    Connected      │          │
└──────────┘                   └──────────┘
```

### 4.2 订阅频道

```json
// 订阅订单状态更新
{
    "action": "subscribe",
    "channel": "order_updates"
}

// 订阅余额变更
{
    "action": "subscribe",
    "channel": "balance_updates"
}

// 订阅成交 (公开)
{
    "action": "subscribe",
    "channel": "trades",
    "symbol": "BTC_USDT"
}
```

### 4.3 推送消息格式

```json
// 订单状态变更
{
    "channel": "order_updates",
    "data": {
        "order_id": 1001,
        "status": "FILLED",
        "filled_qty": "0.001",
        "avg_price": "85000.00",
        "timestamp": 1734533785000
    }
}

// 余额变更
{
    "channel": "balance_updates",
    "data": {
        "asset": "USDT",
        "available": "9915.00",
        "frozen": "0.00",
        "timestamp": 1734533785000
    }
}
```

---

## 5. 安全设计

### 5.1 身份验证

| 方法 | 适用场景 | 说明 |
|------|----------|------|
| **API Key + Secret** | 程序化交易 | HMAC-SHA256 签名 |
| **JWT Token** | Web/Mobile | 短期有效，需刷新 |

#### HMAC 签名示例

```python
# Python 客户端示例
import hmac
import hashlib
import time

api_key = "your_api_key"
secret = "your_secret"

timestamp = str(int(time.time() * 1000))
body = '{"symbol":"BTC_USDT","side":"BUY",...}'

# 签名 = HMAC-SHA256(secret, timestamp + body)
signature = hmac.new(
    secret.encode(),
    (timestamp + body).encode(),
    hashlib.sha256
).hexdigest()

headers = {
    "X-API-KEY": api_key,
    "X-TIMESTAMP": timestamp,
    "X-SIGNATURE": signature,
}
```

### 5.2 Rate Limiting

| 资源 | 限制 | 窗口 |
|------|------|------|
| 订单提交 | 10 req/s | 滑动窗口 |
| 订单取消 | 10 req/s | 滑动窗口 |
| 查询 | 100 req/s | 滑动窗口 |
| WebSocket 消息 | 100 msg/s | - |

---

## 6. 通信架构设计

### 6.1 通信方案选择

| 方案 | 延迟 | 复杂度 | 适用场景 | 选择 |
|------|------|--------|----------|------|
| **同进程 + Ring Buffer** | ~100ns | ⭐ | **MVP** | ✅ 采用 |
| 跨进程 SharedMem | ~1µs | ⭐⭐⭐ | 分离部署 | 未来 |
| TCP/Unix Socket | ~10µs | ⭐⭐ | 分布式 | 未来 |
| gRPC | ~100µs | ⭐⭐ | 微服务 | 未来 |

**MVP 决策**：Gateway 和 Trading Core 运行在**同一进程**中，通过 `Arc<ArrayQueue>` 通信。

**优势**：
- ✅ 零改动：直接复用现有的 `crossbeam::ArrayQueue`
- ✅ 最低延迟：无需序列化，无网络开销
- ✅ 最简单：不需要额外的通信协议
- ✅ 易于集成测试：单进程启动

### 6.2 MVP 架构：同进程 Ring Buffer

```
┌─────────────────────────────────────────────────────────────────────────┐
│                     Single Process (--gateway mode)                      │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│  ┌──────────────────────────────────────────────────────────────────┐   │
│  │                    HTTP/WS Server (tokio runtime)                 │   │
│  │  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐         │   │
│  │  │ POST /order  │    │ DELETE /order│    │ WS Handler   │         │   │
│  │  │              │    │              │    │              │         │   │
│  │  └──────┬───────┘    └──────┬───────┘    └──────────────┘         │   │
│  │         │                   │                                      │   │
│  │         └───────────────────┴──────────────────┐                   │   │
│  │                                                │                   │   │
│  │                                                ▼                   │   │
│  │  ┌──────────────────────────────────────────────────────────────┐ │   │
│  │  │             order_queue: Arc<ArrayQueue<OrderAction>>        │ │   │
│  │  │                        (共享 Ring Buffer)                    │ │   │
│  │  └──────────────────────────────────────────────────────────────┘ │   │
│  └──────────────────────────────────────────────────────────────────┘   │
│                                                │                         │
│                                                │ 同一进程内直接访问        │
│                                                ▼                         │
│  ┌──────────────────────────────────────────────────────────────────┐   │
│  │                    Trading Core Threads                           │   │
│  │                                                                    │   │
│  │   Thread 1: Ingestion    (消费 order_queue)                       │   │
│  │   Thread 2: UBSCore      (WAL + Lock + Settle)                    │   │
│  │   Thread 3: ME           (Matching + Cancel)                      │   │
│  │   Thread 4: Settlement   (Persistence)                            │   │
│  │                                                                    │   │
│  └──────────────────────────────────────────────────────────────────┘   │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

### 6.3 代码结构

```rust
// main.rs
fn main() {
    // 共享的 Ring Buffer
    let queues = Arc::new(MultiThreadQueues::new());
    
    if args.gateway {
        // 模式1: Gateway + Trading Core (同进程)
        let queues_clone = queues.clone();
        
        // 启动 HTTP Server (tokio runtime)
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(run_http_server(queues_clone));
        });
        
        // 启动 Trading Core (现有代码)
        run_pipeline_multi_thread(queues, ...);
    } else {
        // 模式2: 原有的 CSV 批量处理模式
        run_pipeline_multi_thread(queues, ...);
    }
}
```

### 6.4 演进路径

```
Phase 1 (MVP - 当前):  同进程 Ring Buffer
                            ↓
Phase 2:               Unix Socket (同机多进程，可独立重启)
                            ↓
Phase 3:               TCP + 自定义协议 (跨机部署)
                            ↓
Phase 4:               Kafka/Redpanda (高可用，多消费者)
```

### 6.5 部署拓扑 (未来 - Phase 3+)

```
                    Load Balancer
                         │
         ┌───────────────┼───────────────┐
         ▼               ▼               ▼
    ┌─────────┐     ┌─────────┐     ┌─────────┐
    │Gateway 1│     │Gateway 2│     │Gateway N│  ← 无状态，可水平扩展
    └────┬────┘     └────┬────┘     └────┬────┘
         │               │               │
         └───────────────┼───────────────┘
                         │ TCP / Kafka
                         ▼
                ┌─────────────────┐
                │  Trading Core   │  ← 单点，保证顺序
                │  (Active)       │
                └─────────────────┘
                         │
                         ▼
                ┌─────────────────┐
                │   Database      │  ← 持久化层
                │   (PostgreSQL)  │
                └─────────────────┘
```

---

## 7. 实现规范

### 7.1 数据结构定义

#### 请求类型

```rust
// src/gateway/types.rs

/// 创建订单请求
#[derive(Debug, Deserialize)]
pub struct CreateOrderRequest {
    /// 客户端订单ID (可选)
    pub cid: Option<String>,
    /// 交易对
    pub symbol: String,
    /// 买卖方向: "BUY" | "SELL" (SCREAMING_CASE)
    pub side: String,
    /// 订单类型: "LIMIT" | "MARKET" (SCREAMING_CASE)
    #[serde(rename = "type")]
    pub order_type: String,
    /// 价格 (LIMIT 订单必填)
    pub price: Option<String>,
    /// 数量 (统一使用 qty)
    pub qty: String,
}

/// 取消订单请求
#[derive(Debug, Deserialize)]
pub struct CancelOrderRequest {
    pub order_id: u64,
}

/// 订单响应
#[derive(Debug, Serialize)]
pub struct OrderResponse {
    pub order_id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cid: Option<String>,
    /// 状态: "ACCEPTED" | "REJECTED" (SCREAMING_CASE)
    pub status: String,
    pub accepted_at: u64, // Unix timestamp (ms)
}

/// 错误响应
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: ErrorDetail,
}

#[derive(Debug, Serialize)]
pub struct ErrorDetail {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}
```

#### 应用状态

```rust
// src/gateway/state.rs

/// Gateway 应用状态 (共享)
pub struct AppState {
    /// 订单队列 (发送到 Trading Core)
    pub order_queue: Arc<ArrayQueue<OrderAction>>,
    /// Symbol Manager (只读)
    pub symbol_mgr: Arc<SymbolManager>,
    /// 活跃交易对 ID
    pub active_symbol_id: u32,
    /// 订单 ID 生成器
    pub order_id_gen: Arc<AtomicU64>,
}

impl AppState {
    pub fn new(
        order_queue: Arc<ArrayQueue<OrderAction>>,
        symbol_mgr: Arc<SymbolManager>,
        active_symbol_id: u32,
    ) -> Self {
        Self {
            order_queue,
            symbol_mgr,
            active_symbol_id,
            order_id_gen: Arc::new(AtomicU64::new(1)),
        }
    }
    
    pub fn next_order_id(&self) -> u64 {
        self.order_id_gen.fetch_add(1, Ordering::SeqCst)
    }
}
```

### 7.2 API Handler 实现要求

#### POST /api/v1/create_order

**职责**:
1. 解析 JSON 请求体
2. 验证参数 (symbol, side, type, price, qty)
3. 从 Header 提取 `X-User-ID`
4. 转换 decimal 字符串为 u64
5. 生成 order_id
6. 构造 `OrderAction::Place`
7. 推送到 `order_queue`
8. 返回 202 Accepted

**错误处理**:
- 400: 参数格式错误 (`INVALID_PARAMETER`)
- 401: 缺少 `X-User-ID` (`UNAUTHORIZED`)
- 503: 队列满 (`SERVICE_UNAVAILABLE`)

**示例代码框架**:

```rust
async fn create_order(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<CreateOrderRequest>,
) -> Result<(StatusCode, Json<OrderResponse>), (StatusCode, Json<ErrorResponse>)> {
    // 1. 提取 user_id
    let user_id = extract_user_id(&headers)?;
    
    // 2. 验证参数
    validate_create_order(&req)?;
    
    // 3. 转换价格和数量
    let symbol_info = state.symbol_mgr.get_symbol_info_by_id(state.active_symbol_id)
        .ok_or_else(|| error_response("INVALID_SYMBOL", "Symbol not found"))?;
    
    let price = parse_price(&req, symbol_info)?;
    let qty = parse_quantity(&req, symbol_info)?;
    
    // 4. 生成 order_id
    let order_id = state.next_order_id();
    
    // 5. 构造 OrderAction
    let order = InternalOrder { /* ... */ };
    let action = OrderAction::Place(SequencedOrder::new(order_id, order, now_ns()));
    
    // 6. 推送到队列
    state.order_queue.push(action)
        .map_err(|_| error_response("SERVICE_UNAVAILABLE", "Queue full"))?;
    
    // 7. 返回响应
    Ok((StatusCode::ACCEPTED, Json(OrderResponse {
        order_id,
        cid: req.cid,
        status: "ACCEPTED".to_string(),
        accepted_at: now_ms(),
    })))
}
```

#### POST /api/v1/cancel_order

**职责**:
1. 解析 JSON 请求体
2. 从 Header 提取 `X-User-ID`
3. 构造 `OrderAction::Cancel`
4. 推送到 `order_queue`
5. 返回 200 OK

**错误处理**:
- 400: 参数格式错误
- 401: 缺少 `X-User-ID`
- 503: 队列满

#### GET /api/v1/order/{order_id}

**Phase 2 实现** (需要数据库)

返回订单状态:
```json
{
  "order_id": 1001,
  "status": "FILLED",
  "filled_qty": "0.001",
  "avg_price": "85000.00"
}
```

### 7.3 启动模式

#### 命令行参数

```bash
# Gateway 模式 (HTTP + Trading Core)
cargo run --release -- --gateway --input fixtures/test_with_cancel_highbal

# 指定端口
cargo run --release -- --gateway --port 8080
```

#### main.rs 集成

```rust
fn use_gateway_mode() -> bool {
    std::env::args().any(|a| a == "--gateway")
}

fn get_port() -> u16 {
    let args: Vec<String> = std::env::args().collect();
    for i in 0..args.len() {
        if args[i] == "--port" && i + 1 < args.len() {
            return args[i + 1].parse().unwrap_or(8080);
        }
    }
    8080
}

fn main() {
    // ...
    
    if use_gateway_mode() {
        let port = get_port();
        let queues = Arc::new(MultiThreadQueues::new());
        
        // 启动 HTTP Server
        let queues_clone = queues.clone();
        let symbol_mgr_clone = symbol_mgr.clone();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                gateway::run_server(port, queues_clone, symbol_mgr_clone, active_symbol_id).await
            });
        });
        
        // 启动 Trading Core
        run_pipeline_multi_thread(/* ... */);
    } else {
        // 原有模式
    }
}
```

### 7.4 验收标准

#### 功能验收

- [ ] **F1**: 启动 `--gateway` 模式，HTTP 服务器在指定端口监听
- [ ] **F2**: POST /api/v1/create_order 返回 202 Accepted，包含 order_id
- [ ] **F3**: POST /api/v1/cancel_order 返回 200 OK
- [ ] **F4**: 缺少 `X-User-ID` 返回 401 Unauthorized
- [ ] **F5**: 参数格式错误返回 400 Bad Request
- [ ] **F6**: 订单成功推送到 `order_queue`，Trading Core 可消费

#### 集成测试

```bash
# 测试脚本: scripts/test_gateway.sh

# 1. 启动 Gateway
cargo run --release -- --gateway --port 8080 &
GATEWAY_PID=$!
sleep 2

# 2. 提交订单
curl -X POST http://localhost:8080/api/v1/create_order \
  -H "Content-Type: application/json" \
  -H "X-User-ID: 1001" \
  -d '{
    "symbol": "BTC_USDT",
    "side": "BUY",
    "type": "LIMIT",
    "price": "85000.00",
    "qty": "0.001"
  }'

# 3. 取消订单
curl -X POST http://localhost:8080/api/v1/cancel_order \
  -H "Content-Type: application/json" \
  -H "X-User-ID: 1001" \
  -d '{"order_id": 1}'

# 4. 清理
kill $GATEWAY_PID
```

#### 性能验收

- [ ] **P1**: 单个请求延迟 < 1ms (P99)
- [ ] **P2**: 支持 10,000 req/s 吞吐量
- [ ] **P3**: 队列满时返回 503，不阻塞其他请求

---


## 8. 测试策略


### 8.1 单元测试

```rust
#[tokio::test]
async fn test_submit_order() {
    let app = create_test_app().await;
    
    let response = app
        .post("/api/v1/order")
        .json(&OrderRequest { ... })
        .send()
        .await;
    
    assert_eq!(response.status(), 202);
    let body: OrderResponse = response.json().await;
    assert!(body.order_id > 0);
}
```

### 8.2 集成测试

```bash
# 启动 Gateway + Trading Core
cargo run --release -- --gateway

# 发送测试订单
curl -X POST http://localhost:8080/api/v1/order \
  -H "Content-Type: application/json" \
  -d '{"symbol":"BTC_USDT","side":"BUY","price":"85000","quantity":"0.001"}'
```

---

## Summary

本章设计了 Gateway 作为客户端接入层：

| 设计点 | 方案 |
|--------|------|
| HTTP Framework | axum (高性能、类型安全) |
| **通信方式** | **同进程 Ring Buffer** (MVP 阶段) |
| 订单提交 | 异步接收，返回 202 Accepted |
| Pre-Check | 只读余额查询，过滤无效订单 |
| 队列连接 | `Arc<ArrayQueue>` 共享 |
| 安全 | 简单 Header 认证 (MVP) → HMAC 签名 (未来) |

**核心理念**：

> Gateway 是**速度门卫**而不是**业务处理器**：快速接收、快速校验、快速转发。真正的业务逻辑在 Trading Core 执行。

下一章 (0x09-b) 将实现 Settlement Persistence，将成交数据持久化到数据库。
