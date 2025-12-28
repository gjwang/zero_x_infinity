# QA Test Plan: Phase 0x11 Funding

**Role**: QA Engineer
**Context**: Verifying a critical financial module (Money In/Out).
**Risk Level**: **EXTREME** (Double Spend = Loss of Funds).

## 1. Verification Basis
*   **Strict Checklist**: [`docs/src/0x11-acceptance-checklist.md`](../../../src/0x11-acceptance-checklist.md)
    *   You MUST NOT pass the release unless *every* item is checked.

## 2. Test Strategy Focus
### 2.1 "The Double Spend" (Idempotency)
*   **Scenario**: Developer sends the same `tx_hash` 20 times in parallel.
*   **Expected**:
    *   Database: 1 row.
    *   User Balance: Credited 1 time.
    *   API Response: 200 OK (all look successful to client, but backend filters duplicates).

### 2.2 "The Race" (Concurrency)
*   **Scenario**: User has 100 USDT. Sends 3 requests to withdraw 50 USDT simultaneously.
*   **Expected**:
    *   2 Requests succeed.
    *   3rd Request fails (Insufficient Balance).
    *   User Balance = 0 USDT.

### 2.3 "The Mock Chain"
*   **Scenario**: Request BTC Address.
*   **Expected**: Returns Base58 format (not Hex).

## 3. Automation Requirements
*   Create `scripts/test_funding_concurrency.py` using `asyncio` to flood endpoint.
*   Verify results against `GET /api/v1/private/balances`.
