# QA to Dev Handover: Phase 0x11 (Deposit & Withdraw)

**Date**: 2025-12-28
**To**: Development Team (@Dev)
**From**: QA Team (@QA)
**Status**: 🟢 **PASSED (RELEASE READY)**

## 1. Overview
The Phase 0x11 Release Candidate has passed Independent Verification for:
- [x] Deposit & Withdraw Core Flows
- [x] Idempotency (Double Spend Protection)
- [x] Security (Address Isolation, Sanitization)

However, **4 non-blocking but important fixes** are requested before the next patch.

## 2. Artifacts
- **Full QA Report**: [`docs/src/qa/0x11_funding/report_v1.md`](../../../../docs/src/qa/0x11_funding/report_v1.md)
- **Fix Requirements**: [`docs/src/qa/0x11_funding/fix_requests_v1.md`](../../../../docs/src/qa/0x11_funding/fix_requests_v1.md)
- **Verification Suite**: `scripts/tests/0x11_funding/run_qa_full.sh`

## 3. High-Priority Fixes (See Fix Requirements for details)
1.  **[QA-01] History API 404**: `GET /api/v1/capital/*/history` is missing. Please implement or expose this for Frontend.
2.  **[QA-02] SQL Injection Error Handling**: Ensure invalid inputs return `400 Bad Request` instead of `500 Internal Error`.
3.  **[QA-03] Mock Endpoint Isolation**: Secure `/internal/mock` against public access.

## 4. Next Steps
- **QA**: Signed off for 0.11.0-rc1 deployment.
- **Dev**: Please acknowledge receiving the Fix Requirements and schedule for 0.11.1 Patch.

---
*Verified by Agentic QA System*
