# Fix Requirements: Phase 0x11 (Deposit & Withdraw)

**Date**: 2025-12-28
**Source**: QA Verification Cycle 1
**Priority**: High (Must Fix for Production Readiness)

The following issues were identified during the multi-agent QA verification. While the core flows pass, these defects affect usability, stability, and security monitoring.

## 1. [QA-01] Missing History API Endpoints (404)
**Severity**: **Medium** (Feature Gap)
**Description**: The frontend and users require transaction history. The documented endpoints for deposit/withdrawal history are returning `404 Not Found`.

- **Endpoints Affected**:
    - `GET /api/v1/capital/deposit/history`
    - `GET /api/v1/capital/withdraw/history`
- **Reproduction**:
    ```bash
    curl -H "Authorization: Bearer <JWT>" "http://localhost:8080/api/v1/capital/deposit/history?asset=BTC"
    # Returns: 404
    ```
- **Expected Behavior**: Return JSON list of transactions (empty list `[]` if none), status `200 OK`.
- **Reference**: `scripts/tests/0x11_funding/test_deposit_withdraw_core.py` (Line 79)

## 2. [QA-02] Unhandled Error (HTTP 500) on Invalid Input
**Severity**: **Low** (Stability/UX)
**Description**: Sending a SQL injection payload or malformed address causes the server to panic or return a generic `500 Internal Server Error`. While this prevents the injection (Safe), it exposes potential panic logs and indicates fragile validation logic.

- **Reproduction**:
    ```bash
    POST /api/v1/capital/withdraw/apply
    Payload: {"address": "addr'; DROP TABLE withdrawals; --"}
    # Returns: 500 Internal Server Error
    ```
- **Expected Behavior**: Rigid validation (Regex/Length) should catch invalid addresses *before* DB interaction and return `400 Bad Request` with `msg: "Invalid address format"`.
- **Reference**: `scripts/tests/0x11_funding/test_funding_security.py` (Line 115)

## 3. [QA-03] Internal Mock Endpoint Exposure
**Severity**: **Low** (Configuration)
**Description**: The `/internal/mock/deposit` endpoint is accessible via the main Gateway port without strict firewalling or "Internal-Only" checks in the logic.
- **Risk**: If deployed as-is, external attackers could try to credit themselves if the network firewall configuration fails.
- **Recommendation**:
    - Enforce a specific `X-Internal-Secret` header for all `/internal/*` routes.
    - OR bind internal routes to a different port (e.g., 8081) not exposed to the public load balancer.

## 4. [QA-04] Mock Deposit vs Balance Timing
**Severity**: **Low** (Testability)
**Description**: In `test_funding_idempotency.py`, mock deposits sometimes fail to credit the `available` balance immediately, causing "Race Condition" tests to fail (0 approved) because user has no funds.
- **Requirement**: Ensure the `Deposit` FSM state transition to `SUCCESS` is synchronous or provides a callback mechanism for tests to know when funds are usable.

---
**Action Item**:
Please review and assign these tickets to the Development Team for Phase 0x11.1 Patch.
