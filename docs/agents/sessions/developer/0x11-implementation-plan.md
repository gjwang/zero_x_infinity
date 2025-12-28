# Phase 0x11 Implementation Plan: Deposit & Withdraw

| Status | **Ready for Dev** |
| :--- | :--- |
| **Architect** | GJ Wang |
| **Date** | 2025-12-28 |

## 1. Objective
Implement the full Deposit and Withdraw lifecycle using the **Ring Buffer Pipeline** for safe balance management.

## 2. Existing Scaffolding
The Architect has already created:
*   `src/funding/chain_adapter.rs`: Mock Chain interfaces.
*   `src/funding/service.rs`: Core logic for Address Gen and Mock Deposit.
*   `tests/test_funding.rs`: Integration test for Service logic.

## 3. Implementation Steps

### Step 1: Verification of Scaffolding
*   [ ] Run `cargo test --test test_funding` to ensure the base logic works.
*   [ ] Fix any module visibility issues (ensure `funding` mod is accessible).

### Step 2: Pipeline Wiring (The "Hard" Part)
*   **Deposit**:
    *   [ ] Verify `src/ubscore.rs` handles `OrderAction::Deposit`.
    *   [ ] Ensure `BalanceUpdate` struct correctly maps to `BalanceEvent`.
*   **Withdraw**:
    *   [ ] Define `OrderAction::WithdrawLock` in `src/pipeline.rs`.
    *   [ ] Implement `UBSCore::withdraw_lock` in `src/ubscore.rs`:
        *   Check logic: `if available >= amount { frozen += amount; available -= amount; return Ok; }`.
    *   [ ] Implement `OrderAction::WithdrawConfirm` (Burn) and `WithdrawReject` (Refund) for finalization.

### Step 3: API Gateway Integration
*   [ ] Create `src/funding/handlers.rs` (or modify `src/gateway/handlers.rs`).
*   [ ] Implement Endpoints:
    *   `POST /api/v1/capital/deposit/address` -> Calls `FundingService::get_deposit_address`.
    *   `POST /api/v1/capital/withdraw/apply` -> Calls `FundingService::submit_withdrawal` (which pushes to Pipeline).
    *   `POST /internal/mock/deposit` -> Calls `FundingService::mock_deposit`.
*   [ ] Wire `FundingService` into `AppState` in `src/main.rs`.

### Step 4: Background Worker (Withdrawal Broadcaster)
*   [ ] Implement a background loop (in `src/funding/worker.rs` or `service.rs`) that:
    1.  Polls `withdraw_history` for `PENDING` status.
    2.  Calls `ChainAdapter::broadcast_withdrawal`.
    3.  Updates DB to `BROADCASTED`.
    4.  Pushes `OrderAction::WithdrawConfirm` to Pipeline to burn the frozen funds.

## 4. Testing Plan
*   **Unit Tests**: Inside `src/ubscore.rs` for `WithdrawLock`.
*   **Integration**: Extend `tests/test_funding.rs` to include Pipeline consumption.
*   **E2E**: Use `curl` scripts to simulate full flow.

## 5. Definition of Done
*   [ ] `argo check` passes.
*   [ ] `cargo test` passes.
*   [ ] Manual E2E: User can deposit (Mock) and see balance increase. User can withdraw and see balance decrease.
