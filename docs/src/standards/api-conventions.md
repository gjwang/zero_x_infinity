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

## 3. HTTP 响应格式

### 3.1 统一响应结构

**所有 HTTP API 响应统一使用以下格式**:

```json
{
    "code": 0,          // 0 = 成功, 非0 = 错误码
    "msg": "ok",        // 消息描述 (简短)
    "data": {}          // 实际数据 (成功时) 或 null (失败时)
}
```

### 3.2 设计原则

| 字段 | 选择 | 理由 |
|------|------|------|
| `code` | ✅ 而非 `status` | 避免与 HTTP status 混淆 |
| `msg` | ✅ 而非 `message` | 简短明确，减少流量 |
| `data` | ✅ 统一容器 | 成功/失败都使用相同结构 |

### 3.3 成功响应示例

```json
// HTTP 200/202
{
    "code": 0,
    "msg": "ok",
    "data": {
        "order_id": 1001,
        "order_status": "ACCEPTED"
    }
}
```

### 3.4 错误响应示例

```json
// HTTP 400
{
    "code": 1001,
    "msg": "Invalid parameter: price must be greater than zero",
    "data": null
}
```

### 3.5 错误码设计

**简化方案** (不使用 HTTP*100):

| Code | 说明 | HTTP Status |
|------|------|-------------|
| 0 | 成功 | 200/202 |
| 1xxx | 客户端错误 (参数/业务) | 400 |
| 2xxx | 认证/授权错误 | 401/403 |
| 4xxx | 资源错误 | 404/429 |
| 5xxx | 服务器错误 | 500/503 |

**常用错误码**:

| Code | 说明 | HTTP Status |
|------|------|-------------|
| 1001 | 参数格式错误 | 400 |
| 1002 | 余额不足 | 400 |
| 2001 | 缺少认证信息 | 401 |
| 2002 | 认证失败 | 401 |
| 4001 | 订单不存在 | 404 |
| 4291 | 请求过于频繁 | 429 |
| 5001 | 服务不可用 | 503 |

---

## 4. 参考：Binance API 规范


### 3.1 Order Status

| Status | 说明 |
|--------|------|
| `NEW` | 订单被接受，等待成交 |
| `PARTIALLY_FILLED` | 部分成交 |
| `FILLED` | 完全成交 |
| `CANCELED` | 用户取消（注意：单 L） |
| `PENDING_CANCEL` | 取消中（未使用）|
| `REJECTED` | 订单被拒绝 |
| `EXPIRED` | 订单过期（未使用）|

---

## 5. 数字格式规范 ⭐ 重要

### 5.1 核心原则

**所有返回给客户端的数字必须使用字符串格式，并应用 display_decimals**

### 5.2 内部 vs 外部表示

| 层级 | 格式 | 精度 | 示例 |
|------|------|------|------|
| **内部存储** | `u64` | `decimals` (8位) | `100000000` (1.0 BTC) |
| **API 响应** | `String` | `display_decimals` (6位) | `"1.000000"` |
| **TDengine** | `BIGINT UNSIGNED` | `decimals` (8位) | `100000000` |

### 5.3 转换规则

```rust
// ❌ 错误：直接返回内部 u64
{
    "avail": 9000000000,      // 90.0 BTC (内部表示)
    "frozen": 1000000000       // 10.0 BTC (内部表示)
}

// ✅ 正确：转换为 display_decimals 的字符串
{
    "avail": "90.000000",     // 使用 asset.display_decimals = 6
    "frozen": "10.000000"
}
```

### 5.4 适用字段

所有数量和价格字段必须转换：

| 字段 | 内部类型 | API 类型 | 精度来源 |
|------|----------|----------|----------|
| `qty` | `u64` | `String` | `base_asset.display_decimals` |
| `price` | `u64` | `String` | `symbol.price_display_decimal` |
| `avail` | `u64` | `String` | `asset.display_decimals` |
| `frozen` | `u64` | `String` | `asset.display_decimals` |
| `filled_qty` | `u64` | `String` | `base_asset.display_decimals` |

### 5.5 实现示例

```rust
use rust_decimal::Decimal;

// 转换函数
fn format_amount(value: u64, decimals: u32, display_decimals: u32) -> String {
    let decimal_value = Decimal::from(value) / Decimal::from(10u64.pow(decimals));
    format!("{:.prec$}", decimal_value, prec = display_decimals as usize)
}

// 使用示例
let avail_str = format_amount(
    balance.avail,           // 9000000000 (内部)
    asset.decimals,          // 8
    asset.display_decimals   // 6
);
// 结果: "90.000000"
```

---

## 6. 资产和交易对表示规范 ⭐ 重要

### 6.1 核心原则

**API 响应中使用人类可读的名称，而非内部 ID**

### 6.2 字段映射

| 概念 | 内部字段 | API 字段 | 类型 | 示例 |
|------|----------|----------|------|------|
| 资产 | `asset_id: u32` | `asset: String` | 名称 | `"BTC"`, `"USDT"` |
| 交易对 | `symbol_id: u32` | `symbol: String` | 名称 | `"BTC_USDT"` |
| 基础资产 | `base_asset_id: u32` | `base_asset: String` | 名称 | `"BTC"` |
| 计价资产 | `quote_asset_id: u32` | `quote_asset: String` | 名称 | `"USDT"` |

### 6.3 错误示例 vs 正确示例

```json
// ❌ 错误：使用内部 ID
{
    "code": 0,
    "msg": "ok",
    "data": {
        "user_id": 1001,
        "asset_id": 1,           // ❌ 内部 ID
        "avail": 9000000000,     // ❌ 内部 u64
        "frozen": 1000000000
    }
}

// ✅ 正确：使用名称和字符串
{
    "code": 0,
    "msg": "ok",
    "data": {
        "user_id": 1001,
        "asset": "BTC",          // ✅ 资产名称
        "avail": "90.000000",    // ✅ display_decimals 字符串
        "frozen": "10.000000"
    }
}
```

### 6.4 订单响应示例

```json
{
    "code": 0,
    "msg": "ok",
    "data": {
        "order_id": 1001,
        "symbol": "BTC_USDT",     // ✅ 交易对名称
        "side": "BUY",
        "order_type": "LIMIT",
        "price": "85000.00",      // ✅ price_display_decimal
        "qty": "0.001000",        // ✅ base_asset.display_decimals
        "filled_qty": "0.000000",
        "status": "NEW"
    }
}
```

### 6.5 余额响应示例

```json
{
    "code": 0,
    "msg": "ok",
    "data": {
        "user_id": 1001,
        "asset": "BTC",           // ✅ 资产名称
        "avail": "90.000000",     // ✅ 6 位小数
        "frozen": "10.000000",
        "total": "100.000000"     // avail + frozen
    }
}
```

---

## 7. 实施检查清单

在实现任何 API 端点时，必须检查：

- [ ] 所有枚举使用 SCREAMING_CASE
- [ ] 字段名使用 snake_case
- [ ] 响应使用统一的 `{code, msg, data}` 结构
- [ ] **所有数字转换为字符串**
- [ ] **使用 display_decimals 精度**
- [ ] **资产使用名称而非 ID**
- [ ] **交易对使用名称而非 ID**
- [ ] 错误码符合分类规范

---

## 8. 参考资料

- [0x03 Decimal World](./0x03-decimal-world.md) - 精度设计详解
- [0x07-a Testing Framework](./0x07-a-testing-framework.md) - decimals vs display_decimals
- [Binance API Documentation](https://binance-docs.github.io/apidocs/spot/en/)

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

## 5. JSON 序列化

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

## 6. 变更历史

| 版本 | 日期 | 变更 |
|------|------|------|
| 0.8d | 2025-12-17 | 初始规范：OrderStatus 改为 SCREAMING_CASE |
| 0.9a | 2025-12-19 | 新增参数命名一致性 (Section 2) + HTTP 响应格式 (Section 3) |
