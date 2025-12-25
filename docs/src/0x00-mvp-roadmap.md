# 0x00 Project Roadmap

> **Vision**: Build a production-grade cryptocurrency exchange from Hello World to Microsecond Latency.

---

## 📊 Progress Overview

This project documents the complete journey of building a **1.3M orders/sec** matching engine. Below is the current status of each phase.

---

## ✅ Phase I: Core Matching Engine

*Status: **Complete***

| Chapter | Title | Description |
|---------|-------|-------------|
| 0x01 | [Genesis](./0x01-genesis.md) | Basic OrderBook with `Vec<Order>` |
| 0x02 | [Float Curse](./0x02-the-curse-of-float.md) | Why floats fail → `u64` refactoring |
| 0x03 | [Decimal World](./0x03-decimal-world.md) | Precision configuration system |
| 0x04 | [BTree OrderBook](./0x04-btree-orderbook.md) | BTreeMap-based order book |
| 0x05 | [User Balance](./0x05-user-balance.md) | Account & balance management |
| 0x06 | [Enforced Balance](./0x06-enforced-balance.md) | Type-safe fund locking |
| 0x07 | [Testing Framework](./0x07-a-testing-framework.md) | 1M order batch testing |
| 0x08 | [Trading Pipeline](./0x08-a-trading-pipeline-design.md) | LMAX-style Ring Buffer architecture |
| 0x09 | [Gateway & Persistence](./0x09-a-gateway.md) | HTTP API, TDengine, WebSocket, K-Line |

---

## ✅ Phase II: Productization

*Status: **Complete***

| Chapter | Title | Description |
|---------|-------|-------------|
| 0x0A | [Account System](./0x0A-a-account-system.md) | PostgreSQL user management |
| 0x0A-b | [ID Specification](./0x0A-b-id-specification.md) | Identity addressing rules |
| 0x0A-c | [API Authentication](./0x0A-c-api-auth.md) | Ed25519 cryptographic auth |
| 0x0B | [Funding & Transfer](./0x0B-funding.md) | Internal transfer architecture |
| 0x0C | [Trade Fee](./0x0C-trade-fee.md) | Maker/Taker fees + VIP discount |

---

## 🔶 Phase III: Resilience & Funding

*Status: **In Progress** (0x0D Complete)*

| Chapter | Title | Description |
|---------|-------|-------------|
| 0x0D | [Snapshot & Recovery](./0x0D-snapshot-recovery.md) | ✅ State snapshot, crash recovery |
| 0x0E | Deposit & Withdraw | Blockchain integration (planned) |
| 0x0F | Frontend | Trading UI (planned) |

---

## ⏳ Phase IV: Extreme Optimization

*Status: **Planned***

| Chapter | Title | Description |
|---------|-------|-------------|
| 0x10 | [Zero-Copy](./0x10-zero-copy.md) | Zero-copy deserialization |
| 0x11 | [CPU Affinity](./0x11-cpu-affinity.md) | Cache-friendly optimization |
| 0x12 | [SIMD Matching](./0x12-simd-matching.md) | Vectorized acceleration |

---

## 🏆 Key Milestones

| Git Tag | Phase | Highlights |
|---------|-------|------------|
| `v0.09-f-integration-test` | 0x09 | **1.3M orders/sec** baseline achieved |
| `v0.10-a-account-system` | 0x0A | PostgreSQL account integration |
| `v0.10-b-api-auth` | 0x0A | Ed25519 authentication |
| `v0.0B-a-transfer-fsm` | 0x0B | ULID-based FSM transfer |
| `v0.0C-trade-fee` | 0x0C | Maker/Taker fee system |
| `v0.0D-persistence` | 0x0D | Universal WAL & Snapshot persistence |

---

## 🎯 What You'll Learn

By following this project, you will learn:

1. **Financial Precision** - Why `f64` fails and how to use fixed-point `u64`
2. **High-Performance Data Structures** - BTreeMap for O(log n) order matching
3. **Lock-Free Concurrency** - LMAX Disruptor-style Ring Buffer
4. **Event Sourcing** - WAL-based deterministic state reconstruction
5. **Production Architecture** - PostgreSQL + TDengine dual-database design
6. **Cryptographic Security** - Ed25519 asymmetric authentication
7. **Financial Integrity** - Maker/Taker fee calculation with 10^6 precision

---

## 📚 References

- [Performance Report](./perf-report.md) - Latest benchmark results
- [Database Selection](./database-selection-tdengine.md) - Why TDengine?
- [API Conventions](../standards/api-conventions.md) - REST API standards

---

*Last Updated: 2024-12-25*
