# 0x09-c WebSocket Push: Real-time Notification

<h3>
  <a href="#-english">🇺🇸 English</a>
  &nbsp;&nbsp;&nbsp;|&nbsp;&nbsp;&nbsp;
  <a href="#-chinese">🇨🇳 中文</a>
</h3>

<div id="-english"></div>

## 🇺🇸 English

> **📦 Code Changes**: [View Diff](https://github.com/gjwang/zero_x_infinity/compare/v0.9-b-settlement-persistence...v0.9-c-websocket-push)

> **Core Objective**: Implement WebSocket real-time push so clients can receive order updates, trade notifications, and balance changes.

---

## Background: From Polling to Push

Current Query Method (Polling):

```
Client                    Gateway
  │                          │
  ├─── GET /orders ─────────>│  (Poll)
  │<──────────────────────────┤
  │       ... seconds ...      │
  ├─── GET /orders ─────────>│  (Poll again)
  │<──────────────────────────┤
```

**Issues**:
*   ❌ High Latency
*   ❌ Wasted Resources
*   ❌ Poor Real-time experience

This Chapter's Solution (Push):

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

## 1. Push Event Types

### 1.1 Classification

| Event Type | Trigger | Recipient |
|------------|---------|-----------|
| `order.update` | Status change (NEW/FILLED/CANCELED) | Order Owner |
| `trade` | Trade execution | Buyer & Seller |
| `balance.update` | Balance change | Account Owner |

### 1.2 Message Format

```json
// Order Update
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

// Trade Notification
{
    "type": "trade",
    "data": {
        "trade_id": 5001,
        "order_id": 1001,
        "symbol": "BTC_USDT",
        "side": "BUY",
        "role": "TAKER",
        "traded_at": 1734533790000
    }
}

// Balance Update
{
    "type": "balance.update",
    "data": {
        "asset": "BTC",
        "avail": "1.501000",
        "frozen": "0.000000"
    }
}
```

---

## 2. Architecture Design

### 2.1 Design Principles

> [!IMPORTANT]
> **Data Consistency First**: When a user receives a push, the database MUST already be updated.

**Correct Flow**:
ME Match → Settlement Persist → Push → User Query → Data Exists ✅

**Incorrect Flow**:
ME Match → Push → User Query → Data Not Found ❌

### 2.2 System Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                    Multi-Thread Pipeline                         │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  Thread 3: ME         ──▶  trade_queue  ──▶  Thread 4: Settlement│
│                       └──▶  balance_update_queue                │
│                                                                  │
│  Thread 4: Settlement ──▶  push_event_queue  ──▶  WsService     │
│                       │                                          │
│                       └──▶  TDengine (persist)                   │
│                                                                  │
│  WsService (Gateway)  ──▶  ConnectionManager  ──▶  Clients      │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

**Key Decisions**:
*   ✅ **Settlement is the only push source**.
*   ✅ Push events generated ONLY after persistence success.
*   ✅ WsService runs in the Gateway's tokio runtime.

### 2.3 Connection Management

`ConnectionManager` uses `DashMap` to handle concurrent connections, supporting multiple connections per user.

---

## 3. API Design

### 3.1 Endpoint

`ws://host:port/ws`

### 3.2 Connection Flow

1.  Connect.
2.  Send Auth: `{"type": "auth", "token": "..."}`.
3.  Receive Auth Success.
4.  Receive Push Events.

### 3.3 Heartbeat

Client sends `{"type": "ping"}` every 30s, Server responds `{"type": "pong"}`.

---

## 4. Implementation

### 4.1 Core Structures

**PushEvent (Internal Queue)**:

```rust
pub enum PushEvent {
    OrderUpdate { ... },
    Trade { ... },
    BalanceUpdate { ... },
}
```

**TradeEvent Extension**:
Added `taker_filled_qty`, `maker_filled_qty` etc., to `TradeEvent` to allow Settlement to determine order status (FILLED vs PARTIAL) without querying generic order state.

### 4.2 Implementation Plan

*   [x] **Phase 1: Basic Connection** (Manager, Handler, Gateway Integration).
*   [x] **Phase 2: Push Integration** (`push_event_queue`, `WsService`, Settlement logic).
*   [x] **Phase 3: Refinement** (Error handling, Performance tests).

---

## 5. Verification

### 5.1 Automated Tests

Run `sh run_test.sh`:
*   Validates WS connection.
*   Submits orders and verifies receiving `order_update`, `trade`, and `balance_update` events.

### 5.2 Manual Test

```bash
websocat "ws://localhost:8080/ws?user_id=1001"
# Send {"type": "ping"} -> Receive {"type": "pong"}
```

---

## Summary

This chapter implements WebSocket real-time push.

**Key Design Decisions**:
1.  **Settlement-first**: Ensuring consistency.
2.  **Single Source**: All events originate from Settlement.
3.  **Extended TradeEvent**: Carrying adequate state for downstream consumers.

Next Chapter: **0x09-d K-Line Aggregation**.

<br>
<div align="right"><a href="#-english">↑ Back to Top</a></div>
<br>

---

<div id="-chinese"></div>

## 🇨🇳 中文

> **📦 代码变更**: [查看 Diff](https://github.com/gjwang/zero_x_infinity/compare/v0.9-b-settlement-persistence...v0.9-c-websocket-push)

> **本节核心目标**：实现 WebSocket 实时推送，客户端可接收订单状态更新、成交通知、余额变化。

---

## 背景：从轮询到推送

当前系统查询方式（轮询）：

```
Client                    Gateway
  │                          │
  ├─── GET /orders ─────────>│  (轮询 polling)
  │<──────────────────────────┤
  │       ... 数秒后 ...       │
  ├─── GET /orders ─────────>│  (再次轮询)
  │<──────────────────────────┤
```

**问题**：
- ❌ 延迟高
- ❌ 浪费资源
- ❌ 实时性差

本章解决方案（推送）：

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
| `order.update` | 订单状态变化 | 订单所有者 |
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
```

---

## 2. 架构设计

### 2.1 设计原则

> [!IMPORTANT]
> **数据一致性优先**: 用户收到推送时，数据库必须已更新。

**正确流程**:
ME 成交 → Settlement 持久化 → 推送 → 用户查询 → 数据已存在 ✅

### 2.2 系统架构

```
┌─────────────────────────────────────────────────────────────────┐
│                    Multi-Thread Pipeline                         │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  Thread 3: ME         ──▶  trade_queue  ──▶  Thread 4: Settlement│
│                       └──▶  balance_update_queue                │
│                                                                  │
│  Thread 4: Settlement ──▶  push_event_queue  ──▶  WsService     │
│                       │                                          │
│                       └──▶  TDengine (persist)                   │
│                                                                  │
│  WsService (Gateway)  ──▶  ConnectionManager  ──▶  Clients      │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

**关键设计**:
- ✅ Settlement 作为**唯一推送源**
- ✅ 持久化成功后才生成 `PushEvent`
- ✅ WsService 运行在 Gateway 的 tokio runtime

---

## 3. API 设计

### 3.1 端点

`ws://host:port/ws`

### 3.2 连接流程

1.  Client 连接
2.  发送认证: `{"type": "auth", "token": "..."}`
3.  接收推送

### 3.3 心跳

Client 发送 `{"type": "ping"}` (每30秒)，Server 回复 `{"type": "pong"}`。

---

## 4. 实现细节

### 4.1 核心结构

**PushEvent (内部队列)**: 定义了三种核心事件结构。

**TradeEvent 扩展**: 新增了 `taker_filled_qty` 等字段，允许 Settlement 判断订单最终状态。

### 4.2 实现计划

*   [x] **Phase 1**: 基础连接管理
*   [x] **Phase 2**: 推送集成 (Settlement -> WsService)
*   [x] **Phase 3**: 完善与验证

---

## 5. 验证

### 5.1 自动化测试

运行 `sh run_test.sh`，覆盖连接、下单、接收各类推送的全流程。

### 5.2 手动测试

```bash
websocat "ws://localhost:8080/ws?user_id=1001"
```

---

## 总结

本章实现了 WebSocket 实时推送。

**关键设计决策**:
1.  **settlement-first**: 确保一致性。
2.  **单一推送源**: 简化架构。
3.  **TradeEvent 扩展**: 携带足够状态。

下一章 (0x09-d) 将实现 K-Line 聚合服务。
