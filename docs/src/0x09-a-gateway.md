# 0x09-a Gateway: Client Access Layer

<h3>
  <a href="#-english">🇺🇸 English</a>
  &nbsp;&nbsp;&nbsp;|&nbsp;&nbsp;&nbsp;
  <a href="#-chinese">🇨🇳 中文</a>
</h3>

<div id="-english"></div>

## 🇺🇸 English

> **📦 Code Changes**: [View Diff](https://github.com/gjwang/zero_x_infinity/compare/v0.8-h-performance-monitoring...v0.9-a-gateway)

> **Core Objective**: Implement a **lightweight** HTTP Gateway to connect clients with the trading core system.

---

## Background: From Core to MVP

We have built a functional **Trading Core**:
*   OrderBook (0x04)
*   Balance Management (0x05-0x06)
*   Matching Engine (0x08)
*   Pipeline & Monitoring (0x08-f/g/h)

To become a usable **MVP**, we need auxiliary systems:

```
┌─────────────────────────────────────────────────────────────────────────┐
│                        Complete Trading System MVP                       │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│  Client (Web/Mobile/API)                                                 │
│       │                                                                  │
│       ▼                                                                  │
│  ┌─────────────────┐                                                     │
│  │   0x09-a        │  ← This Chapter: Accept orders, return response     │
│  │   Gateway       │                                                     │
│  └────────┬────────┘                                                     │
│           │                                                                  │
│           ▼                                                                  │
│  ┌─────────────────────────────────────────────────────────────────┐     │
│  │              Trading Core (Completed)                            │     │
│  │  Ingestion → UBSCore → ME → Settlement                          │     │
│  └─────────────────────────────────────────────────────────────────┘     │
```

### 0x09 Series Plan

| Chapter | Topic | Core Function |
|---------|-------|---------------|
| **0x09-a** | Gateway | HTTP/WS Entry, Pre-Check |
| 0x09-b | Settlement Persistence | DB Persistence for Balances/Trades |
| 0x09-c | K-Line Aggregation | Real-time Candles |
| 0x09-d | WebSocket Push | Real-time Market Data |

---

## 1. Gateway Design

### 1.1 Responsibilities

The Gateway is the **sole entry point** for clients.

*   **Protocol Conversion**: HTTP/WebSocket → Internal Formats
*   **Authentication**: API Key / JWT
*   **Pre-Check**: Fast balance validation
*   **Rate Limiting**: Anti-DDoS
*   **Response**: Synchronous acknowledgment

### 1.2 Why Separate Gateway & Core?

*   **Decoupling**: Network I/O doesn't block matching.
*   **Scalability**: Gateway can scale horizontally.
*   **Predictability**: Async queues ensure predictable matching latency.

### 1.3 Tech Stack

*   **HTTP**: `axum` (High performance, tokio-native)
*   **WebSocket**: `tokio-tungstenite`
*   **Serialization**: `serde` + JSON
*   **Rate Limiting**: `tower` middleware

---

## 2. Core Data Flow

### 2.1 Order Submission

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

### 2.2 Pre-Check Logic

```rust
async fn submit_order(order: OrderRequest) -> Result<OrderResponse, ApiError> {
    // 1. Validation
    validate_order(&order)?;
    
    // 2. Auth
    let user_id = authenticate(&headers)?;
    
    // 3. Pre-Check: Balance (Read-Only)
    let balance = ubscore.query_balance(user_id, order.asset_id).await?;
    if balance.avail < required {
        return Err(ApiError::InsufficientBalance);
    }
    
    // 4. Assign ID
    let order_id = id_generator.next();
    
    // 5. Push to Ring Buffer
    order_queue.push(SequencedOrder { ... })?;
    
    // 6. Return Accepted
    Ok(OrderResponse { status: "PENDING", ... })
}
```

**Key Points**:
*   Pre-Check is "best effort".
*   Final locking happens in UBSCore.
*   Returns `202 Accepted` to indicate async processing.

---

## 3. API Design

### 3.1 RESTful Endpoints

*   `POST /api/v1/create_order`: Submit order
*   `POST /api/v1/cancel_order`: Cancel order
*   `GET /api/v1/order/{order_id}`: Query status

### 3.2 Request/Response Format

**Submit Order**:

```json
// POST /api/v1/create_order
{
    "symbol": "BTC_USDT",
    "side": "BUY",
    "type": "LIMIT",
    "price": "85000.00",
    "qty": "0.001"
}

// Response (202 Accepted)
{
    "code": 0,
    "msg": "ok",
    "data": {
        "order_id": 1001,
        "status": "ACCEPTED",
        "accepted_at": 1734533784000
    }
}
```

### 3.3 Unified Response Format

```json
{
    "code": 0,          // 0 = Success, Non-0 = Error
    "msg": "ok",        // Short description
    "data": {}          // Payload or null
}
```

### 3.4 API Conventions

> **Important**: Must follow [API Conventions](./api-conventions.md).

1.  **SCREAMING_CASE Enums**: `"BUY"`, `"SELL"`, `"LIMIT"`.
2.  **Naming**: `qty` (not quantity), `cid` (client_order_id).
3.  **SCREAMING_SNAKE_CASE Error Codes**: `INVALID_PARAMETER`.

---

## 4. WebSocket Push

### 4.1 Flow

Clients connect via WS, authenticate, and subscribe to channels.

### 4.2 Channels

*   `order_updates`: Private order status changes.
*   `balance_updates`: Private balance changes.
*   `trades`: Public trade feed.

---

## 5. Security

| Level | Method | Scenario |
|-------|--------|----------|
| **MVP** | Header `X-User-ID` | Internal / Reliability Testing |
| **Prod** | API Key (HMAC) | Programmatic Trading |
| **Prod** | JWT | Web/Mobile |

---

## 6. Communication Architecture

### 6.1 MVP Choice: Single Process Ring Buffer

Gateway and Trading Core run in the **same process**, communicating via `Arc<ArrayQueue>`.

**Pros**:
*   ✅ Zero network overhead (~100ns latency).
*   ✅ Reuse existing `crossbeam` queues.
*   ✅ Simple deployment.

### 6.2 Architecture Diagram

```
┌─────────────────────────────────────────────────────────────────────────┐
│                     Single Process (--gateway mode)                      │
├─────────────────────────────────────────────────────────────────────────┤
│  ┌─────────────────────────────┐                                         │
│  │ HTTP Server (tokio runtime) │                                         │
│  └──────────────┬──────────────┘                                         │
│                 │                                                        │
│                 ▼                                                        │
│  ┌─────────────────────────────┐                                         │
│  │         order_queue         │ (Shared Ring Buffer)                    │
│  └──────────────┬──────────────┘                                         │
│                 │                                                        │
│                 ▼                                                        │
│  ┌─────────────────────────────┐                                         │
│  │      Trading Core Threads   │                                         │
│  └─────────────────────────────┘                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

### 6.3 Evolution Path

1.  **MVP**: Single Process.
2.  **Phase 2**: Unix Domain Socket (Multi-process on same host).
3.  **Phase 3**: TCP / RPC (Distributed).

---

## 7. Implementation Guidelines

### 7.1 Startup Modes

```bash
# Gateway Mode
cargo run --release -- --gateway --port 8080

# Batch Mode (Original)
cargo run --release -- --pipeline-mt
```

### 7.2 Main Integration

```rust
if args.gateway {
    // Spawn HTTP Server in a thread
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(run_http_server(queues));
    });
    // Run Trading Core
    run_pipeline_multi_thread(queues, ...);
}
```

---

## Summary

This chapter implements the Gateway as the client access layer.

**Core Philosophy**:
> The Gateway is a **speed guard**, not a business processor. Accept fast, validate fast, forward fast.

<br>
<div align="right"><a href="#-english">↑ Back to Top</a></div>
<br>

---

<div id="-chinese"></div>

## 🇨🇳 中文

> **📦 代码变更**: [查看 Diff](https://github.com/gjwang/zero_x_infinity/compare/v0.8-h-performance-monitoring...v0.9-a-gateway)

> **本节核心目标**：实现一个**轻量级**的 HTTP Gateway，连接客户端与交易核心系统。

---

## 背景：从核心到完整 MVP

在前面的章节中，我们已经构建了一个功能完整的**交易核心系统**：
*   OrderBook (0x04)
*   Balance Management (0x05-0x06)
*   Matching Engine (0x08)
*   Pipeline (0x08-f/g/h)

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
│           │                                                                  │
│           ▼                                                                  │
│  ┌─────────────────────────────────────────────────────────────────┐     │
│  │              Trading Core (已完成)                               │     │
│  │  Ingestion → UBSCore → ME → Settlement                          │     │
│  └─────────────────────────────────────────────────────────────────┘     │
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

*   **协议转换**：HTTP/WebSocket → 内部消息格式
*   **身份验证**：API Key / JWT
*   **Pre-Check**：快速余额校验
*   **限流**：防止 DDoS
*   **响应**：同步返回接收确认

### 1.2 为什么 Gateway + Trading Core 分离？

*   **解耦**：网络 I/O 不阻塞撮合。
*   **扩展性**：Gateway 可水平扩展。
*   **可预测性**：异步队列确保撮合延迟可预测。

### 1.3 技术选型

*   **HTTP**: `axum` (高性能、tokio 原生)
*   **WebSocket**: `tokio-tungstenite`
*   **Serialization**: `serde` + JSON
*   **Rate Limiting**: `tower` middleware

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
    
    // 2. 身份验证
    let user_id = authenticate(&headers)?;
    
    // 3. Pre-Check: 余额检查 (只读)
    let balance = ubscore.query_balance(user_id, order.asset_id).await?;
    if balance.avail < required {
        return Err(ApiError::InsufficientBalance);
    }
    
    // 4. 分配 ID
    let order_id = id_generator.next();
    
    // 5. 推送到 Ring Buffer
    order_queue.push(SequencedOrder { ... })?;
    
    // 6. 返回接收确认
    Ok(OrderResponse { status: "PENDING", ... })
}
```

**关键点**：
*   Pre-Check 是"尽力而为"的检查。
*   最终锁定在 UBSCore 执行。
*   返回 `202 Accepted` 表示异步处理中。

---

## 3. API 设计

### 3.1 RESTful Endpoints

*   `POST /api/v1/create_order`: 提交订单
*   `POST /api/v1/cancel_order`: 取消订单
*   `GET /api/v1/order/{order_id}`: 查询状态

### 3.2 请求/响应格式

**提交订单**:

```json
// POST /api/v1/create_order
{
    "symbol": "BTC_USDT",
    "side": "BUY",
    "type": "LIMIT",
    "price": "85000.00",
    "qty": "0.001"
}

// Response (202 Accepted)
{
    "code": 0,
    "msg": "ok",
    "data": {
        "order_id": 1001,
        "status": "ACCEPTED",
        "accepted_at": 1734533784000
    }
}
```

### 3.3 统一响应格式

```json
{
    "code": 0,          // 0 = 成功, 非0 = 错误码
    "msg": "ok",        // 简短描述
    "data": {}          // 数据或 null
}
```

### 3.4 API 规范

> **重要**: 必须遵循 [API Conventions](./api-conventions.md) 规范。

1.  **大写枚举**: `"BUY"`, `"SELL"`, `"LIMIT"`。
2.  **命名一致**: `qty` (而非 quantity), `cid` (client_order_id)。
3.  **大写蛇形错误码**: `INVALID_PARAMETER`。

---

## 4. WebSocket 实时推送

### 4.1 流程

客户端连接 WS，认证，并订阅频道。

### 4.2 频道

*   `order_updates`: 私有订单状态变更。
*   `balance_updates`: 私有余额变更。
*   `trades`: 公共成交推送。

---

## 5. 安全设计

| 级别 | 方法 | 场景 |
|------|------|------|
| **MVP** | Header `X-User-ID` | 内部测试 |
| **Prod** | API Key (HMAC) | 程序化交易 |
| **Prod** | JWT | Web/移动端 |

---

## 6. 通信架构设计

### 6.1 MVP 选择：单进程 Ring Buffer

Gateway 和 Trading Core 运行在**同一进程**中，通过 `Arc<ArrayQueue>` 通信。

**优势**：
*   ✅ 零网络开销 (~100ns 延迟)。
*   ✅ 复用现有 `crossbeam` 队列。
*   ✅ 部署简单。

### 6.2 架构图

```
┌─────────────────────────────────────────────────────────────────────────┐
│                     Single Process (--gateway mode)                      │
├─────────────────────────────────────────────────────────────────────────┤
│  ┌─────────────────────────────┐                                         │
│  │ HTTP Server (tokio runtime) │                                         │
│  └──────────────┬──────────────┘                                         │
│                 │                                                        │
│                 ▼                                                        │
│  ┌─────────────────────────────┐                                         │
│  │         order_queue         │ (共享 Ring Buffer)                      │
│  └──────────────┬──────────────┘                                         │
│                 │                                                        │
│                 ▼                                                        │
│  ┌─────────────────────────────┐                                         │
│  │      Trading Core Threads   │                                         │
│  └─────────────────────────────┘                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

### 6.3 演进路径

1.  **MVP**: 单进程。
2.  **Phase 2**: Unix Domain Socket (同机多进程)。
3.  **Phase 3**: TCP / RPC (分布式)。

---

## 7. 实现指引

### 7.1 启动模式

```bash
# Gateway 模式
cargo run --release -- --gateway --port 8080

# 批量模式 (原有)
cargo run --release -- --pipeline-mt
```

### 7.2 Main 集成

```rust
if args.gateway {
    // 启动 HTTP Server 线程
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(run_http_server(queues));
    });
    // 运行 Trading Core
    run_pipeline_multi_thread(queues, ...);
}
```

---

## 总结

本章实现了 Gateway 作为客户端接入层。

**核心理念**：
> Gateway 是**速度门卫**而不是**业务处理器**。快速接收、快速校验、快速转发。

