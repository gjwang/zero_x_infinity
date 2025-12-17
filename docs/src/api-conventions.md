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

## 2. 参考：Binance API 规范

### 2.1 Order Status

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

### 2.2 Order Type

| Type | 说明 |
|------|------|
| `LIMIT` | 限价单 |
| `MARKET` | 市价单 |
| `STOP_LOSS` | 止损单 |
| `STOP_LOSS_LIMIT` | 限价止损单 |
| `TAKE_PROFIT` | 止盈单 |
| `TAKE_PROFIT_LIMIT` | 限价止盈单 |
| `LIMIT_MAKER` | 只做 Maker 单 |

### 2.3 Time In Force

| TIF | 说明 |
|-----|------|
| `GTC` | Good Till Cancel - 一直有效直到取消 |
| `IOC` | Immediate Or Cancel - 立即成交剩余取消 |
| `FOK` | Fill Or Kill - 全部成交或取消 |
| `GTX` | Good Till Crossing - 只做 Maker |
| `GTD` | Good Till Date - 有效期至指定时间 |

---

## 3. JSON 序列化

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

## 4. 变更历史

| 版本 | 日期 | 变更 |
|------|------|------|
| 0.8d | 2025-12-17 | 初始规范：OrderStatus 改为 SCREAMING_CASE |
