# Security Current Task: Phase 0x11 Audit

**Role**: Security Engineer
**Status**: Reviewing Design

## 🎯 Current Focus: Funding Layer Risks

### 1. The "Double Spend" Threat
*   **Requirement**: Verify that the Developer implements *Strict Idempotency* using `tx_hash` Unique Key.
*   **Action Plan**: Review PR for `src/funding/service.rs`.

### 2. The "Fake Deposit" Threat
*   **Requirement**: Ensure `POST /internal/mock/deposit` does NOT exist in Production release build, OR is protected by Admin Auth.
*   **Constraint**: Mock endpoints must be gated.

### 3. Address Isolation
*   **Requirement**: User A can never sweep funds from User B's address.

---
*Synced with Architect on 2025-12-28*
