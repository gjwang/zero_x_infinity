# 0x11 Delivery Acceptance Checklist (Strict)

| Context | Phase 0x11 (Deposit & Withdraw) |
| :--- | :--- |
| **Audience** | QA, Developers, Product Manager |
| **Purpose** | **Anti-Corner-Cutting**: Define strict "Definition of Done". |

## 1. Product Manager's Checklist (User Experience & Logic)

Each item must be verified by a manual walkthrough or E2E test video.

### 1.1 Deposit (Asset Inflow)
- [ ] **Address Generation**:
    - [ ] Evaluating `GET /deposit/address` for a new user returns a **valid** address immediately.
    - [ ] Evaluating it again returns the **SAME** address (Persistence Check).
    - [ ] **BTC Support**: Requests for BTC return a Base58/Segwit format address.
    - [ ] **ETH Support**: Requests for ETH return a 0x Hex address.
- [ ] **Status Transitions**:
    - [ ] "Confirming" status is visible user-side when block depth < Required.
    - [ ] "Success" status appears automatically after block depth is reached.
    - [ ] Balance updates **exactly** when status turns "Success", not before.
- [ ] **History Accuracy**:
    - [ ] Deposit History (`GET /deposit/history`) shows correct Time, Amount, TxHash.
    - [ ] Precision: 8 decimal places for BTC are distinct and accurate (no rounding errors).

### 1.2 Withdraw (Asset Outflow)
- [ ] **Application Flow**:
    - [ ] Apply for withdrawal > User Balance is **IMMEDIATELY FROZEN/DEDUCTED**.
    - [ ] Insufficient balance > Rejects immediately with HTTP 400.
- [ ] **Status Tracking**:
    - [ ] User can see "Processing" (Broadcasting).
    - [ ] User can see "Completed" (On-chain confirmed) with valid TxHash.
- [ ] **Fees**:
    - [ ] Withdrawal Amount = Request Amount + Fee ? OR Request = Receive + Fee?
    - [ ] **Strict Rule**: `User Balance Delta = Request Amount`. `Network Receive = Request - Fee`.

---

## 2. Technical Architect's Checklist (System Integrity)

Developers must provide logs or test artifacts proving these edge cases are handled.

### 2.1 The "Double-Spend" & Idempotency Trap
- [ ] **Deposit Replay Attack**:
    - [ ] **Test**: Send the SAME mock deposit payload (same TxHash) 10 times concurrently.
    - [ ] **Requirement**: User balance credited **EXACTLY ONCE**. DB should reject duplicates.
- [ ] **Withdrawal Race Condition**:
    - [ ] **Test**: User has 100 USDT. Send 5 concurrent requests for 100 USDT.
    - [ ] **Requirement**: Only **ONE** succeeds. 4 must fail with "Insufficient Funds".
    - [ ] **Proof**: Log showing distinct Database transactions or Atomic CAS failure.

### 2.2 Mock Chain Fidelity
- [ ] **Multi-Chain Simulation**:
    - [ ] **BTC Mock**: Simulates UTXO-like behavior (optional deep check, but address format must be native).
    - [ ] **ETH Mock**: Simulates Nonce increment.
- [ ] **Re-org Simulation (Chaos Testing)**:
    - [ ] **Test**: Simulate block re-org (block N becomes invalid).
    - [ ] **Requirement**: System detects re-org OR waits for safe confirmation depth (e.g., 6 blocks) before crediting. *Note: For MVP, waiting for 6 blocks is acceptable.*

### 2.3 Database & State
- [ ] **Zero Negative Balance**:
    - [ ] DB constraint or Service logic MUST prevent negative balance under any load.
- [ ] **Audit Trail**:
    - [ ] Every balance change must link to a specific `deposit_id` or `withdraw_id`.
    - [ ] No "Mystery Money".

---

## 3. Security Checklist (Basic)
- [ ] **Address Isolation**: User A cannot see logic/history for User B's address.
- [ ] **Fake Deposit Detection**:
    - [ ] System only trusts Internal Sentinel/Scanner, **NEVER** trusts User API claims of "I deposited X".
