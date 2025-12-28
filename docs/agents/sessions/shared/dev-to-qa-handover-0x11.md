# Developer to QA Handover: Phase 0x11

| **Feature** | Phase 0x11: Deposit & Withdraw |
| :--- | :--- |
| **Status** | **Ready for QA** |
| **Developer** | Antigravity Agent |
| **Date** | 2025-12-28 |

## 1. Feature Summary
Implemented the core capital flow mechanism for the exchange using a "Mock Chain" strategy.
- **Deposit**: Supports ETH/BTC mock deposits via internal trigger. Logic includes strict idempotency (anti-replay).
- **Withdraw**: Supports applying for withdrawal, balance freezing, and simulated broadcasting.
- **Security**: Strict address validation (Mock Adapters), JWT Authentication for user endpoints.

## 2. Artifacts Delivered
*   **Database Schema**: `migrations/010_deposit_withdraw.sql` (Tables: `deposit_history`, `withdraw_history`, `user_addresses`)
*   **Codebase**: `src/funding/*` (Service & Handlers)
*   **Verification Script**: `scripts/test_0x11_funding.sh` (One-Click)

## 3. How to Verify (Standard Procedure)

### 3.1 One-Click Verification (Recommended)
Run the automated E2E suite which initializes the DB and tests the full flow:
```bash
./scripts/test_0x11_funding.sh
```
**Expected Output**:
```text
✅ Phase 0x11 Verification Passed!
[TEST] ✅ One-Click Verification PASSED!
```

### 3.2 Manual Verification Steps
1.  **Start Gateway**: `./target/release/zero_x_infinity --gateway`
2.  **Get Token**: Register/Login via `/api/v1/user/*`
3.  **Generate Address**: `GET /api/v1/capital/deposit/address?asset=USDT&network=ETH`
4.  **Mock Deposit**:
    ```bash
    curl -X POST http://localhost:8080/internal/mock/deposit \
      -H "Content-Type: application/json" \
      -d '{"user_id": 1, "asset": "USDT", "amount": "100.0", "tx_hash": "0xUNIQUE_HASH"}'
    ```
5.  **Idempotency Test**: Repeat Step 4 with the **SAME** `tx_hash`.
    *   **Result**: HTTP 200 OK body: `{"code":0, "msg":"Ignored: Already Processed"}`.
6.  **Withdraw**: `POST /api/v1/capital/withdraw/apply`

## 4. Known Constraints (MVP)
*   **Mock Chain**: Address validation allows 34-char (Legacy Mock) and 42-char (ETH Std) strings. No real blockchain connection.
*   **Deposit Status**: Mock deposits transition immediately to `SUCCESS`.
*   **Withdraw Status**: Transitions to `PROCESSING` then `SUCCESS` (Simulated) or `FAILED` (if broadcast "fails").

## 5. Risk Areas Checked
*   **Double Crediting**: Verified via Idempotency Test.
*   **Negative Balance**: Verified via Withdrawal logic (Atomic `UPDATE ... SET available = available - amount`).
