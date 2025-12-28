# Current Task: Phase 0x11 Implementation

**Objective**: Implement Deposit & Withdraw System with Mock Chain Support.

## Core Tasks
1.  **Scaffold Module**: `src/funding/`
2.  **Database Migration**: `deposit_history`, `withdraw_history`, `user_addresses`.
3.  **Mock Chain Adapter**:
    -   Trait `ChainClient`.
    -   Impl `MockBtcChain`, `MockEvmChain`.
4.  **Service Logic**:
    -   `FundingService::process_deposit()` (Idempotent!).
    -   `FundingService::process_withdraw()` (Atomic Check!).

## Resources
- Design: `docs/src/0x11-deposit-withdraw.md`
- Checklist: `docs/src/0x11-acceptance-checklist.md` (MUST READ)
