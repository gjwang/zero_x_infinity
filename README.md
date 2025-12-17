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

---

## 🏃 Quick Start

```bash
# Run the matching engine
cargo run

# Run the tests
cargo test

# Run the float precision demo
cargo run --example the_curse_of_float
```

---

[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)