<div align="center">

# ⚔️ 0xInfinity

<h3>
  <a href="#-english">🇺🇸 English</a>
  &nbsp;&nbsp;&nbsp;|&nbsp;&nbsp;&nbsp;
  <a href="#-chinese">🇨🇳 中文</a>
</h3>

</div>

<div id="-english"></div>

# 🇺🇸 English

> **"From Hello World to Microsecond Latency."**

[![Build Status](https://img.shields.io/badge/build-passing-brightgreen)]()
[![License](https://img.shields.io/badge/license-MIT-blue)]()
[![Rust](https://img.shields.io/badge/language-Rust-orange)]()
[![mdBook](https://img.shields.io/badge/docs-mdBook-blue)](https://gjwang.github.io/zero_x_infinity/)

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
*   **Crash Recovery**: Universal WAL & Snapshot for bit-perfect state reconstruction.

## 🏗️ Architecture

```mermaid
graph TD
    Client[Client] -->|HTTP/WS| Gateway
    Gateway -->|RingBuffer| Ingestion
    subgraph "Trading Core (Single Thread)"
        Ingestion -->|SeqOrder| UBSCore["UBSCore (Risk/Balance)"]
        UBSCore -->|LockedOrder| ME["Matching Engine"]
        ME -->|Trade/OrderUpdate| Settlement
    end
    Settlement -->|Async| Persistence[TDengine]
    Settlement -->|Async| MktData["Market Data (K-Line)"]
    Settlement -->|Async| WS["WebSocket Push"]
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

**📊 [Project Roadmap →](./docs/src/0x00-mvp-roadmap.md)**

### Chapters

| Stage | Title | Description |
|-------|-------|-------------|
| 0x01 | [Genesis](./docs/src/0x01-genesis.md) | Basic OrderBook Engine |
| 0x11 | [Deposit & Withdraw](./docs/src/0x11-deposit-withdraw.md) | ✅ Mock Chain Integration (RC2 Released) |
| 0x02 | [The Curse of Float](./docs/src/0x02-the-curse-of-float.md) | Float Curse → u64 Refactor |
| 0x03 | [Decimal World](./docs/src/0x03-decimal-world.md) | Decimal System & Precision |
| 0x04 | [BTree OrderBook](./docs/src/0x04-btree-orderbook.md) | BTreeMap Structure Refactor |
| 0x05 | [User Balance](./docs/src/0x05-user-balance.md) | Account & Balance Management |
| 0x06 | [Enforced Balance](./docs/src/0x06-enforced-balance.md) | Type-Safe Enforced Balance |
| 0x07-a | [Testing Framework](./docs/src/0x07-a-testing-framework.md) | 1M Orders Batch Testing |
| 0x07-b | [Performance Baseline](./docs/src/0x07-b-perf-baseline.md) | Baseline & Bottleneck Analysis |
| 0x08-a | [Trading Pipeline Design](./docs/src/0x08-a-trading-pipeline-design.md) | Pipeline Architecture |
| 0x08-b | [UBSCore Implementation](./docs/src/0x08-b-ubscore-implementation.md) | UBSCore Implementation |
| 0x08-c | [Complete Event Flow](./docs/src/0x08-c-ring-buffer-pipeline.md) | Complete Event Flow |
| 0x08-d | [Complete Order Lifecycle](./docs/src/0x08-d-complete-order-lifecycle.md) | Complete Order Lifecycle |
| 0x08-e | [Cancel Optimization](./docs/src/0x08-e-cancel-optimization.md) | Cancel Optimization: Order Index |
| 0x08-f | [Ring Buffer Pipeline](./docs/src/0x08-f-ring-buffer-pipeline.md) | Ring Buffer Performance |
| 0x08-g | [Multi-Thread Pipeline](./docs/src/0x08-g-multi-thread-pipeline.md) | Multi-Thread Pipeline |
| 0x08-h | [Performance Monitoring](./docs/src/0x08-h-performance-monitoring.md) | Monitoring & Intent-Encoded |
| 0x09-a | [Gateway: Client Access Layer](./docs/src/0x09-a-gateway.md) | HTTP Gateway Access Layer |
| 0x09-b | [Settlement Persistence](./docs/src/0x09-b-settlement-persistence.md) | TDengine Persistence |
| 0x09-c | [WebSocket Push](./docs/src/0x09-c-websocket-push.md) | Real-time Push |
| 0x09-d | [K-Line Aggregation](./docs/src/0x09-d-kline-aggregation.md) | K-Line Aggregation |
| 0x09-e | [Order Book Depth](./docs/src/0x09-e-orderbook-depth.md) | OrderBook Depth |
| 0x09-f | [Full Integration Test](./docs/src/0x09-f-integration-test.md) | E2E Integration & Regression |
| **Part II** | **Productization** | |
| 0x0A | [Part II Introduction](./docs/src/0x0A-part-ii-introduction.md) | Productization Roadmap |
| 0x0A-a | [Account System](./docs/src/0x0A-a-account-system.md) | PostgreSQL Account Manager |
| 0x0A-b | [API Auth](./docs/src/0x0A-c-api-auth.md) | Ed25519 Authentication |
| 0x0B | [Funding & Transfer](./docs/src/0x0B-funding.md) | Funding System Architecture |
| 0x0B-a | [Internal Transfer](./docs/src/0x0B-a-transfer.md) | FSM-based 2PC Transfer |
| 0x0C | [Trade Fee](./docs/src/0x0C-trade-fee.md) | Maker/Taker Fee System |
| **Part III** | **Resilience** | |
| 0x0D | [Snapshot & Recovery](./docs/src/0x0D-snapshot-recovery.md) | Universal WAL & Snapshot Persistence |
| 0x0E | [OpenAPI Integration](./docs/src/0x0E-openapi-integration.md) | Swagger UI & SDK Generation |
| 0x0F | [Admin Dashboard](./docs/src/0x0F-admin-dashboard.md) | FastAPI Amis Admin + Trace ID |
| 0x10 | [Web Frontend](./docs/src/0x10-web-frontend.md) | Frontend Outsourcing Requirement |
| 0x11 | [Deposit & Withdraw](./docs/src/0x11-deposit-withdraw.md) | ✅ Mock Chain Integration |
| 0x11-a | [Real Chain Integration](./docs/src/0x11-a-real-chain.md) | ✅ BTC Sentinel MVP |
| 0x11-a | [Real Chain Integration](./docs/src/0x11-a-real-chain.md) | ✅ BTC Sentinel MVP |
| 0x11-b | [Sentinel Hardening](./docs/src/0x11-b-sentinel-hardening.md) | ✅ DEF-002 Fix, ADR-005/006 |
| **Phase V** | **Extreme Optimization** | **"Metal Mode"** |
| 0x14 | [Zero-Copy](./docs/src/0x14-zero-copy.md) | 🚧 Zero-Copy Deserialization (rkyv) |
| 0x15 | [CPU Affinity](./docs/src/0x15-cpu-affinity.md) | Core Pinning & Cache Isolation |


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

# Compare both pipelines (ST vs MT)
./scripts/test_pipeline_compare.sh highbal

# Regression check (vs Golden Baseline)
./scripts/test_pipeline_compare.sh 100k

# Generate new baseline (requires --force)
./scripts/generate_baseline.sh 100k -f
```

---

## 📑 Regression Testing

We use a **Golden Set** strategy. Baselines are stored in `baseline/`.

- **Consistnecy**: MT mode must match ST mode 100%.
- **Persistence**: MT mode relies on **TDengine** for audit trails.
- **Protection**: Updates to baseline require `generate_baseline.sh --force`.

---

## 💾 Persistence (TDengine)

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

### Start Persistence Mode

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
```

<br>
<div align="right"><a href="#-english">↑ Back to Top</a></div>
<br>

---

<div id="-chinese"></div>

# 🇨🇳 中文文档

> **"From Hello World to Microsecond Latency."**

---

## ⚡ 为什么选择 0xInfinity?

**这不是另一个 "玩具级撮合引擎" 教程。**

我们正在构建一个**生产级**的加密货币交易引擎，在单核上可处理 **130万订单/秒** (P99 < 200µs)。本项目记录了从最朴素的 `Vec<Order>` 实现到专业的 LMAX Disruptor 风格 Ring Buffer 架构的完整演进过程。

### 🔥 硬核技术栈
*   **零 GC (Zero GC)**: 纯 Rust 实现，无垃圾回收暂停。
*   **无锁并发 (Lock-free)**: 基于高性能 Ring Buffer (`crossbeam-queue`) 的线程间通信。
*   **确定性 (Determinism)**: 事件溯源架构，确保 100% 可重现性。
*   **安全性 (Safety)**: Ed25519 非对称鉴权 & 类型安全的资产处理。
*   **持久化 (Persistence)**: 集成 TDengine 时序数据库，实现极速审计日志。
*   **崩溃恢复 (Crash Recovery)**: 通用 WAL & 快照，实现比特级精确状态重建。

---

## 🏗️ 架构概览

```mermaid
graph TD
    Client[客户端] -->|HTTP/WS| Gateway
    Gateway -->|RingBuffer| Ingestion
    subgraph "核心交易线程 (Single Thread)"
        Ingestion -->|SeqOrder| UBSCore["UBSCore (风控/余额)"]
        UBSCore -->|LockedOrder| ME[撮合引擎]
        ME -->|Trade/OrderUpdate| Settlement
    end
    Settlement -->|异步| Persistence[TDengine]
    Settlement -->|异步| MktData["行情数据 (K-Line)"]
    Settlement -->|异步| WS[WebSocket 推送]
```

## ✨ 核心特性

*   **订单管理**: 限价单、市价单、撤单、Maker/Taker 逻辑。
*   **风控系统**: 交易前余额检查、精确资金锁定。
*   **行情数据**: 实时深度 (Orderbook)、K线 (Binance 格式)、Ticker。
*   **接口支持**: REST API、WebSocket流 (Pub/Sub)。
*   **回放机制**: 全确定性设计，允许从创世状态重放以实现精确的状态恢复。

---

## 🚀 学习之旅

**📖 [在线阅读完整教程 →](https://gjwang.github.io/zero_x_infinity/)**

**📊 [项目路线图 →](./docs/src/0x00-mvp-roadmap.md)**

### 章节索引

| 阶段 | 标题 | 描述 |
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
| 0x0A-b | [API Auth](./docs/src/0x0A-c-api-auth.md) | Ed25519 安全鉴权 |
| 0x0B | [Funding & Transfer](./docs/src/0x0B-funding.md) | 资金体系架构 |
| 0x0B-a | [Internal Transfer](./docs/src/0x0B-a-transfer.md) | FSM 内部转账 |
| 0x0C | [Trade Fee](./docs/src/0x0C-trade-fee.md) | Maker/Taker 手续费系统 |
| **Part III** | **鲁棒性阶段 (Resilience)** | |
| 0x0D | [Snapshot & Recovery](./docs/src/0x0D-snapshot-recovery.md) | 通用 WAL & 快照持久化 |
| 0x0E | [OpenAPI Integration](./docs/src/0x0E-openapi-integration.md) | Swagger UI 与 SDK 生成 |
| 0x0F | [Admin Dashboard](./docs/src/0x0F-admin-dashboard.md) | 管理后台 + Trace ID 证据链 |
| 0x10 | [Web Frontend](./docs/src/0x10-web-frontend.md) | 前端外包需求文档 |
| 0x11 | [Deposit & Withdraw](./docs/src/0x11-deposit-withdraw.md) | ✅ 充值与提现 (Mock Chain) |
| 0x11-a | [Real Chain Integration](./docs/src/0x11-a-real-chain.md) | 🚧 真实区块链集成 |

---

## 🏃 快速开始

```bash
# 安装 git hooks
./scripts/install-hooks.sh

# 运行 Gateway 模式 (HTTP API + 交易核心)
cargo run --release -- --gateway --port 8080

# 运行单线程流水线 (吞吐量基准测试)
cargo run --release -- --pipeline --input fixtures/test_with_cancel_highbal

# 运行多线程流水线
cargo run --release -- --pipeline-mt --input fixtures/test_with_cancel_highbal

# 对比测试 (单线程 vs 多线程)
./scripts/test_pipeline_compare.sh highbal

# 回归检查 (对比黄金基线)
./scripts/test_pipeline_compare.sh 100k
```

---

## 📑 回归测试与基线 (Regression)

本项目采用 **Golden Set** 基线比对策略。基线数据存储在 `baseline/` 目录下，代表了系统 100% 正确的状态。

- **100% 资产一致性**：多线程模式必须在 `avail` 和 `frozen` 金额上与单线程基准完全对齐。
- **DB 持久化优先**：多线程模式已移除本地 CSV 流水，全面采用 **TDengine** 进行审计。
- **基线保护**：禁止随意修改基线，更新必须通过 `generate_baseline.sh --force` 并在确认逻辑正确后提交。

---

## 💾 结算持久化 (TDengine)

### 1. 启动 TDengine

```bash
docker run -d --name tdengine -p 6030:6030 -p 6041:6041 tdengine/tdengine:latest
```

### 2. 启用持久化配置

编辑 `config/dev.yaml`:

```yaml
persistence:
  enabled: true
  tdengine_dsn: "taos+ws://root:taosdata@localhost:6041"
```

### 3. API 概览

**公开接口 (Public)**:
- `GET /api/v1/public/depth` - 盘口深度
- `GET /api/v1/public/klines` - K 线数据
- `GET /api/v1/public/trades` - 公开交易历史
- `GET /api/v1/public/assets` - 资产列表

**私有接口 (Private, Ed25519 签名)**:
- `POST /api/v1/private/order` - 创建订单
- `POST /api/v1/private/cancel` - 取消订单
- `POST /api/v1/private/transfer` - 内部转账
- `GET /api/v1/private/balances/all` - 查询余额
- `WS /ws` - WebSocket 实时推送

---

[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
