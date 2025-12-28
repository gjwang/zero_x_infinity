# Dev to Arch: Phase 0x11-a QA Verification Complete

| **Milestone** | Phase 0x11-a: Real Chain Integration (QA Verification) |
| :--- | :--- |
| **Status** | 🟢 **12/12 TESTS PASS - READY FOR MERGE** |
| **From** | QA Team (@QA) via Development Team (@Dev) |
| **To** | Technical Architect (@Arch) |
| **Date** | 2025-12-29 |

## 1. Executive Summary

Phase 0x11-a (Deposit/Withdraw + Real Chain Sentinel) has completed independent QA verification.
All 12 core, chaos, and security tests have passed in a clean test environment.

**Key Verified Fixes:**
- **DEF-001**: Gateway now generates cryptographically valid `bcrt1...` P2WPKH addresses.
- **QA-01**: History API (`/capital/deposit/history`, `/capital/withdraw/history`) now correctly returns data.
- **QA-02**: Business logic errors (e.g., insufficient balance) now return 400 Bad Request instead of 500.
- **QA-03**: Internal mock endpoint (`/internal/mock`) is protected by `X-Internal-Secret` header.

## 2. Verification Results

| Persona | Test Suite | Objective | Result |
| :--- | :--- | :--- | :---: |
| **Agent B (Core)** | `test_deposit_withdraw_core.py` | Address Persistence, Deposit Lifecycle, Withdraw Flow | ✅ PASS |
| **Agent A (Chaos)** | `test_funding_idempotency.py` | Deposit Replay Attack (9/10 blocked), Withdrawal Bank Run | ✅ PASS |
| **Agent C (Security)** | `test_funding_security.py` | Cross-User Isolation, Internal Endpoint Auth, Input Sanitization | ✅ PASS |

## 3. Integer-Only Persistence Audit

All monetary columns have been verified to use `BIGINT` in PostgreSQL:
- `balances_tb.available`, `balances_tb.frozen`
- `deposit_history.amount`
- `withdraw_history.amount`, `withdraw_history.fee`
- `transfers_tb.amount`

Rust services (`DepositService`, `WithdrawService`, `TransferService`) correctly scale `Decimal` inputs to `i64` before database operations.

**CI Guardrail**: The `schema-lint` job in `basic-checks.yml` successfully detects and blocks any future `DECIMAL`/`FLOAT` introductions.

## 4. Outstanding Items (DEF-002)

> [!CAUTION]
> **Sentinel Blindness**: The Sentinel service currently fails to detect real BTC deposits to P2WPKH (SegWit) addresses.
> 
> This does **not** affect the Mock Chain flow (used in current E2E tests), but will require a fix before connecting to a real Bitcoin node.
> 
> **Recommendation**: Address DEF-002 in a follow-up Phase 0x11-b before production deployment.

## 5. Artifacts

- **Verification Script**: [run_0x11a_verification.sh](file:///Users/gjwang/eclipse-workspace/rust_source/zero_x_infinity_test/scripts/run_0x11a_verification.sh)
- **Full Log**: [qa_verification_0x11a.log](file:///Users/gjwang/eclipse-workspace/rust_source/zero_x_infinity_test/scripts/qa_verification_0x11a.log)
- **Walkthrough**: [walkthrough.md](file:///Users/gjwang/.gemini/antigravity/brain/ddf6b501-5d5c-4cda-8e17-fafeb207e18c/walkthrough.md)
- **Branch**: `0x11-a-real-chain` (1 commit ahead of `origin`)

## 6. Recommendation to Architect

**Merge Approved from QA Perspective.** Suggest:
1. Merge `0x11-a-real-chain` into `main`.
2. Tag as `v0.11-a-funding-qa`.
3. Open follow-up ticket for DEF-002 (Sentinel SegWit Parsing).
