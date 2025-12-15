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