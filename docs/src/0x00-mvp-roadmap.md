# 0x00 Project Roadmap

> **Vision**: Build a production-grade cryptocurrency exchange from Hello World to Microsecond Latency.
> **Current Status**: Phase III (Resilience & Funding) - Sentinel Integration.

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

*Status: **Complete***

| Chapter | Title | Description | Status |
|---------|-------|--------------|---|
| 0x0D | [Snapshot & Recovery](./0x0D-snapshot-recovery.md) | State snapshot, crash recovery | ✅ Done |
| 0x0E | [OpenAPI Integration](./0x0E-openapi-integration.md) | Swagger UI, SDK generation | ✅ Done |
| 0x0F | [Admin Dashboard](./0x0F-admin-dashboard.md) | Ops Panel, KYC, hot-reload | ✅ Done |
| 0x11 | [Deposit & Withdraw](./0x11-deposit-withdraw.md) | Mock Chain integration, Idempotency | ✅ Done |
| 0x11-a | [Real Chain Integration](./0x11-a-real-chain.md) | Sentinel Service (Pull Model) | ✅ MVP Done |
| 0x11-b | Sentinel Hardening | **SegWit Fix (DEF-002)** & ETH/ERC20 & ADR-005/006 | ✅ Done |

---

## 🔶 Phase IV: Trading Integration & Verification

*Status: **Pending Verification***

> **Context**: The Core Engine and Trading APIs are implemented but currently tested with **Mocks**. This phase bridges the gap between the Real Chain (0x11) and the Matching Engine (0x01).

| Chapter | Title | Description | Status |
|---------|-------|-------------|---|
| 0x12 | Real Trading Verification | End-to-End: `Bitcoind -> Sentinel -> Order -> Trade` | � Code Ready (Needs Real-Chain Test) |
| 0x13 | Market Data Experience | WebSocket Verification (`Ticker`, `Trade`, `Depth`) | � Code Ready (Needs E2E Test) |

---

## ⏳ Phase V: Extreme Optimization (Metal Mode)

*Status: **In Progress***

> **Codename**: "Metal Mode"
> **Goal**: Push Rust to the physical limits of the hardware.

| Chapter | Title | Description |
|---------|-------|-------------|
| 0x14 | [Extreme Optimization](./0x14-extreme-optimization.md) | ✅ Methodology & Benchmark Harness (0x14-a) |
| 0x15 | [Zero-Copy](./0x15-zero-copy.md) | Zero-copy deserialization with `rkyv` |
| 0x16 | [CPU Affinity](./0x16-cpu-affinity.md) | Core pinning and cache line isolation |
| 0x17 | [SIMD Matching](./0x17-simd-matching.md) | AVX-512 vectorized matching |

---

## 🏆 Key Milestones

| Git Tag | Phase | Highlights |
|---------|-------|------------|
| `v0.09-f-integration-test` | 0x09 | **1.3M orders/sec** baseline achieved |
| `v0.10-a-account-system` | 0x0A | PostgreSQL account integration |
| `v0.10-b-api-auth` | 0x0A | Ed25519 authentication |
| `v0.0C-trade-fee` | 0x0C | Maker/Taker fee system |
| `v0.0D-persistence` | 0x0D | Universal WAL & Snapshot persistence |
| `v0.0F-admin-dashboard` | 0x0F | Admin Operations Dashboard |
| `v0.11-a-funding-qa` | 0x11-a | Real Chain Sentinel MVP (Deposit/Withdraw) |
| `v0.11-b-sentinel-hardening` | 0x11-b | DEF-002 Fix, ADR-005/006, Hot Listing |

---

## 🎯 What You'll Learn

1. **Financial Precision** - Why `f64` fails and how to use fixed-point `u64`
2. **High-Performance Data Structures** - BTreeMap for O(log n) order matching
3. **Lock-Free Concurrency** - LMAX Disruptor-style Ring Buffer
4. **Event Sourcing** - WAL-based deterministic state reconstruction
5. **Real-World Blockchain Integration** - Handling Re-orgs, Confirmations, and UTXO management
6. **Production Security** - Watch-only wallets & Ed25519 authentication

---

*Last Updated: 2025-12-29*
