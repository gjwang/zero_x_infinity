# Implement: Phase 0x11 Deposit & Withdraw

**Role**: Senior Rust Developer
**Context**: Adding Funding Layer to High-Performance Exchange.
**Blocked By**: N/A (Design & Spec Ready).

## 1. Technical Specification (Must Follow)
*   **Architecture**: [`docs/agents/sessions/shared/arch-to-dev-handover-0x11.md`](../shared/arch-to-dev-handover-0x11.md)
    *   *Includes*: SQL Schema (`deposit_history`, `user_addresses`) & Rust Trait (`ChainClient`).
*   **Definition of Done**: [`docs/src/0x11-acceptance-checklist.md`](../../../src/0x11-acceptance-checklist.md)
    *   *Critical*: Idempotency (TxHash unique constraint) is P0.

## 2. Implementation Steps
### 2.1 Database Layer
*   Create `migrations/20251228132000_deposit_withdraw.sql`.
*   Copy Schema from Handover Doc Section 2.1 exactly.

### 2.2 Chain Adapter Module (`src/funding/chain_adapter.rs`)
*   Implement `trait ChainClient`.
*   Implement `MockBtcChain`: Address start with "1", "3", or "bc1".
*   Implement `MockEvmChain`: Address start with "0x" (len > 10).
*   *Note*: Use `uuid` for generating fake TxHashes.

### 2.3 Funding Service (`src/funding/service.rs`)
*   `get_deposit_address(user, asset)`:
    *   Check `user_addresses` table first.
    *   If missing, call `ChainClient::generate_address` and INSERT.
    *   Return address.
*   `mock_deposit_callback`:
    *   **Transactional**: Insert `deposit_history` -> Credit `UBScore`.
    *   **Idempotency**: Handle "Duplicate Key Error" gracefully (ignore).

## 3. Verification
*   **Unit Test**: Does `MockBtcChain` generate valid-looking addresses?
*   **Integration**: Can I register a user -> Get Address -> Mock Deposit -> See Balance?
