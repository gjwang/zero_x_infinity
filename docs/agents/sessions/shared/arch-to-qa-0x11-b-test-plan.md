# Handover: Architect -> QA (Phase 0x11-b)

**Date**: 2025-12-29
**Phase**: 0x11-b (Sentinel Hardening & ETH Support)
**Topic**: Test Strategy for SegWit Fix and ETH Integration

## 1. Context
The Dev team is implementing two major changes:
1.  **Fix for DEF-002**: BTC Sentinel will now detect SegWit (P2WPKH) deposits.
2.  **New Feature**: ETH Sentinel will now detect ERC20 Token Transfers.

## 2. Testing Directives

### 2.1 Directive A: BTC SegWit Verification (Regression + New)
*   **Target**: `BtcSentinel` behavior with `bcrt1...` addresses.
*   **Test Case 1 (Legacy Protocol)**:
    *   Deposit 1 BTC to `1A1z...` (P2PKH).
    *   **Expect**: Detected (No Regression).
*   **Test Case 2 (SegWit Protocol) - CRITICAL**:
    *   Deposit 1 BTC to `bcrt1...` (P2WPKH).
    *   **Expect**: Status `DETECTED` -> `CONFIRMING` -> `FINALIZED`.
    *   **Failure Condition**: Deposit remains invisible (DEF-002 not fixed).

### 2.2 Directive B: ETH / ERC20 Verification
*   **Target**: `EthScanner` Log Parsing.
*   **Test Case 3 (ERC20 Deposit)**:
    *   Trigger `transfer(to=user_addr, amount=100)` on MockUSDT.
    *   **Expect**: `DetectedDeposit` created with correct Decimal precision.
    *   **Note**: Ensure `anvil` is producing blocks or events are emitted.

### 2.3 Directive C: Chaos Testing (Re-org)
*   **Target**: Both Chains.
*   **Scenario**:
    *   Sentinel scans Block N.
    *   Invalidate Block N (Re-org).
    *   Generate new Block N' (different hash).
    *   **Expect**: Sentinel detects hash mismatch, rolls back cursor, and re-scans.

## 3. Success Metric
Phase 0x11-b is complete when:
1.  `run_0x11a_verification.sh` passes (Core Logic).
2.  New `TC-B02` (ETH) passes.
3.  DEF-002 is marked CLOSED in `docs/qa/0x11a_real_chain/test_report.md`.
