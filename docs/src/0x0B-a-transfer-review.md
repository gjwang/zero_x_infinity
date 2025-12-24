# Internal Transfer FSM - Code Review Report

**Date**: 2024-12-24  
**Reviewer**: Claude AI  
**Version**: Pre-Production Review

---

## Summary

| Severity | Count | Status |
|----------|-------|--------|
| 🔴 Critical | 1 | **Must Fix** |
| 🟡 Medium | 2 | Should Fix |
| 🟢 Low | 2 | Nice to Have |

---

## 🔴 Critical Issues

### 1. TradingAdapter.rollback() Does Not Actually Rollback

**File**: `src/transfer/adapters/trading.rs:261-283`

**Problem**: The rollback method returns `OpResult::Success` without restoring funds.

```rust
async fn rollback(&self, req_id: RequestId) -> OpResult {
    // TODO: In production, send refund Deposit order to UBSCore
    warn!(req_id, "Trading rollback - would need transfer details from DB");
    OpResult::Success  // ⚠️ DOES NOTHING BUT CLAIMS SUCCESS
}
```

**Impact**: Fund loss when source=Trading and target deposit fails.

**Fix**: Query transfer details from `fsm_transfers_tb`, send Deposit order to UBSCore.

---

## 🟡 Medium Issues

### 2. Amount Precision Loss

**File**: `src/transfer/db.rs:220-224`

```rust
let amount_u64 = amount.trunc().to_i64().unwrap_or(0) as u64;
```

**Fix**: Add assertion `assert_eq!(amount.fract(), Decimal::ZERO)` or handle decimals.

### 3. Snowflake Generator Race

**File**: `src/transfer/coordinator.rs:33-48`

- Sequence can overflow beyond 32K/ms
- No clock skew handling

**Fix**: Use `ulid` or `uuid` crate.

---

## 🟢 Low Issues

### 4. FundingAdapter.deposit() No Explicit Transaction
### 5. Warning in Production When No Channel

---

## ✅ Verified Correct

- CAS state updates: `WHERE state = $expected`
- Persist-before-call pattern in `step_init()`, `step_source_done()`
- Asymmetric rollback: Trading source → infinite retry (never compensate)
- FundingAdapter idempotency via `transfer_operations_tb`
- SELECT FOR UPDATE in withdraw

---

## Action Items

| Priority | Task | Owner |
|----------|------|-------|
| P0 | Fix TradingAdapter.rollback() | Dev |
| P1 | Add precision assertion | Dev |
| P1 | Replace custom Snowflake | Dev |
| P2 | Add transaction to deposit | Dev |
