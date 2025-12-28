# 0x11-a Real Chain Integration

| Status | **DRAFT** |
| :--- | :--- |
| **Date** | 2025-12-28 |
| **Context** | Phase 0x11 Extension: From Mock to Reality |
| **Goal** | Integrate real Blockchain Nodes (Regtest/Testnet) and handle distributed system failures. |

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
| **DETECTED** | 0 | Log Tx. Do **NOT** credit balance. | "Confirming (0/6)" |
| **CONFIRMING** | 1-5 | Update confirmation count. Check for Re-org (BlockHash mismatch). | "Confirming (N/6)" |
| **FINALIZED** | >= 6 | **Action**: Push `OrderAction::Deposit` to Pipeline. | "Success" |
| **ORPHANED** | N/A | Tx disappeared from chain. Mark as `FAILED`. | "Failed" |

### 2.2 Re-org Detection Logic
1.  Sentinel remembers `Block(Height H) = Hash A`.
2.  Sentinel scans `Height H` again later.
3.  If `Hash != A`, a Re-org happened.
4.  **Action**: Rollback scan cursor, re-evaluate all affected deposits.

## 3. Infrastructure Requirements

To implement 0x11-a, we need "Real" nodes running in Docker.

### 3.1 Bitcoin (Regtest)
*   Image: `ruimarinho/bitcoin-core`
*   Command: `bitcoind -regtest -server -rpcuser=...`
*   **Reason**: `Regtest` allows instant block generation (`generatetoaddress`), essential for integration testing Re-org scenarios.

### 3.2 Ethereum (Anvil/Geth)
*   Image: `foundry-rs/foundry` (Anvil) or `ethereum/go-ethereum` (Dev mode).
*   **Reason**: Fast block times, snapshot/reset capability.

## 4. Implementation Constraints
1.  **Strict Isolation**: `Sentinel` Service MUST NOT share memory with `UBSCore`. It communicates *only* via Database (for state) and Pipeline (for commands).
2.  **No Hot Keys**: The Sentinel is "Watch-Only". It processes Deposits. It does *not* sign Withdrawals (that is a separate Signing Service/Air-gapped Module).

## 5. Next Steps
1.  **Docker Compose**: Add `bitcoind` and `postgres` to `docker-compose.yml`.
2.  **Sentinel Skeleton**: Create `src/sentinel/` module.
3.  **RPC Client**: Implement `BitcoinRpcClient` using `reqwest` (avoid heavy crates if possible, or use `bitcoincore-rpc`).
