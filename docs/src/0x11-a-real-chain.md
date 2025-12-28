# 0x11-a Real Chain Integration

<h3>
  <a href="#-english">🇺🇸 English</a>
  &nbsp;&nbsp;&nbsp;|&nbsp;&nbsp;&nbsp;
  <a href="#-chinese">🇨🇳 中文</a>
</h3>

<div id="-english"></div>

## 🇺🇸 English

> **Status**: 🚧 **Construction** (Detailed Design Phase)
> **Goal**: Integrate real Blockchain Nodes (Regtest/Testnet) and handle distributed system failures (Re-orgs, Network Partition).

---

## 1. Core Architecture Change: Pull vs Push

The "Mock" phase (0x11) relied on a **Push Model** (API Call -> Deposit).
Real Chain Integration (0x11-a) requires a **Pull Model** (Sentinel -> DB).

### 1.1 The Sentinel (New Service)
A dedicated, independent service loop responsible for "watching" the blockchain.

*   **Block Scanning**: Polls `getblockchaininfo` / `eth_blockNumber`.
*   **Filter**: Index `user_addresses` in memory. Scan every transaction in new blocks against this filter.
*   **State Tracking**: Updates confirmation counts for existing `CONFIRMING` deposits.

## 2. Supported Chains (Phase I)

### 2.1 Bitcoin (The UTXO Archetype)
*   **Node**: `bitcoind` (Regtest Mode).
*   **Key Challenge**: **UTXO Management**. A deposit is not a "balance update", it's a new Unspent Output. Re-orgs can invalidate specific inputs.
*   **Docker**: `ruimarinho/bitcoin-core:24`

### 2.2 Ethereum (The Account/EVM Archetype)
*   **Node**: `anvil` (from Foundry-rs).
*   **Key Challenge**: **Event Log Parsing**. ERC20 deposits are `Transfer` events in receipt logs, not native ETH transfers.
*   **Docker**: `ghcr.io/foundry-rs/foundry:latest`

---

## 3. Reconciliation & Safety (The Financial Firewall)

### 3.1 The "Truncation Protocol" (100% Match)
To solve the "Floating Point Curse" on-chain:

*   **Precision Constraint**: The system supports `N` decimals as defined in **Asset Configuration** (e.g., `ETH`=12 or 18).
*   **Ingress Logic**:
    *   `Deposit_Credited = Truncate(Deposit_Raw, Configured_Precision)`
    *   *Residue*: `Deposit_Raw - Deposit_Credited` remains in the wallet as "System Dust".
*   **Reconciliation Equation**:
    ```text
    Truncate(Wallet_Start + Deposits - Withdrawals - GasFees, N) 
    == 
    Sum(User_Balances)
    ```
*   **Alerting**: **Zero Tolerance**. Any deviation triggers **P0 Alert** and suspends withdrawals.

### 3.2 Re-org Recovery Protocol
We must handle two types of Re-orgs:

#### 3.2.1 Shallow Re-org (Before Finalization)
*   **Scenario**: Block 100 (Hash A) -> Block 100 (Hash B).
*   **Action**: Sentinel detects hash mismatch, rolls back `chain_cursor`, and marks orphaned deposits as `ORPHANED`. No user balance impact.

#### 3.2.2 Deep Re-org (The "Clawback")
*   **Scenario**: User credited after 6 confs, but chain re-orgs 10 blocks deep (51% attack/network split).
*   **Action**:
    1.  Sentinel detects deep re-org.
    2.  Engine injects `OrderAction::ForceDeduct` (Administrative Correction).
    3.  User balance might go negative. Account frozen until settled.

---

## 4. Wallet Architecture (Warm/Cold)

### 4.1 Address Derivation
*   **Standard**: BIP32/BIP44/BIP84.
*   **Pattern**: **Watch-Only**.
    *   Server only holds **Extended Public Key (`xpub`)**.
    *   Private keys stay offline (Cold Storage) for Phase I.
    *   Hot Wallet signing done via separate isolated signer (or Mock for Stage).

### 4.2 The "Gap Limit" Solution
*   **Problem**: HD Wallets stop scanning after 20 unused addresses.
*   **Solution**: **Full Index Scanning**.
    *   Sentinel loads **ALL** active allocated addresses into a **Bloom Filter**.
    *   Scans every block against this filter, ignoring gap limits.

---

<br>
<div align="right"><a href="#-english">↑ Back to Top</a></div>
<br>

---

<div id="-chinese"></div>

## 🇨🇳 中文

> **状态**: 🚧 **设计阶段**
> **目标**: 集成真实区块链节点 (Regtest/Testnet) 并处理分布式系统故障 (分叉、重组)。

---

## 1. 核心架构变更: Pull vs Push

Mock 阶段 (0x11) 依赖 **Push 模型** (API 调用 -> 充值)。
真实链集成 (0x11-a) 需要 **Pull 模型** (哨兵 -> 数据库)。

### 1.1 哨兵服务 (Sentinel)
一个独立的、死循环的服务，负责 "注视" 区块链。

*   **区块扫描**: 轮询 `getblockchaininfo` / `eth_blockNumber`。
*   **过滤器**: 内存中索引所有 `user_addresses`。
*   **状态追踪**: 更新 `CONFIRMING` 状态存款的确认数。

## 2. 支持链 (第一阶段)

### 2.1 Bitcoin (UTXO 原型)
*   **节点**: `bitcoind` (Regtest 模式)。
*   **挑战**: **UTXO 管理**。存款是新的 UTXO，而不是余额数字更新。
*   **Docker**: `ruimarinho/bitcoin-core:24`

### 2.2 Ethereum (账户/EVM 原型)
*   **节点**: `anvil` (Foundry-rs)。
*   **挑战**: **Event Log 解析**。ERC20 存款是 Log 中的 `Transfer` 事件。
*   **Docker**: `ghcr.io/foundry-rs/foundry:latest`

---

## 3. 对账与安全 (金融防火墙)

### 3.1 "截断协议" (100% 匹配)
解决链上浮点数问题：

*   **精度约束**: 系统仅支持配置定义的 `N` 位小数 (如 ETH=12)。
*   **入金逻辑**: `入账金额 = Truncate(链上原始金额, N)`。
*   **对账公式**:
    ```text
    Truncate(钱包初始 + 充值 - 提现 - Gas费, N) 
    == 
    Sum(用户余额)
    ```
*   **报警**: **零容忍**。任何偏差触发 P0 报警并暂停提现。

### 3.2 重组恢复协议 (Re-org Recovery)

#### 3.2.1 浅层重组 (Finalization 之前)
*   **场景**: 区块 100 (Hash A) 变为 (Hash B)。
*   **动作**: 哨兵发现 Hash 不匹配，回滚 `chain_cursor`，标记孤块存款为 `ORPHANED`。不影响用户余额。

#### 3.2.2 深层重组 ("回撤" Clawback)
*   **场景**: 6 确认后入账，但链发生 10个块的深层重组。
*   **动作**:
    1.  哨兵检测到深层重组。
    2.  引擎注入 `OrderAction::ForceDeduct` (行政冲正)。
    3.  用户余额可能变为负数。账户冻结直至平账。

---

## 4. 钱包架构 (温/冷)

### 4.1 地址派生
*   **标准**: BIP32/BIP44/BIP84。
*   **模式**: **Watch-Only** (只读)。
    *   服务器仅持有 **扩展公钥 (`xpub`)**。
    *   私钥保持离线 (冷存储)。

### 4.2 "Gap Limit" 解决方案
*   **问题**: HD 钱包通常在遇到 20 个未使用地址后停止扫描。
*   **方案**: **全索引扫描**。
    *   哨兵将 **所有** 已分配地址加载到 **Bloom Filter**。
    *   扫描每个区块的所有输出，忽略 Gap Limit。
