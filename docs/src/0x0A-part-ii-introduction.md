# Part II: Productization

<h3>
  <a href="#-english">🇺🇸 English</a>
  &nbsp;&nbsp;&nbsp;|&nbsp;&nbsp;&nbsp;
  <a href="#-chinese">🇨🇳 中文</a>
</h3>

<div id="-english"></div>

## 🇺🇸 English

> **📦 Code Changes**: [View Diff](https://github.com/gjwang/zero_x_infinity/compare/v0.9-f-integration-test...main)

> **Core Objective**: Upgrade the core matching engine into a complete trading system with Account System, Fund Transfer, and Security Authentication.

---

## 1. Review: Achievements of Part I

| Chapter | Topic | Key Achievement |
|---------|-------|-----------------|
| **0x01** | Genesis | Minimal Matching Prototype |
| **0x02-03** | Floats & Decimals | Financial Grade Precision |
| **0x04** | BTree OrderBook | O(log n) Matching |
| **0x05-06** | User Balance | Locking/Unlocking |
| **0x07** | Testing Framework | 100K Order Baseline |
| **0x08** | Multi-Thread Pipeline | 4-Thread Concurrency |
| **0x09** | Gateway & Persistence | Gateway, TDengine, WebSocket |

---

## 2. Gap Analysis: From Engine to System

| Dimension | Current State | Target State |
|-----------|---------------|--------------|
| **Identity** | Raw `user_id` | API Key Signature |
| **Accounts** | Single Balance | Funding + Spot Dual-Account |
| **Funds** | Manual `deposit()` | Deposit/Withdraw/Transfer |
| **Economics** | Zero Fee | Maker/Taker Fees |

---

## 3. Blueprint for Part II

```
0x0A ─── Account System & Security
        ├── 0x0A-a: Account System (exchange_info + DB)
        ├── 0x0A-b: ID Specification (Asset/Symbol Naming)
        └── 0x0A-c: Authentication (API Key Middleware)

0x0B ─── Fund System & Transfers
        ├── Funding/Spot Dual-Account Structure
        └── Deposit/Withdraw API

0x0C ─── Economic Model
        └── Fee Calculation & Deduction

0x0D ─── Snapshot & Recovery
        └── Graceful Shutdown & State Restoration
```

---

## 4. Tech Stack Choices

| Component | Choice | Purpose |
|-----------|--------|---------|
| **PostgreSQL 18** | Account/Asset/Symbol | Relational Config Data |
| **TDengine** | Orders/Trades/K-Lines | Time-Series Trading Data |
| **sqlx** | Rust PG Driver | Async + Compile-time Check |

---

## 5. Design Principles

| Principle | Description |
|-----------|-------------|
| **Minimal External Deps** | Auth/Transfer logic is cohesive |
| **Auditability** | All fund changes must have event logs |
| **Progressive** | System remains runnable after each module |
| **Backward Compatible** | Reuse Core types from Part I |

<br>
<div align="right"><a href="#-english">↑ Back to Top</a></div>
<br>

---

<div id="-chinese"></div>

## 🇨🇳 中文

> **📦 代码变更**: [查看 Diff](https://github.com/gjwang/zero_x_infinity/compare/v0.9-f-integration-test...main)

> **核心目的**：将撮合引擎核心升级为具备账户体系、资金划转和安全鉴权的完整交易系统。

---

## 1. 回顾：第一部分的成就

| 章节 | 主题 | 关键成果 |
|------|------|----------|
| **0x01** | 创世纪 | 最简撮合原型 |
| **0x02-03** | 浮点数与定点数 | 金融级精度保障 |
| **0x04** | BTree OrderBook | O(log n) 撮合 |
| **0x05-06** | 用户余额 | 锁定/解锁机制 |
| **0x07** | 测试框架 | 100K 订单基线 |
| **0x08** | 多线程 Pipeline | 四线程并发架构 |
| **0x09** | 接入层 & 持久化 | Gateway, TDengine, WebSocket |

---

## 2. 差距分析：从引擎到系统

| 维度 | 当前状态 | 目标状态 |
|------|----------|----------|
| **身份认证** | `user_id` 裸奔 | API Key 签名校验 |
| **账户管理** | 单一余额结构 | Funding + Spot 双账户 |
| **资金流转** | 手动 `deposit()` | 完整充提+划转流程 |
| **经济模型** | 零手续费 | Maker/Taker 费率 |

---

## 3. 第二部分蓝图

```
0x0A ─── 账户体系与安全鉴权
        ├── 0x0A-a: 账户体系 (exchange_info + DB 管理)
        ├── 0x0A-b: ID 规范 (Asset/Symbol 命名)
        └── 0x0A-c: 安全鉴权 (API Key 中间件)

0x0B ─── 资金体系与划转
        ├── Funding/Spot 双账户结构
        └── 充提币 API

0x0C ─── 经济模型
        └── 手续费计算与扣除

0x0D ─── 快照与恢复
        └── 优雅停机与状态恢复
```

---

## 4. 技术选型

| 组件 | 选型 | 用途 |
|------|------|------|
| **PostgreSQL 18** | 账户/资产/交易对 | 关系型配置数据 |
| **TDengine** | 订单/成交/K线 | 时序交易数据 |
| **sqlx** | Rust PG Driver | 异步 + 编译时检查 |

---

## 5. 设计原则

| 原则 | 说明 |
|------|------|
| **最小外部依赖** | 鉴权、划转等逻辑内聚 |
| **可审计性** | 所有资金变动必须有完整事件流水 |
| **渐进式增强** | 每个子模块完成后保持系统可运行 |
| **向后兼容** | 复用 Part I 的核心类型 |
