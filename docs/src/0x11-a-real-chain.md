# 0x11-a Real Chain Integration

<h3>
  <a href="#-english">🇺🇸 English</a>
  &nbsp;&nbsp;&nbsp;|&nbsp;&nbsp;&nbsp;
  <a href="#-chinese">🇨🇳 中文</a>
</h3>

<div id="-english"></div>

## 🇺🇸 English

| Status | ✅ **IMPLEMENTED / QA VERIFIED** (Phase 0x11-a Complete) |
| :--- | :--- |
| **Date** | 2025-12-29 |
| **Context** | Phase 0x11 Extension: From Mock to Reality |
| **Goal** | Integrate real Blockchain Nodes (Regtest/Testnet) and handle distributed system failures (Re-orgs, Network Partition). |

---

## 1. Core Architecture Change: Pull vs Push

The "Mock" phase (0x11) relied on a **Push Model** (API Call -> Deposit).
Real Chain Integration (0x11-a) requires a **Pull Model** (Sentinel -> DB).

### 1.1 The Sentinel (New Service)
A dedicated, independent service loop responsible for "watching" the blockchain.

*   **Block Scanning**: Polls `getblockchaininfo` / `eth_blockNumber`.
*   **Filter**: Index `user_addresses` in memory. Scan every transaction in new blocks against this filter.
*   **State Tracking**: Updates confirmation counts for existing `CONFIRMING` deposits.

## 2. Critical Challenge: Re-org (Chain Reorganization)

In a real blockchain, the "latest" block is not final. It can be orphaned.

### 2.1 Confirmation State Machine
We must expand the Deposit Status flow to handle volatility.

| Status | Confirmations | Action | UI Display |
| :--- | :--- | :--- | :--- |
| **DETECTED** | 0 | Log Tx. Do **NOT** credit balance. | "Confirming (0/X)" |
| **CONFIRMING** | 1 to (X-1) | Update confirmation count. Check for Re-org (BlockHash mismatch). | "Confirming (N/X)" |
| **FINALIZED** | >= X | **Action**: Push `OrderAction::Deposit` to Pipeline. | "Success" |

> [!IMPORTANT]
> **X** represents the `REQUIRED_CONFIRMATIONS` parameter. Hardcoding is forbidden.

### 2.2 Re-org Detection Logic
1.  Sentinel remembers `Block(Height H) = Hash A`.
2.  Sentinel scans `Height H` again later.
3.  If `Hash != A`, a Re-org happened.
4.  **Action**: Rollback scan cursor, re-evaluate all affected deposits.

## 3. Supported Chains (Phase I)

### 3.1 Bitcoin (The UTXO Archetype)
*   **Node**: `bitcoind` (Regtest Mode).
*   **Key Challenge**: **UTXO Management**. A deposit is not a "balance update", it's a new Unspent Output.
*   **Docker**: `ruimarinho/bitcoin-core:24`

### 3.2 Ethereum (The Account/EVM Archetype) - 🚧 PENDING
*   **Status**: Design Complete, Implementation Pending (Phase 0x11-b).
*   **Node**: `anvil` (from Foundry-rs).
*   **Key Challenge**: **Event Log Parsing**. ERC20 deposits are `Transfer` events in receipt logs.
*   **Docker**: `ghcr.io/foundry-rs/foundry:latest`

## 4. Sentinel Architecture (Detailed)

### 4.1 `BtcSentinel` (Implemented)
1.  `getblockhash` -> `getblock` (Verbosity 2).
2.  Iterate outputs `vout`: Match `scriptPubKey` against `user_addresses`.
3.  **Re-org Check**: Keep a rolling window. If `previousblockhash` mismatch, trigger **Rollback**.

### 4.2 `EthSentinel` (Planned for 0x11-b)
1.  `eth_getLogs` (Topic0 = Transfer).
2.  **Re-org Check**: Check `blockHash` of confirmed logs.

## 5. Reconciliation & Safety (The Financial Firewall)

### 5.1 The "Truncation Protocol"
*   **Ingress Logic**: `Deposit_Credited = Truncate(Deposit_Raw, Configured_Precision)`
*   **Residue**: Remainder stays in wallet as "System Dust".

### 5.2 The Triangular Reconciliation
We verify solvency using three independent data sources:

| Source | Alias | Data Point |
| :--- | :--- | :--- |
| **Blockchain RPC** | Proof of Assets (PoA) | `getbalance()` or sum of UTXOs |
| **Internal Ledger** | Proof of Liabilities (PoL) | `SUM(user.available + user.frozen)` |
| **Transaction History** | Proof of Flow (PoF) | `SUM(deposits) - SUM(withdrawals) - SUM(fees)` |

**The Equation**: `PoA == PoL + SystemProfit`

### 5.3 Re-org Recovery Protocol
*   **Shallow Re-org**: Sentinel rolls back cursor.
*   **Deep Re-org (> Max Depth)**: Manual intervention (Freeze + Clawback).

## 6. Database Schema Extensions

```sql
CREATE TABLE chain_cursor (
    chain_id VARCHAR(16) PRIMARY KEY,
    last_scanned_height BIGINT NOT NULL,
    last_scanned_hash VARCHAR(128) NOT NULL,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

ALTER TABLE deposit_history 
ADD COLUMN chain_id VARCHAR(16),
ADD COLUMN block_height BIGINT,
ADD COLUMN block_hash VARCHAR(128),
ADD COLUMN tx_index INT,
ADD COLUMN confirmations INT DEFAULT 0;
```

## 7. Configuration: No Hardcoding

All chain-specific parameters (confirmations, reorg depth, dust threshold) must be loaded from YAML.

## 8. Security: HD Wallet Architecture

### 8.1 Key Storage
*   **Cold Storage**: Private Key (Mnemonic) offline.
*   **Hot Server**: XPUB only.

### 8.2 Address Derivation
*   **BTC**: BIP84 (`m/84'/0'/0'/0/{index}`)
*   **ETH**: BIP44 (`m/44'/60'/0'/0/{index}`)

### 8.3 The "Gap Limit" Solution
*   **Solution**: **Full Index Scanning**. Sentinel loads **ALL** active allocated addresses from the `user_addresses` table into a **HashSet** (Memory) or **Bloom Filter** (Future optimization).
*   **Scanning**: Scan every block transaction output against this set, ignoring standard Gap Limits.

## 9. Future Work (Out of Scope for 0x11-a)
1.  **Bloom Filters**: For million-user address matching (Phase 0x12).
2.  **Automated Clawback**: For deep re-orgs.
3.  **Multi-Source Validation**: Anti-RPC-spoofing.

## 10. Summary
Phase 0x11-a transitions the Funding System to production-ready blockchain integration.

## 11. Implementation Status (2025-12-29)

### 11.1 Completed Features
- **Core Funding**: `DepositService` and `WithdrawService` fully implemented with Integer-Only Persistence (`BigInt/i64`).
- **Sentinel (BTC)**: Basic `BtcScanner` implemented (Polling `getblock`, `HashSet` address matching).
- **Api Layer**: Deposit/Withdraw history APIs fixed (QA-01) and internal auth secured (QA-03).
- **Address Validation**: Strict Regex for BTC/ETH addresses (DEF-001).

### 11.2 Verification & Testing Guide
Run the verified QA suite covering Core, Chaos, and Security scenarios:

```bash
bash scripts/run_0x11a_verification.sh
```

**Results:**
- **Agent B (Core)**: Address Persistence, Deposit/Withdraw Lifecycle ✅
- **Agent A (Chaos)**: Idempotency, Race Condition Resilience ✅
- **Agent C (Security)**: Address Isolation, Internal Auth ✅

### 11.3 Known Limitations (Deferred to 0x11-b)
- **ETH / ERC20 Support**: Real chain integration for Ethereum is **Pending**. `EthScanner` is currently a stub.
- **DEF-002 (Sentinel SegWit)**: The current `bitcoincore-rpc` integration has issues parsing P2WPKH addresses in `regtest`. Sentinel runs but may miss specific SegWit deposits.
- **Bloom Filters**: Currently using `HashSet` for address matching. Bloom Filters deferred to Phase 0x12 optimizations.

<br>
<div align="right"><a href="#-english">↑ Back to Top</a></div>
<br>

---

<div id="-chinese"></div>

## 🇨🇳 中文

| 状态 | ✅ **已实施 / QA 验证通过** (Phase 0x11-a 完成) |
| :--- | :--- |
| **日期** | 2025-12-29 |
| **上下文** | Phase 0x11 扩展: 从模拟到现实 |
| **目标** | 集成真实区块链节点 (Regtest/Testnet) 并处理分布式系统容错 (链重组、网络分区)。 |

---

## 1. 核心架构升级：推 (Push) vs 拉 (Pull)

模拟阶段 (0x11) 依赖 **推模式** (API 调用 -> 触发充值)。
真实链集成 (0x11-a) 必须采用 **拉模式** (哨兵 -> 被动轮询数据库)。

### 1.1 哨兵服务 (Sentinel - 新增组件)
一个独立运行的守护进程，负责持续“注视”区块链。

*   **区块扫描 (Block Scanning)**: 轮询 `getblockchaininfo` (BTC) 或 `eth_blockNumber` (ETH)。
*   **过滤器 (Filter)**: 在内存中索引所有 `user_addresses` (HashSet)。扫描新块交易时进行快速匹配。
*   **状态追踪 (State Tracking)**: 持续跟进 `CONFIRMING` 状态存款的确认数变化。

## 2. 核心挑战：链重组 (Chain Re-org)

真实区块链中，"最新" 区块并非最终态。它随时可能被孤立 (Orphaned)。

### 2.1 确认数状态机 (Confirmation State Machine)
必须扩展存款状态流以处理链的不确定性。

| 状态 | 确认数 | 动作 | UI 显示 |
| :--- | :--- | :--- | :--- |
| **DETECTED** (已检测) | 0 | 记录交易，但 **绝对不** 增加用户余额。 | "确认中 (0/X)" |
| **CONFIRMING** (确认中) | 1 ~ (X-1) | 更新确认数。检查父哈希以防重组。 | "确认中 (N/X)" |
| **FINALIZED** (已完成) | >= X | **动作**: 向撮合引擎提交 `OrderAction::Deposit`。 | "成功" |

> [!IMPORTANT]
> **X** 代表 `REQUIRED_CONFIRMATIONS` (所需确认数) 参数。**禁止硬编码**，必须按链配置。

### 2.2 重组检测逻辑
1.  哨兵记录 `Block(Height H) = Hash A`。
2.  哨兵稍后再次扫描 `Height H`。
3.  如果 `Hash != A`，说明发生了**重组**。
4.  **动作**: 回滚扫描游标 (Cursor)，重新评估所有受影响的存款。

## 3. 支持的链 (第一阶段)

### 3.1 Bitcoin (UTXO 原型)
*   **节点**: `bitcoind` (Regtest 模式)。
*   **挑战**: **UTXO 管理**。比特币存款是新的未花费输出 (UTXO)，而非简单的余额变动。
*   **Docker**: `ruimarinho/bitcoin-core:24`

### 3.2 Ethereum (账户/EVM 原型) - 🚧 待实现
*   **状态**: 设计完成，等待实现 (Phase 0x11-b)。
*   **节点**: `anvil` (Foundry-rs)。
*   **挑战**: **Event Log 解析**。ERC20 存款体现为 Receipt Log 中的 `Transfer` 事件。
*   **Docker**: `ghcr.io/foundry-rs/foundry:latest`

## 4. 哨兵架构详解

### 4.1 `BtcSentinel` (已实现 - 比特币哨兵)
1.  `getblockhash` -> `getblock` (Verbosity 2，获取完整交易细节)。
2.  遍历输出 `vout`: 将 `scriptPubKey` 与 `user_addresses` 匹配。
3.  **重组检查**: 维护一个滚动窗口。如果 `previousblockhash` 不匹配，触发 **回滚 (Rollback)**。

### 4.2 `EthSentinel` (计划中 - 0x11-b)
1.  `eth_getLogs` (Topic0 = Transfer 事件签名)。
2.  **重组检查**: 检查已确认日志的 `blockHash` 是否变更。

## 5. 对账与安全 (金融防火墙)

### 5.1 "截断协议" (The Truncation Protocol)
解决链上浮点数/大整数与系统精度不匹配的问题：
*   **入金逻辑**: `入账金额 = Truncate(链上原始金额, 系统配置精度)`。
*   **系统粉尘 (System Dust)**: 截断后的余数留在热钱包中，归系统所有，不归属用户。

### 5.2 三角对账策略 (Triangular Reconciliation)
使用三个独立数据源验证系统偿付能力：

| 来源 | 别名 | 数据点 |
| :--- | :--- | :--- |
| **区块链 RPC** | 资产证明 (PoA) | `getbalance()` 或 UTXO 总和 |
| **内部账本** | 负债证明 (PoL) | `SUM(user.available + user.frozen)` |
| **流水历史** | 流水证明 (PoF) | `SUM(充值) - SUM(提现) - SUM(手续费)` |

**核心对账公式**: `PoA == PoL + 系统利润`

### 5.3 重组恢复协议
*   **浅层重组**: 哨兵自动回滚游标。
*   **深层重组 (> 最大深度)**: 触发熔断，需人工介入 (冻结提现 + 资金冲正)。

## 6. 数据库模式扩展

```sql
CREATE TABLE chain_cursor (
    chain_id VARCHAR(16) PRIMARY KEY, -- 'BTC', 'ETH'
    last_scanned_height BIGINT NOT NULL,
    last_scanned_hash VARCHAR(128) NOT NULL,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

ALTER TABLE deposit_history 
ADD COLUMN chain_id VARCHAR(16),
ADD COLUMN confirmations INT DEFAULT 0;
-- (其他字段省略)
```

## 7. 配置：拒绝硬编码

所有特定于链的参数（确认数、重组深度、最小入金阈值）必须从 YAML 配置文件加载。

## 8. 安全：HD 钱包架构

### 8.1 密钥存储
*   **冷存储 (离线)**: 私钥/助记词永远离线保存。
*   **热服务 (在线)**: 仅部署 **扩展公钥 (XPUB)**。

### 8.2 地址派生
*   **BTC**: BIP84 (原生 SegWit) `m/84'/0'/0'/0/{index}`
*   **ETH**: BIP44 `m/44'/60'/0'/0/{index}`

### 8.3 "Gap Limit" 解决方案
*   **问题**: 标准钱包在连续 20 个空地址后停止扫描。
*   **方案**: **全索引扫描**。哨兵将 `user_addresses` 表中 **所有** 活跃地址加载到 **HashSet** (当前实现) 或 **Bloom Filter** (未来优化)，无视 Gap Limit。

## 9. 未来工作 (本次范围之外)
1.  **Bloom Filters**: 百万级用户地址匹配优化。
2.  **自动冲正 (Automated Clawback)**: 针对深层重组的自动化处理。
3.  **多源验证**: 对抗单一 RPC 节点被劫持的风险。

## 10. 总结
Phase 0x11-a 将资金系统从模拟环境升级为生产就绪的区块链集成架构。

## 11. 实施状态报告 (2025-12-29)

### 11.1 已完成功能
- **核心资金流**: `DepositService`/`WithdrawService` 实现，并严格遵守整型持久化 (`BigInt/i64`)。
- **哨兵 (BTC)**: 基础 `BtcScanner` 已上线 (轮询 `getblock`, `HashSet` 地址匹配)。
- **API 层**: 充提历史接口已修复 (QA-01)，内部 mock 接口已加固 (QA-03)。
- **地址校验**: 实现 BTC/ETH 下的严格格式正则校验 (DEF-001)。

### 11.2 验证与测试指南
运行全量验证套件 (包含 Core/Chaos/Security 测试):

```bash
bash scripts/run_0x11a_verification.sh
```

**验证结果:**
- **Agent B (Core)**: 地址持久化, 充提生命周期 ✅
- **Agent A (Chaos)**: 幂等性, 竞态条件鲁棒性 ✅
- **Agent C (Security)**: 地址隔离, 内部接口鉴权 ✅

### 11.3 已知限制 (推迟至 0x11-b)
- **ETH / ERC20 支持**: Ethereum 的真实链集成 **尚未实现** (Pending)。`EthScanner` 目前仅为 Stub。
- **DEF-002 (Sentinel SegWit)**: 当前 `bitcoincore-rpc` 集成在 `regtest` 环境下解析 P2WPKH 地址存在问题，可能会漏掉隔离见证存款。
- **Bloom Filter**: 当前版本使用 `HashSet` 进行地址匹配，Bloom Filter 优化推迟至 Phase 0x12。

<br>
<div align="right"><a href="#-chinese">↑ 回到顶部</a></div>
<br>
