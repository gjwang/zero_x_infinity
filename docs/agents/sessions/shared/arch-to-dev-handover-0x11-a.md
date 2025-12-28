# Architect to Developer/DevOps Handover: Phase 0x11-a

| Phase | 0x11-a Real Chain Integration |
| :--- | :--- |
| **Priority** | High (Architecture Upgrade) |
| **Status** | Ready for Implementation |

## 1. Objective
Upgrade the Funding System from Mock to **Real Chain Integration** using a "Sentinel" service and local Dockerized nodes.

## 2. Infrastructure (DevOps Task)
**Target File**: `docker-compose.yml`

You must add the following services:

### Bitcoin (Regtest)
```yaml
  bitcoind:
    image: ruimarinho/bitcoin-core:24
    command: 
      -printtoconsole 
      -regtest=1 
      -rpcbind=0.0.0.0 
      -rpcallowip=0.0.0.0/0 
      -rpcuser=admin 
      -rpcpassword=admin
    ports:
      - "18443:18443" # RPC
```

### Ethereum (Anvil)
```yaml
  anvil:
    image: ghcr.io/foundry-rs/foundry:latest
    command: anvil --host 0.0.0.0
    ports:
      - "8545:8545"
```

## 3. Database Schema (Developer Task)
Implement the schema defined in [`docs/src/0x11-a-real-chain.md`](../../../src/0x11-a-real-chain.md) Section 6.1.
*   New Table: `chain_cursor`
*   Alter Table: `deposit_history` (add `chain_id`, `confirmations`, `block_hash`)

## 4. Sentinel Implementation (Developer Task)
Create a new module `src/sentinel/`.
*   **Trait**: `ChainScanner`
*   **Impl**: `BtcScanner` (using `reqwest` for JSON-RPC)
*   **Impl**: `EthScanner` (using `alloy` crate)
*   **Worker**: `SentinelWorker` loop (Poll -> Check Re-org -> Update DB -> Push Pipeline).

## 5. Success Criteria
1.  **Re-org Test**:
    *   Script: Mine 10 blocks -> Deposit -> Mine 1 block -> Invalidate 2 blocks (Re-org).
    *   Result: System detects Re-org, Rolls back confirmation count.
2.  **Deposit Flow**:
    *   `bitcoin-cli sendtoaddress ...` -> System detects -> Status `CONFIRMING` -> Status `SUCCESS` -> Balance Updated.
