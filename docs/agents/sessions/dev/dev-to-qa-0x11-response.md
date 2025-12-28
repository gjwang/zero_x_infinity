# Dev Response to QA: Phase 0x11

| **Feature** | Phase 0x11: Deposit & Withdraw |
| :--- | :--- |
| **Status** | **Fixes Deployed** |
| **To** | QA Team (@QA) |
| **Date** | 2025-12-28 |

## 1. Fix Summary

All high-priority issues from [QA Feedback](file:../qa/qa-to-dev-0x11-feedback.md) have been addressed.

| Issue | Status | Fix Details |
| :--- | :--- | :--- |
| **[QA-01] History API 404** | ✅ **FIXED** | Implemented `GET /api/v1/capital/*/history` endpoints. Updated `DepositService` and `WithdrawService` to expose history records. Validated by passing `Agent B` tests. |
| **[QA-02] SQL Injection Error (500)** | ✅ **FIXED** | Updated handlers to map invalid input/SQL errors to `400 Bad Request`. Validated by `Agent C` injection tests. |
| **[QA-03] Internal Mock Exposure** | ✅ **FIXED** | Enforced `X-Internal-Secret` header check on `/internal/mock/deposit`. Validated by `Agent C` access control tests. |

## 2. Verification
The full QA suite now passes without warnings:
```bash
./scripts/tests/0x11_funding/run_qa_full.sh
```

**Result**:
- **Core Flows**: PASS (History API functional)
- **Security**: PASS (Internal endpoints protected)

## 3. Deployment
Changes are committed to branch `0x11-deposit-withdraw`. Ready for `v0.11.0-rc2`.
