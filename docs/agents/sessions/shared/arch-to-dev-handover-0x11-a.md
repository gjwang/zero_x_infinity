# Architect to Developer Handover: Phase 0x11-a

| Phase | 0x11-a Real Chain Integration |
| :--- | :--- |
| **Priority** | High (Architecture Upgrade) |
| **Status** | **ARCHITECTURE APPROVED** |

## 1. Objective
Transition the Funding system from a "Push" model to a "Pull" model. Implement the **Sentinel Service** to scan real blockchain nodes (BTC/ETH).

## 2. Core Constraints (Strict Adherence)
1.  **Pull-Only**: All deposits MUST originate from the `Sentinel` scanning the chain. API-based "Mock" deposits are deprecated.
2.  **No Hardcoding**: All thresholds (`REQUIRED_CONFIRMATIONS`, `MAX_REORG_DEPTH`) and asset metadata MUST be loaded from per-chain configuration.
3.  **Pipeline Integration**: Sentinel MUST use `OrderAction::Deposit` to inject finalized deposits into the Ring Buffer.

## 3. Key Components
### 3.1 `ChainScanner` Trait
- Located in `src/funding/chain_adapter.rs`.
- Implement `BtcRpcScanner` and `EthRpcScanner` using `reqwest`.
- MUST support `verify_canonical` for re-org detection.

### 3.2 `Sentinel` Service
- Located in `src/funding/sentinel.rs`.
- Loop: `get_latest_block_number` -> Check `chain_cursor` -> Fetch Block -> Verify Parent Hash.
- **Atomic Operation**: Updating `chain_cursor` and inserting `deposit_history` MUST happen in one SQL transaction.

### 3.3 Re-org Protocol
- **Shallow Re-org**: If `block.parent_hash != last_scanned_hash`, roll back cursor and restart scan.
- **Deep Re-org**: If depth > `MAX_REORG_DEPTH`, stop scan and log a **CRITICAL** error for manual intervention.

## 4. Infrastructure
- Add `bitcoind` (Regtest) and `anvil` (Anvil) to `docker-compose.yml`.
- Apply migration `0x11_a_real_chain.sql` for `chain_cursor` tracking.

## 5. References
- [ADR-003: Real Chain Sentinel](file:///Users/gjwang/eclipse-workspace/rust_source/zero_x_infinity_arch_design/docs/src/architecture/decisions/ADR-003-real-chain-sentinel.md)
- [0x11-a Real Chain Design](file:///Users/gjwang/eclipse-workspace/rust_source/zero_x_infinity_arch_design/docs/src/0x11-a-real-chain.md)
