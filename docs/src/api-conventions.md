# API 规范 (API Conventions)

> 本文档定义 0xInfinity 交易系统的 API 规范，确保内外部接口一致性。

---

## 1. 命名规范

### 1.1 枚举值使用 SCREAMING_CASE

所有对外暴露的枚举类型使用 **SCREAMING_CASE**（全大写下划线分隔）：

```rust
// ✅ 正确：SCREAMING_CASE
pub enum OrderStatus {
    NEW,
    PARTIALLY_FILLED,
    FILLED,
    CANCELED,
    REJECTED,
    EXPIRED,
}

// ❌ 错误：PascalCase
pub enum OrderStatus {
    New,
    PartiallyFilled,
    Filled,
}
```

**原因**：
- 与 Binance/FTX/OKX 等主流交易所 API 保持一致
- JSON/REST API 输出时直接可读
- 避免序列化时的大小写转换问题

### 1.2 适用范围

以下类型必须使用 SCREAMING_CASE：

| 类型 | 示例值 |
|------|--------|
| `OrderStatus` | `NEW`, `FILLED`, `CANCELED` |
| `OrderType` | `LIMIT`, `MARKET`, `STOP_LIMIT` |
| `Side` | `BUY`, `SELL` |
| `TimeInForce` | `GTC`, `IOC`, `FOK`, `GTX` |
| `RejectReason` | `INSUFFICIENT_BALANCE`, `INVALID_PRICE` |

### 1.3 Rust 编译器警告处理

SCREAMING_CASE 会触发 Rust 的 `non_camel_case_types` 警告，需要显式允许：

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(clippy::upper_case_acronyms)]
#[allow(non_camel_case_types)]
pub enum OrderStatus {
    NEW,
    PARTIALLY_FILLED,
    // ...
}
```

---

## 2. 参数命名一致性

### 2.1 核心原则

**保持前后一致，减少转换的认知代价**

所有 API（HTTP、内部消息）应使用统一的字段名，避免同一概念有多个名称。
除非有特殊设计的, 例如WebSocket的推送消息格式,会专门设计一种紧凑的方式,减少推送消息的大小。

### 2.2 标准字段名

| 概念 | 标准名称 | ❌ 避免使用 | 说明 |
|------|----------|------------|------|
| 数量 | `qty` | `quantity`, `amount`, `size` | 与 `InternalOrder.qty` 一致 |
| 价格 | `price` | `px`, `prc` | 清晰明确 |
| 订单ID | `order_id` | `orderId`, `oid` | snake_case |
| 用户ID | `user_id` | `userId`, `uid` | snake_case |
| 交易ID | `trade_id` | `tradeId`, `tid` | snake_case |
| 客户端订单ID | `cid` | `client_order_id`, `clOrdId` | 简短但明确 |
| 交易对 | `symbol` | `pair`, `market` | 行业标准 |

### 2.3 命名风格

- **JSON API**: 使用 `snake_case` (与 Rust 字段名一致)
  ```json
  {
    "order_id": 1001,
    "user_id": 1001,
    "qty": "0.001"
  }
  ```

- **内部 Rust 结构**: 使用 `snake_case`
  ```rust
  pub struct InternalOrder {
      pub order_id: u64,
      pub user_id: u64,
      pub qty: u64,
  }
  ```

### 2.4 缩写规则

**何时使用缩写**:
- ✅ 行业通用缩写: `qty` (quantity), `cid` (client_order_id)
- ✅ 高频使用字段: `qty` 比 `quantity` 更简洁
- ❌ 避免自创缩写: 不要用 `ord` 代替 `order`

---

## 3. 参考：Binance API 规范


### 3.1 Order Status

| Status | 说明 |
|--------|------|
| `NEW` | 订单被接受，等待成交 |
| `PARTIALLY_FILLED` | 部分成交 |
| `FILLED` | 完全成交 |
| `CANCELED` | 用户取消（注意：单 L） |
| `PENDING_CANCEL` | 取消中（未使用）|
| `REJECTED` | 订单被拒绝 |
| `EXPIRED` | 订单过期（GTD/IOC/FOK）|
| `EXPIRED_IN_MATCH` | STP 导致过期 |

### 3.2 Order Type

| Type | 说明 |
|------|------|
| `LIMIT` | 限价单 |
| `MARKET` | 市价单 |
| `STOP_LOSS` | 止损单 |
| `STOP_LOSS_LIMIT` | 限价止损单 |
| `TAKE_PROFIT` | 止盈单 |
| `TAKE_PROFIT_LIMIT` | 限价止盈单 |
| `LIMIT_MAKER` | 只做 Maker 单 |

### 3.3 Time In Force

| TIF | 说明 |
|-----|------|
| `GTC` | Good Till Cancel - 一直有效直到取消 |
| `IOC` | Immediate Or Cancel - 立即成交剩余取消 |
| `FOK` | Fill Or Kill - 全部成交或取消 |
| `GTX` | Good Till Crossing - 只做 Maker |
| `GTD` | Good Till Date - 有效期至指定时间 |

---

## 4. JSON 序列化

建议使用 `serde` 的 `rename_all` 来自动处理：

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OrderStatus {
    New,              // 序列化为 "NEW"
    PartiallyFilled,  // 序列化为 "PARTIALLY_FILLED"
    Filled,           // 序列化为 "FILLED"
}
```

**注意**：本项目选择直接在代码中使用 SCREAMING_CASE，而不是依赖 serde 转换，以保持代码和输出的一致性。

---

## 5. 变更历史

| 版本 | 日期 | 变更 |
|------|------|------|
| 0.8d | 2025-12-17 | 初始规范：OrderStatus 改为 SCREAMING_CASE |
| 0.9a | 2025-12-19 | 新增参数命名一致性规范 (Section 2) |
