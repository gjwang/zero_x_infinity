# 0x09-c WebSocket Push: 实时推送

> **📦 代码变更**: [查看 Diff](https://github.com/gjwang/zero_x_infinity/compare/v0.9-b-settlement-persistence...v0.9-c-websocket-push)

> **本节核心目标**：实现 WebSocket 实时推送，客户端可接收订单状态更新、成交通知、余额变化。

---

## 背景：从轮询到推送

当前系统查询方式：

```
Client                    Gateway
  │                          │
  ├─── GET /orders ─────────>│  (轮询 polling)
  │<──────────────────────────┤
  │       ... 5秒后 ...       │
  ├─── GET /orders ─────────>│  (再次轮询)
  │<──────────────────────────┤
```

**问题**：
- ❌ 延迟高：最多 5 秒延迟
- ❌ 浪费资源：大量无效请求
- ❌ 实时性差

本章解决方案：

```
Client                    Gateway                Trading Core
  │                          │                        │
  ├── WS Connect ───────────>│                        │
  │<── Connected ────────────┤                        │
  │                          │                        │
  │                          │<── Order Filled ───────┤
  │<── push: order.update ───┤                        │
  │                          │                        │
  │                          │<── Trade ──────────────┤
  │<── push: trade ──────────┤                        │
```

---

## 1. 推送事件类型

### 1.1 事件分类

| 事件类型 | 触发时机 | 接收者 |
|----------|----------|--------|
| `order.update` | 订单状态变化 (NEW/FILLED/PARTIALLY_FILLED/CANCELED) | 订单所有者 |
| `trade` | 成交发生 | 双方用户 |
| `balance.update` | 余额变化 | 账户所有者 |


### 1.2 消息格式

```json
// 订单更新
{
    "type": "order.update",
    "data": {
        "order_id": 1001,
        "symbol": "BTC_USDT",
        "status": "FILLED",
        "filled_qty": "0.001",
        "avg_price": "85000.00",
        "updated_at": 1734533790000
    }
}

// 成交通知
{
    "type": "trade",
    "data": {
        "trade_id": 5001,
        "order_id": 1001,
        "symbol": "BTC_USDT",
        "side": "BUY",
        "price": "85000.00",
        "qty": "0.001",
        "fee": "0.00001",
        "role": "TAKER",
        "traded_at": 1734533790000
    }
}

// 余额变化
{
    "type": "balance.update",
    "data": {
        "asset": "BTC",
        "avail": "1.501000",
        "frozen": "0.000000",
        "change": "+0.001000",
        "reason": "trade_settled"
    }
}
```

---

## 2. 架构设计

### 2.1 系统架构

```
┌─────────────────────────────────────────────────────────────────────────┐
│                              Gateway                                     │
│                                                                          │
│  ┌───────────────┐     ┌───────────────┐     ┌───────────────┐          │
│  │  HTTP Server  │     │  WS Server    │     │  Push Service │          │
│  │  (create_order)│    │  (connections)│     │  (broadcast)  │          │
│  └───────┬───────┘     └───────┬───────┘     └───────┬───────┘          │
│          │                     │                     │                   │
│          └─────────────────────┼─────────────────────┘                   │
│                                │                                         │
│                    ┌───────────▼───────────┐                             │
│                    │   Connection Manager  │                             │
│                    │  user_id → Vec<Tx>    │                             │
│                    └───────────────────────┘                             │
└─────────────────────────────────────────────────────────────────────────┘
                                 │
                    ┌────────────▼────────────┐
                    │      Trading Core       │
                    │                         │
                    │  Settlement ──> Events  │
                    └─────────────────────────┘
```

### 2.2 连接管理

```rust
// src/websocket/manager.rs
pub struct ConnectionManager {
    // user_id -> list of sender channels
    connections: DashMap<u64, Vec<mpsc::UnboundedSender<Message>>>,
}

impl ConnectionManager {
    pub fn add_connection(&self, user_id: u64, tx: mpsc::UnboundedSender<Message>);
    pub fn remove_connection(&self, user_id: u64, tx: &mpsc::UnboundedSender<Message>);
    pub fn send_to_user(&self, user_id: u64, message: &str);
    pub fn broadcast(&self, message: &str);
}
```

### 2.3 事件传播

```
Settlement Thread                Push Service
      │                               │
      │── OrderFilled(order_id) ─────>│
      │                               │
      │                               ├── lookup user_id
      │                               │
      │                               ├── format message
      │                               │
      │                               └── send to user's connections
```

---

## 3. API 设计

### 3.1 WebSocket 端点

| 端点 | 描述 |
|------|------|
| `ws://host:port/ws` | WebSocket 连接入口 |

### 3.2 连接流程

```
1. Client: ws://localhost:8080/ws
2. Server: Upgrade to WebSocket
3. Client: {"type": "auth", "token": "user_token_or_api_key"}
4. Server: {"type": "auth.success", "user_id": 1001}
5. Server: (push events as they occur)
```

### 3.3 心跳

```json
// Client -> Server (每30秒)
{"type": "ping"}

// Server -> Client
{"type": "pong"}
```

---

## 4. 模块结构

```
src/
├── websocket/
│   ├── mod.rs              # 模块入口
│   ├── server.rs           # WebSocket 服务器
│   ├── handler.rs          # 连接处理
│   ├── manager.rs          # 连接管理器
│   ├── messages.rs         # 消息类型定义
│   └── push.rs             # 推送服务
├── gateway/
│   └── mod.rs              # 添加 WS 路由
└── ...
```

---

## 5. 依赖

```toml
# Cargo.toml
[dependencies]
tokio-tungstenite = "0.21"   # WebSocket 实现
dashmap = "5.5"               # 并发 HashMap
```

---

## 6. 实现计划

### Phase 1: 基础连接
- [ ] WebSocket 服务器启动
- [ ] 连接管理器 (ConnectionManager)
- [ ] 认证流程
- [ ] 心跳处理

### Phase 2: 事件推送
- [ ] 订单状态推送
- [ ] 成交推送
- [ ] 余额更新推送

### Phase 3: Settlement 集成
- [ ] 从 Settlement 线程接收事件
- [ ] 转换为推送消息
- [ ] 发送到对应用户

---

## 7. 验证计划

### 7.1 单元测试

```rust
#[tokio::test]
async fn test_connection_manager() {
    let manager = ConnectionManager::new();
    let (tx, rx) = mpsc::unbounded_channel();
    
    manager.add_connection(1001, tx);
    manager.send_to_user(1001, r#"{"type":"test"}"#);
    
    // Verify message received
}
```

### 7.2 集成测试

```bash
# 1. 启动 Gateway
cargo run --release -- --gateway --port 8080

# 2. 连接 WebSocket
websocat ws://localhost:8080/ws

# 3. 发送认证
{"type": "auth", "token": "test_user_1001"}

# 4. 在另一个终端提交订单
curl -X POST http://localhost:8080/api/v1/create_order ...

# 5. 观察 WebSocket 收到推送
```

---

## Summary

本章实现 WebSocket 实时推送：

| 设计点 | 方案 |
|--------|------|
| WebSocket 库 | tokio-tungstenite |
| 连接管理 | DashMap (并发安全) |
| 消息格式 | JSON |
| 认证 | Token/API Key |
| 心跳 | 30秒 ping/pong |

**核心理念**：

> 推送是**实时通道**：Settlement 完成后立即推送到客户端，延迟 < 10ms。

下一章 (0x09-d) 将实现 K-Line 聚合服务。
