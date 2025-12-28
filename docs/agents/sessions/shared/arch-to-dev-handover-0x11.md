# Architect to Developer Handover: Phase 0x11

| **Phase** | 0x11 Deposit & Withdraw |
| :--- | :--- |
| **Priority** | P1 (Critical features) |
| **Status** | Ready for Implementation |

## 1. Core Documentation
*   **Design Spec**: [`docs/src/0x11-deposit-withdraw.md`](../../../src/0x11-deposit-withdraw.md)
    *   *Includes*: Mock Chain Strategy (BTC/ETH), DB Schema, Sequence Diagrams.
*   **Definition of Done**: [`docs/src/0x11-acceptance-checklist.md`](../../../src/0x11-acceptance-checklist.md)
    *   **WARNING**: User/PM has requested **STRICT** adherence to this checklist. Do not cut corners on Idempotency or Race Conditions.

## 2. Key Technical Decisions
1.  **Mock Chain Strategy**: Do NOT integrate real blockchains yet. Implement `MockChain` trait.
    *   Simulate latency (`sleep(2s)`).
    *   Simulate confirmations (requires background polling).
2.  **BTC Support**: You must handle `Base58` (Mock) addresses alongside `Hex` addresses.
3.  **Database**:
    *   Use `tx_hash` as Idempotency Key for Deposits.
    *   Use Optimistic Locking or Atomic Updates for Balance Crediting.

## 3. Immediate Next Steps
1.  Create `src/funding/` module structure.
2.  Define `MockChain` trait in `src/funding/chain_adapter.rs`.
3.  Implement `POST /internal/mock/deposit` to unblock QA early testing.
