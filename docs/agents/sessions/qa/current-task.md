# Current Task: Phase 0x11 Test Planning

**Objective**: Prepare Test Plan and Automation for Deposit/Withdraw.

## Status
*   **Design**: COMPLETE.
*   **Dev**: STARTING.

## Action Items
1.  **Review Design**: `docs/src/0x11-deposit-withdraw.md`.
2.  **Review Checklist**: `docs/src/0x11-acceptance-checklist.md`.
    *   Create Test Cases for each item in the checklist.
    *   Focus on: **Idempotency** (Double Spend) and **Race Conditions**.
3.  **Prepare Chaos Tests**:
    *   How will we simulate a "Re-org"? (Mock Chain API requirement?)

**Waiting for Developer to implement `POST /internal/mock/deposit` endpoint.**
