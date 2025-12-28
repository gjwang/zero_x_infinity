# Architect to QA Handover: Phase 0x11-a

| Phase | 0x11-a Real Chain Integration |
| :--- | :--- |
| **Priority** | High (Financial Safety) |
| **Status** | **DESIGN READY FOR VERIFICATION** |

## 1. Verification Objective
Ensure the system accurately detects deposits from real blockchain nodes (local bitcoind/anvil) and correctly handles edge cases like shallow re-orgs and node lag.

## 2. Key Verification Scenarios

### 2.1 Deposit Detection & Lifecycle
1.  **Detection**: Verify that a transaction sent via `bitcoin-cli` or `cast` is detected by the Sentinel.
2.  **Confirmation State**: Verify the status transitions: `DETECTED` (0) -> `CONFIRMING` (1 to N-1) -> `SUCCESS` (>= N).
3.  **Pipeline Integration**: Ensure that once `SUCCESS` is reached, the user's balance in `UBSCore` is updated correctly.

### 2.2 Re-org Handling (Shallow)
1.  **Setup**: Mine block N and detect a deposit.
2.  **Trigger**: Use `bitcoin-cli invalidateblock` or Anvil's `anvil_reset` to simulate a fork.
3.  **Result**: The Sentinel must detect the `parent_hash` mismatch, roll back the `chain_cursor` to N-1, and the orphaned deposit must NOT be credited.

### 2.3 System Resilience
1.  **Node Lag**: Simulate node lag. Sentinel must not "skip" blocks and must catch up correctly.
2.  **Node Offline**: Disconnect the node. Sentinel must retry gracefully and resume from the last known cursor height when the node is back.

### 2.4 Precision Audit
1.  Verify that Satoshi/Wei units are correctly scaled to the internal precision (10^6 or 10^18) without floating-point artifacts.

## 3. Tooling for QA
- **BTC**: `bitcoin-cli` (Regtest).
- **ETH**: `cast` (foundry) and Anvil RPC.
- **DB**: Inspection of `chain_cursor` and `deposit_history` tables.

## 4. References
- [0x11-a Real Chain Design](file:///Users/gjwang/eclipse-workspace/rust_source/zero_x_infinity_arch_design/docs/src/0x11-a-real-chain.md)
- [ADR-003: Real Chain Sentinel](file:///Users/gjwang/eclipse-workspace/rust_source/zero_x_infinity_arch_design/docs/src/architecture/decisions/ADR-003-real-chain-sentinel.md)
