# 0x11-a Real Chain Integration

<h3>
  <a href="#-english">🇺🇸 English</a>
  &nbsp;&nbsp;&nbsp;|&nbsp;&nbsp;&nbsp;
  <a href="#-chinese">🇨🇳 中文</a>
</h3>

<div id="-english"></div>

## 🇺🇸 English

> **📦 Code Changes**: [View Diff](https://github.com/gjwang/zero_x_infinity/compare/v0.11-deposit-withdraw...v0.11-a-real-chain)

> **Core Objective**: Transition from a simulated "Mock Chain" to a true blockchain integration using **Sentinel Pull-Model** architecture.

---

## 1. Background: Why This Phase?

In [Phase 0x11](./0x11-deposit-withdraw.md), we built a complete Deposit and Withdraw system using a "Mock Chain". While this was excellent for validating internal logic (idempotency, balance crediting, risk checks), it had a critical limitation:

> **The system waited for _external API calls_ to tell it about deposits.**

In the real world, this is backwards. Blockchains don't call your API. **You must actively watch the blockchain for incoming transactions.**

This phase, **0x11-a**, introduces the **Sentinel Service**—an independent process that continuously scans blockchain nodes and pulls deposit information into our system.

### 1.1 The Fundamental Shift: Push vs. Pull

| Aspect | 0x11 (Mock) | 0x11-a (Real) |
| :--- | :--- | :--- |
| **Data Source** | API Call (`/internal/mock/deposit`) | Blockchain Node (`bitcoind`, `anvil`) |
| **Initiation** | External System Pushes | Internal Sentinel Pulls |
| **Trust Model** | Trust the Caller | Trust the Consensus |
| **Finality** | Instant | Requires N Confirmations |

### 1.2 Key Questions This Phase Answers

1.  **How do we know a deposit happened?** By scanning every new block.
2.  **How do we know a deposit is "real"?** By waiting for enough confirmations.
3.  **What if the blockchain forks (re-org)?** By tracking block hashes and rolling back.

---

## 2. The Sentinel Service: Core Concepts

The **Sentinel** is a dedicated, continuously-running service with one job: **Watch the blockchain and record deposits.**

### 2.1 Why a Separate Service?

The Matching Engine must be deterministic and fast. It should never block on network I/O. By isolating the blockchain-scanning logic into the Sentinel, we achieve:

*   **Decoupling**: Engine knows nothing about `bitcoind`. Sentinel knows nothing about order matching.
*   **Resilience**: If the Sentinel crashes, the Engine continues trading. When the Sentinel restarts, it picks up where it left off using the `chain_cursor`.
*   **Scalability**: We can run multiple Sentinels for different chains without affecting Engine performance.

### 2.2 The Sentinel Loop (Simplified)

```text
loop forever:
  1. Get the current block height from the node.
  2. Compare with "chain_cursor" (our last known position).
  3. If new blocks exist:
     a. Fetch the next block.
     b. Check: Does this block's parent hash match our last scanned hash?
        - YES: Proceed.
        - NO:  A RE-ORG happened! Roll back and rescan.
     c. For each transaction in the block:
        - Does any output match a user's deposit address?
        - If YES, record it as a "DETECTED" deposit.
     d. Update chain_cursor atomically.
  4. Sleep for a configured interval (e.g., 10 seconds).
```

---

## 3. The Challenge: Blockchain Finality & Re-orgs

Unlike a traditional database where a `COMMIT` is final, blockchains are **probabilistically final**. A block that exists now might be orphaned a minute later.

### 3.1 Why Re-orgs Happen

In Proof-of-Work (Bitcoin), two miners might find a valid block at roughly the same time. The network temporarily has two competing chains. Eventually, one chain becomes longer, and the shorter one is abandoned—its transactions are "orphaned."

This means: **A deposit you saw in block 100 might disappear if block 100 gets replaced.**

### 3.2 The Confirmation State Machine

To handle this, we don't credit a deposit immediately. Instead, we track its **confirmation count**.

| Status | Confirmations | User Balance Impact | UI Display |
| :--- | :--- | :--- | :--- |
| **DETECTED** | 0 | ❌ No credit | "Confirming (0/X)" |
| **CONFIRMING** | 1 to (X-1) | ❌ No credit | "Confirming (N/X)" |
| **FINALIZED** | >= X | ✅ Balance credited | "Success" |
| **ORPHANED** | N/A (Re-org) | ❌ No impact (never credited) | "Failed - Re-org" |

> [!IMPORTANT]
> **X (Required Confirmations)** is a per-chain configuration. Bitcoin typically uses 6. Ethereum uses 12-35. Solana might use 1 (due to different finality model). **Hardcoding is forbidden.**

### 3.3 Re-org Detection: Parent Hash Validation

The Sentinel detects a re-org by checking if the **parent hash** of the new block matches the hash of the block we last scanned.

```text
Stored Cursor: { height: 100, hash: "ABC" }
New Block 101: { parent_hash: "ABC", hash: "DEF" }
-> Parent matches! Proceed normally.

Stored Cursor: { height: 100, hash: "ABC" }
New Block 101: { parent_hash: "XYZ", hash: "QRS" }
-> Parent MISMATCH! Block 100 was replaced. Trigger RE-ORG RECOVERY.
```

**Recovery Action**:
1.  Roll back `chain_cursor` to a known-good height (e.g., 99).
2.  Mark all deposits from block 100+ as `ORPHANED` (if not yet finalized).
3.  Rescan from the rolled-back height.

---

## 4. Infrastructure: Supported Chains

We focus on two archetypes to ensure robust, generalized design.

### 4.1 Bitcoin (UTXO Model)

*   **Node**: `bitcoind` running in Regtest mode for local testing.
*   **RPC**: `getblockcount`, `getblockhash`, `getblock` (verbosity=2 for full tx details).
*   **Challenge**: Deposits are new Unspent Transaction Outputs (UTXOs), not balance increments. We scan `vout` arrays and match `scriptPubKey` to addresses.
*   **Docker**: `ruimarinho/bitcoin-core:24`

### 4.2 Ethereum (Account Model)

*   **Node**: `anvil` (Foundry's local EVM node) for fast, feature-rich local testing.
*   **RPC**: `eth_blockNumber`, `eth_getBlockByNumber`, `eth_getLogs`.
*   **Challenge**: ERC-20 token deposits are `Transfer` event logs, not native ETH transfers. We must filter by `topic0` (event signature) and `topic2` (recipient address).
*   **Docker**: `ghcr.io/foundry-rs/foundry:latest`

---

## 5. Financial Safety: The Reconciliation Equation

A core principle of exchange engineering: **Your liabilities (user balances) must always equal your assets (wallet balances) minus system profit.**

### 5.1 The "Truncation Protocol"

Blockchains use high precision (BTC: 8 decimals, ETH: 18 decimals). To prevent floating-point errors from causing reconciliation mismatches, we enforce a **Truncation Protocol**:

1.  **On Ingress**: `Credited_Amount = Truncate(RawAmount, SystemPrecision)`
2.  **Residue**: Any sub-precision dust remains in the wallet as "System Dust."

This ensures that when we sum all user balances and compare to the wallet balance, the equation holds exactly (no floating-point drift).

### 5.2 The Triangular Reconciliation

We verify solvency using three independent data sources:

| Source | Alias | Data Point |
| :--- | :--- | :--- |
| **Blockchain RPC** | Proof of Assets (PoA) | `getbalance()` or sum of UTXOs |
| **Internal Ledger** | Proof of Liabilities (PoL) | `SUM(user.available + user.frozen)` |
| **Transaction History** | Proof of Flow (PoF) | `SUM(deposits) - SUM(withdrawals) - SUM(fees)` |

**The Equation**: `PoA == PoL + SystemProfit`

Any deviation triggers a **Circuit Breaker** that halts all withdrawals until manually investigated.

---

## 6. Database Schema Extensions

To support the Sentinel, we extend the database with new tables and columns.

### 6.1 `chain_cursor` Table

Tracks how far the Sentinel has scanned for each chain. This enables resumption after restarts.

```sql
CREATE TABLE chain_cursor (
    chain_id VARCHAR(16) PRIMARY KEY, -- 'BTC', 'ETH'
    last_scanned_height BIGINT NOT NULL,
    last_scanned_hash VARCHAR(128) NOT NULL,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
```

### 6.2 `deposit_history` Enhancements

We add on-chain metadata to enable re-org detection and confirmation tracking.

```sql
ALTER TABLE deposit_history 
ADD COLUMN chain_id VARCHAR(16),
ADD COLUMN block_height BIGINT,
ADD COLUMN block_hash VARCHAR(128),
ADD COLUMN tx_index INT,
ADD COLUMN confirmations INT DEFAULT 0;

CREATE INDEX idx_deposit_reorg ON deposit_history(chain_id, block_height);
```

---

## 7. Configuration: No Hardcoding

All chain-specific parameters must be loaded from configuration files, not hardcoded.

### 7.1 Key Parameters

| Parameter | Description | Example (BTC Mainnet) | Example (ETH) |
| :--- | :--- | :--- | :--- |
| `REQUIRED_CONFIRMATIONS` | Blocks needed before crediting | 6 | 12 |
| `MAX_REORG_DEPTH` | Depth beyond which manual intervention is required | 10 | 35 |
| `MIN_DEPOSIT_THRESHOLD` | Ignore deposits below this value (dust protection) | 0.0001 BTC | 0.001 ETH |
| `MAX_BLOCK_LAG_SECONDS` | Alert if node is stale | 3600 (1 hour) | 600 (10 min) |

### 7.2 Config File Structure

```yaml
# config/chains/btc_regtest.yaml
chain_id: BTC
rpc_url: http://127.0.0.1:18443
rpc_user: admin
rpc_password: admin
required_confirmations: 1  # Regtest: fast testing
max_reorg_depth: 10
min_deposit_threshold: 0.00001
```

---

## 8. Security: HD Wallet Architecture

To protect user funds, we use a **Watch-Only** wallet pattern.

### 8.1 Key Storage

*   **Cold Storage (Offline)**: The master private key (mnemonic) is NEVER on any server.
*   **Hot Server**: Only the **Extended Public Key (XPUB)** is deployed. This allows address generation but NOT spending.

### 8.2 Address Derivation

We follow BIP32/BIP44/BIP84 standards:

*   **BTC (SegWit)**: `m/84'/0'/0'/0/{index}` (BIP84)
*   **ETH**: `m/44'/60'/0'/0/{index}` (BIP44)

When a user requests a deposit address, the server:
1.  Atomically increments the `address_index` counter for that chain.
2.  Derives the address from the XPUB at that index.
3.  Stores the `{user_id, asset, address, index}` mapping.

**Security Guarantee**: Even if the entire database and server are compromised, attackers cannot steal funds without the offline private key.

---

## 9. Future Work (Out of Scope for 0x11-a)

The following are recognized as important but are deferred to later phases:

1.  **Bloom Filters**: For million-user address matching. Current implementation uses HashMap (sufficient for <10k addresses).
2.  **Automated Clawback**: For deep re-orgs that invalidate already-credited deposits. Current implementation triggers a manual audit.
3.  **Multi-Source Validation**: Checking block hashes against multiple nodes to detect compromised RPCs.

---

## Summary

Phase 0x11-a transitions the Funding System from a simulated environment to production-ready blockchain integration.

**Key Achievements**:
1.  **Sentinel Service**: An independent, pull-based blockchain scanner.
2.  **Confirmation State Machine**: Safe handling of blockchain's probabilistic finality.
3.  **Re-org Recovery**: Automatic detection and rollback for shallow forks.
4.  **Configuration-Driven**: All thresholds are per-chain, no hardcoding.
5.  **Financial Safety**: Truncation Protocol + Triangular Reconciliation.

**Next Step**:
> **Phase 0x11-b**: Address DEF-002 (Sentinel SegWit parsing) and prepare for Mainnet deployment.

<br>
<div align="right"><a href="#-english">↑ Back to Top</a></div>
<br>

---

<div id="-chinese"></div>

## 🇨🇳 中文

> **📦 代码变更**: [查看 Diff](https://github.com/gjwang/zero_x_infinity/compare/v0.11-deposit-withdraw...v0.11-a-real-chain)

> **核心目标**: 从模拟"Mock Chain"过渡到使用**哨兵拉取模型 (Sentinel Pull-Model)** 架构的真实区块链集成。

---

## 1. 背景：为什么需要这个阶段？

在 [Phase 0x11](./0x11-deposit-withdraw.md) 中，我们使用"Mock Chain"构建了完整的充值和提现系统。虽然这对于验证内部逻辑（幂等性、余额记账、风控检查）非常有效，但它有一个关键的局限性：

> **系统依赖于_外部 API 调用_来告知充值信息。**

在现实世界中，这是本末倒置的。区块链不会主动调用你的 API。**你必须主动监控区块链以发现入账的交易。**

本阶段 **0x11-a** 引入了 **哨兵服务 (Sentinel Service)**——一个独立的进程，持续扫描区块链节点并将充值信息拉取到我们的系统中。

### 1.1 核心转变：Push vs. Pull

| 方面 | 0x11 (模拟) | 0x11-a (真实) |
| :--- | :--- | :--- |
| **数据来源** | API 调用 (`/internal/mock/deposit`) | 区块链节点 (`bitcoind`, `anvil`) |
| **触发方式** | 外部系统推送 | 内部哨兵拉取 |
| **信任模型** | 信任调用者 | 信任共识 |
| **终局性** | 即时 | 需要 N 个确认 |

---

## 2. 哨兵服务：核心概念

**哨兵 (Sentinel)** 是一个专门的、持续运行的服务，只有一个任务：**监控区块链并记录充值。**

### 2.1 为什么是独立服务？

撮合引擎必须是确定性的且快速的。它不应该因网络 I/O 而阻塞。通过将区块链扫描逻辑隔离到哨兵中，我们实现了：

*   **解耦**: 引擎不知道 `bitcoind`。哨兵不知道订单撮合。
*   **弹性**: 如果哨兵崩溃，引擎继续交易。当哨兵重启时，它使用 `chain_cursor` 从上次的位置继续。
*   **可扩展性**: 我们可以为不同的链运行多个哨兵，而不影响引擎性能。

---

## 3. 挑战：区块链终局性与重组

与传统数据库中 `COMMIT` 是最终的不同，区块链是**概率性最终的**。现在存在的区块可能一分钟后就被孤立了。

### 3.1 确认状态机

为了处理这个问题，我们不会立即记账充值。相反，我们跟踪其**确认数**。

| 状态 | 确认数 | 用户余额影响 | UI 显示 |
| :--- | :--- | :--- | :--- |
| **DETECTED** | 0 | ❌ 不记账 | "确认中 (0/X)" |
| **CONFIRMING** | 1 到 (X-1) | ❌ 不记账 | "确认中 (N/X)" |
| **FINALIZED** | >= X | ✅ 余额已记账 | "成功" |
| **ORPHANED** | N/A (重组) | ❌ 无影响 (从未记账) | "失败 - 重组" |

> [!IMPORTANT]
> **X (所需确认数)** 是按链配置的。比特币通常使用 6，以太坊使用 12-35。**禁止硬编码。**

### 3.2 重组检测：父哈希验证

哨兵通过检查新区块的**父哈希**是否与我们上次扫描的区块的哈希匹配来检测重组。

**恢复动作**:
1.  将 `chain_cursor` 回滚到已知良好的高度。
2.  将受影响区块的所有充值标记为 `ORPHANED`（如果尚未最终化）。
3.  从回滚的高度重新扫描。

---

## 4. 金融安全：对账方程

交易所工程的核心原则：**你的负债（用户余额）必须始终等于你的资产（钱包余额）减去系统利润。**

### 4.1 三方对账

我们使用三个独立的数据源验证偿付能力：

| 来源 | 别名 | 数据点 |
| :--- | :--- | :--- |
| **区块链 RPC** | 资产证明 (PoA) | `getbalance()` 或 UTXO 之和 |
| **内部账本** | 负债证明 (PoL) | `SUM(user.available + user.frozen)` |
| **交易历史** | 流水证明 (PoF) | `SUM(充值) - SUM(提现) - SUM(手续费)` |

**方程**: `PoA == PoL + 系统利润`

任何偏差都会触发**熔断器**，暂停所有提现直到人工调查。

---

## 5. 配置：禁止硬编码

所有链特定的参数必须从配置文件加载，不能硬编码。

| 参数 | 描述 | 示例 (BTC 主网) | 示例 (ETH) |
| :--- | :--- | :--- | :--- |
| `REQUIRED_CONFIRMATIONS` | 记账前所需区块数 | 6 | 12 |
| `MAX_REORG_DEPTH` | 超过此深度需人工介入 | 10 | 35 |
| `MIN_DEPOSIT_THRESHOLD` | 忽略低于此值的充值（防尘攻击） | 0.0001 BTC | 0.001 ETH |

---

## 6. 安全：HD 钱包架构

为保护用户资金，我们使用**只读钱包 (Watch-Only)** 模式。

*   **冷存储 (离线)**: 主私钥（助记词）**绝不**存储在任何服务器上。
*   **热服务器**: 仅部署**扩展公钥 (XPUB)**。这允许生成地址但**无法**花费资金。

**安全保证**: 即使整个数据库和服务器都被攻破，攻击者在没有离线私钥的情况下也无法盗取资金。

---

## 总结

Phase 0x11-a 将资金系统从模拟环境过渡到生产就绪的区块链集成。

**关键成就**:
1.  **哨兵服务**: 独立的、基于拉取的区块链扫描器。
2.  **确认状态机**: 安全处理区块链的概率性终局性。
3.  **重组恢复**: 自动检测和回滚浅层分叉。
4.  **配置驱动**: 所有阈值按链配置，无硬编码。
5.  **金融安全**: 截断协议 + 三方对账。

**下一步**:
> **Phase 0x11-b**: 解决 DEF-002（哨兵 SegWit 解析）并准备主网部署。

<br>
<div align="right"><a href="#-chinese">↑ 回到顶部</a></div>
<br>
