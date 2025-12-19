<div align="center">

# ⚔️ 0xInfinity
### The Infinity Engine for High-Frequency Trading

> **"Perfectly balanced, as all things should be."**

[![Build Status](https://img.shields.io/badge/build-passing-brightgreen)]()
[![License](https://img.shields.io/badge/license-MIT-blue)]()
[![Rust](https://img.shields.io/badge/language-Rust-orange)]()
[![mdBook](https://img.shields.io/badge/docs-mdBook-blue)](https://gjwang.github.io/zero_x_infinity/)

</div>

---

## 🚀 The Journey

这是一个从 0 到 1 的硬核交易引擎 in Rust 的教程。
This is a pilgrimage from `Hello World` to `Microsecond Latency`.

**📖 [Read the Book Online →](https://gjwang.github.io/zero_x_infinity/)**

### Chapters

| Stage | Title | Description |
|-------|-------|-------------|
| 0x01 | [Genesis](./docs/src/0x01-genesis.md) | 基础订单簿引擎 |
| 0x02 | [The Curse of Float](./docs/src/0x02-the-curse-of-float.md) | 浮点数的诅咒 → u64 重构 |
| 0x03 | [Decimal World](./docs/src/0x03-decimal-world.md) | 十进制转换与精度配置 |
| 0x04 | [BTree OrderBook](./docs/src/0x04-btree-orderbook.md) | BTreeMap 数据结构重构 |
| 0x05 | [User Balance](./docs/src/0x05-user-balance.md) | 用户账户与余额管理 |
| 0x06 | [Enforced Balance](./docs/src/0x06-enforced-balance.md) | 类型安全的强制余额 |
| 0x07-a | [Testing Framework](./docs/src/0x07-a-testing-framework.md) | 100万订单批量测试框架 |
| 0x07-b | [Performance Baseline](./docs/src/0x07-b-perf-baseline.md) | 性能基线与瓶颈分析 |
| 0x08-a | [Trading Pipeline Design](./docs/src/0x08-a-trading-pipeline-design.md) | 交易流水线设计 |
| 0x08-b | [UBSCore Implementation](./docs/src/0x08-b-ubscore-implementation.md) | UBSCore 实现 |
| 0x08-c | [Complete Event Flow](./docs/src/0x08-c-ring-buffer-pipeline.md) | 完整事件流 |
| 0x08-d | [Complete Order Lifecycle](./docs/src/0x08-d-complete-order-lifecycle.md) | 完整订单生命周期 |
| 0x08-e | [Cancel Optimization](./docs/src/0x08-e-cancel-optimization.md) | 撤单性能优化：Order Index |
| 0x08-f | [Ring Buffer Pipeline](./docs/src/0x08-f-ring-buffer-pipeline.md) | Ring Buffer Pipeline 性能分析 |
| 0x08-g | [Multi-Thread Pipeline](./docs/src/0x08-g-multi-thread-pipeline.md) | 多线程 Pipeline |
| 0x08-h | [Performance Monitoring](./docs/src/0x08-h-performance-monitoring.md) | 性能监控与意图编码 |
| 0x09-a | [Gateway: Client Access Layer](./docs/src/0x09-a-gateway.md) | HTTP Gateway 客户端接入层 |
| 0x09-b | [Settlement Persistence](./docs/src/0x09-b-settlement-persistence.md) | TDengine 持久化层 |
| 0x09-c | [WebSocket Push](./docs/src/0x09-c-websocket-push.md) | 实时推送 |
| 0x09-d | [K-Line Aggregation](./docs/src/0x09-d-kline-aggregation.md) | K线聚合 |

---

## 🏃 Quick Start

```bash
# Install git hooks
./scripts/install-hooks.sh

# Run Gateway mode (HTTP API + Trading Core)
cargo run --release -- --gateway --port 8080

# Run single-threaded pipeline (1.3M orders)
cargo run --release -- --pipeline --input fixtures/test_with_cancel_highbal

# Run multi-threaded pipeline
cargo run --release -- --pipeline-mt --input fixtures/test_with_cancel_highbal

# Compare both pipelines (correctness test)
./scripts/test_pipeline_compare.sh highbal

# Run unit tests
cargo test

# Test Gateway API
./scripts/test_gateway_simple.sh
```

---

## 💾 Settlement Persistence (TDengine)

### Start TDengine

```bash
docker run -d --name tdengine -p 6030:6030 -p 6041:6041 tdengine/tdengine:latest
```

### Enable Persistence

Edit `config/dev.yaml`:

```yaml
persistence:
  enabled: true
  tdengine_dsn: "taos+ws://root:taosdata@localhost:6041"
```

### Run with Persistence

```bash
cargo run --release -- --gateway --env dev
```

### Query Data

```bash
# Connect to TDengine
docker exec -it tdengine taos

# Query orders
USE trading;
SELECT * FROM orders LIMIT 10;

# Query trades
SELECT * FROM trades LIMIT 10;

# Query balances
SELECT * FROM balances LIMIT 10;
```

### API Endpoints

- `POST /api/v1/create_order` - Create order ✅
- `POST /api/v1/cancel_order` - Cancel order ✅
- `GET /api/v1/order/:order_id` - Query order ✅
- `GET /api/v1/orders?user_id=&limit=` - Query orders list ✅
- `GET /api/v1/trades?limit=` - Query trades ✅
- `GET /api/v1/balances?user_id=&asset_id=` - Query balances ✅
- `GET /api/v1/klines?interval=&limit=` - Query K-Line ✅
- `WS /ws?user_id=` - WebSocket real-time push ✅

---


[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
