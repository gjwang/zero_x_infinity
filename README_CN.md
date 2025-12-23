<div align="center">

# ⚔️ 0xInfinity
### 从零打造微秒级高频交易引擎 (实战教程)

> **"From Hello World to Microsecond Latency."**

[![Build Status](https://img.shields.io/badge/build-passing-brightgreen)]()
[![License](https://img.shields.io/badge/license-MIT-blue)]()
[![Rust](https://img.shields.io/badge/language-Rust-orange)]()
[![mdBook](https://img.shields.io/badge/docs-mdBook-blue)](https://gjwang.github.io/zero_x_infinity/)

[🇺🇸 English](README.md)

</div>

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

---

## 🏗️ 架构概览

```mermaid
graph TD
    Client[客户端] -->|HTTP/WS| Gateway
    Gateway -->|RingBuffer| Ingestion
    subgraph "核心交易线程 (Single Thread)"
        Ingestion -->|SeqOrder| UBSCore[UBSCore (风控/余额)]
        UBSCore -->|LockedOrder| ME[撮合引擎]
        ME -->|Trade/OrderUpdate| Settlement
    end
    Settlement -->|异步| Persistence[TDengine]
    Settlement -->|异步| MktData[行情数据 (K-Line)]
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
| 0x0A-b | [API Auth](./docs/src/0x0A-b-api-auth.md) | 安全鉴权 (进行中) |

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

- `POST /api/v1/create_order` - 创建订单
- `POST /api/v1/cancel_order` - 取消订单
- `GET /api/v1/order/:order_id` - 查询订单
- `GET /api/v1/klines?interval=&limit=` - 查询 K 线
- `GET /api/v1/depth?symbol=&limit=` - 查询盘口深度
- `WS /ws?user_id=` - WebSocket 实时推送

---

[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
