<div align="center">

# ⚔️ 0xInfinity
### The Hardest Core HFT Tutorial in Rust

> **"From Hello World to Microsecond Latency."**

[![Build Status](https://img.shields.io/badge/build-passing-brightgreen)]()
[![License](https://img.shields.io/badge/license-MIT-blue)]()
[![Rust](https://img.shields.io/badge/language-Rust-orange)]()
[![mdBook](https://img.shields.io/badge/docs-mdBook-blue)](https://gjwang.github.io/zero_x_infinity/)

</div>

---

## ⚡ Why 0xInfinity?

**This is not another "Toy Matching Engine" tutorial.**

We are building a **production-grade** crypto trading engine that handles **1.3M orders/sec** (P99 < 200µs) on a single core. This project documents the entire journey from a naive `Vec<Order>` implementation to a professional LMAX Disruptor-style Ring Buffer architecture.

### 🔥 Hardcore Tech Stack
*   **Zero GC**: Pure Rust implementation with zero garbage collection pauses.
*   **Lock-free**: High-performance Ring Buffer (`crossbeam-queue`) for inter-thread communication.
*   **Determinism**: Event Sourcing architecture ensures 100% reproduceability.
*   **Safety**: Ed25519 Authentication & Type-safe Asset handling.
*   **Persistence**: TDengine (Time-Series Database) for high-speed audit logging.

## 🏗️ Architecture

```mermaid
graph TD
    Client[Client] -->|HTTP/WS| Gateway
    Gateway -->|RingBuffer| Ingestion
    subgraph "Trading Core (Single Thread)"
        Ingestion -->|SeqOrder| UBSCore[UBSCore (Risk/Balance)]
        UBSCore -->|LockedOrder| ME[Matching Engine]
        ME -->|Trade/OrderUpdate| Settlement
    end
    Settlement -->|Async| Persistence[TDengine]
    Settlement -->|Async| MktData[Market Data (K-Line)]
    Settlement -->|Async| WS[WebSocket Push]
```

## ✨ Core Features

*   **Order Management**: Limit, Market, Cancel, Maker/Taker logic.
*   **Risk Control**: Pre-trade balance check, exact fund locking.
*   **Market Data**: Real-time Depth (Orderbook), K-Line (followers Binance format), Ticker.
*   **Interfaces**: REST API, WebSocket Stream (Pub/Sub).
*   **Replay**: Full determinism allows replaying from genesis for exactly-once state recovery.

---

## 🚀 The Journey

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
| 0x09-e | [Order Book Depth](./docs/src/0x09-e-orderbook-depth.md) | 盘口深度 |
| 0x09-f | [Full Integration Test](./docs/src/0x09-f-integration-test.md) | 全功能集成与回归验收 |
| **Part II** | **产品化阶段 (Productization)** | |
| 0x0A | [Part II Introduction](./docs/src/0x0A-part-ii-introduction.md) | 产品化路线图 |
| 0x0A-a | [Account System](./docs/src/0x0A-a-account-system.md) | PostgreSQL 账户管理 |
| 0x0A-b | [API Auth](./docs/src/0x0A-b-api-auth.md) | 安全鉴权 (进行中) |

---

## 🏃 快速开始

```bash
# Install git hooks
./scripts/install-hooks.sh

# Run Gateway mode (HTTP API + Trading Core)
cargo run --release -- --gateway --port 8080

# Run single-threaded pipeline (1.3M orders)
cargo run --release -- --pipeline --input fixtures/test_with_cancel_highbal

# Run multi-threaded pipeline
cargo run --release -- --pipeline-mt --input fixtures/test_with_cancel_highbal

# Compare both pipelines (ST vs MT)
./scripts/test_pipeline_compare.sh highbal

# Regression check (vs Golden Baseline)
./scripts/test_pipeline_compare.sh 100k

# Generate new baseline (requires --force)
./scripts/generate_baseline.sh 100k -f
```

---

## 📑 回归测试与基线 (Regression)

本项目采用 **Golden Set** 基线比对策略。基线数据存储在 `baseline/` 目录下，代表了系统 100% 正确的状态。

- **100% 资产一致性**：多线程模式必须在 `avail` 和 `frozen` 金额上与单线程基准完全对齐。
- **DB 持久化优先**：多线程模式已移除本地 CSV 流水，全面采用 **TDengine** 进行审计。
- **基线保护**：禁止随意修改基线，更新必须通过 `generate_baseline.sh --force` 并在确认逻辑正确后提交。

---

## 💾 结算持久化 (TDengine)

### 启动 TDengine

```bash
docker run -d --name tdengine -p 6030:6030 -p 6041:6041 tdengine/tdengine:latest
```

### 启用持久化

Edit `config/dev.yaml`:

```yaml
persistence:
  enabled: true
  tdengine_dsn: "taos+ws://root:taosdata@localhost:6041"
```

### 启动持久化模式

```bash
cargo run --release -- --gateway --env dev
```

### 查询数据

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

### API 端点

- `POST /api/v1/create_order` - 创建订单 ✅
- `POST /api/v1/cancel_order` - 取消订单 ✅
- `GET /api/v1/order/:order_id` - 查询订单 ✅
- `GET /api/v1/orders?user_id=&limit=` - 查询订单列表 ✅
- `GET /api/v1/trades?limit=` - 查询成交记录 ✅
- `GET /api/v1/balances?user_id=&asset_id=` - 查询余额 ✅
- `GET /api/v1/klines?interval=&limit=` - 查询 K 线 ✅
- `GET /api/v1/depth?symbol=&limit=` - 查询盘口深度 ✅
- `WS /ws?user_id=` - WebSocket 实时推送 ✅

---


[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
