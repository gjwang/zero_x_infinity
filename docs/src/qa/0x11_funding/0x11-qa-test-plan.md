
# 0x11 QA Strategy: Multi-Agent Consensus

> **Editor's Note (Agent Leader)**: This plan consolidates inputs from our Radical, Conservative, and Security specialists to ensure comprehensive coverage of Phase 0x11 (Deposit & Withdraw).

## 1. Core Stability (Agent B - Conservative)
**Focus**: "Does the happy path work perfectly every time?"
*   [ ] **Deposit Flow**:
    *   Generate address -> Deposit -> 6 Confirmations -> Balance Update.
    *   Verify Address Persistence (User always gets same address).
    *   Verify History API returns accurate records.
*   [ ] **Withdraw Flow**:
    *   Apply Withdraw -> Balance Frozen -> Broadcast -> Success.
    *   Verify "Insufficient Funds" rejection.
*   [ ] **Artifact**: `scripts/test_deposit_withdraw_core.py` (Merged happy path).

## 2. Edge Cases (Agent A - Radical)
**Focus**: "Breaking the system with race conditions and lies."
*   [ ] **The "Double-Spend" Assault**:
    *   Send same `tx_hash` 10 times concurrently.
    *   **Goal**: Ensure balance credits EXACTLY ONCE.
*   [ ] **The "Bank Run" Simulation**:
    *   User has 100 USDT.
    *   Send 5 concurrent withdrawal requests for 100 USDT each.
    *   **Goal**: 1 Success, 4 Failures (Atomic Balance Check).
*   [ ] **Decimal Dust**:
    *   Deposit `0.00000001` BTC. Withdraw `0.00000001` BTC.
    *   **Goal**: No rounding errors to 0. Precision checks.
*   [ ] **Artifact**: `scripts/test_funding_idempotency.py`.

## 3. Security (Agent C - Security Expert)
**Focus**: "Unauthorized access and data leakage."
*   [ ] **Address Isolation**:
    *   Try to view User B's deposit history as User A.
    *   Try to withdraw funds from User B's account.
*   [ ] **Fake Deposit Injection**:
    *   Call Public API pretending to be the "Scanner" (without internal auth).
    *   **Goal**: HTTP 401/403.
*   [ ] **Withdrawal Sanity**:
    *   Attempt to withdraw negative amounts.
    *   Attempt to withdraw `NaN` or `Infinity`.
*   [ ] **Artifact**: `scripts/test_funding_security.py`.

## 4. Execution Plan (Consolidated)

| Agent | Script | Purpose |
| :--- | :--- | :--- |
| **B (Core)** | `test_deposit_withdraw_core.py` | Implementation Integrity |
| **A (Radical)** | `test_funding_idempotency.py` | Concurrency & Data Safety |
| **C (Sec)** | `test_funding_security.py` | Access Control & Validation |

Run all via `verify_0x11_release.sh`.
