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
| P1 | Refactor ID system (see below) | Dev |
| P2 | Add transaction to deposit | Dev |

---

## ID System Refactoring Requirements

### Background

Current `RequestId` is a simple `type RequestId = u64` alias using a custom Snowflake generator with race condition issues.

### Requirements

1. **Use ULID instead of custom Snowflake**
   - Add `ulid` crate to dependencies
   - ULID provides: monotonic, sortable, no coordination needed, 128-bit

2. **Create proper struct wrappers for IDs**
   - Each important ID type should be a newtype struct
   - This allows easy internal implementation swap without API changes

### Required ID Structs

```rust
/// Transfer Request ID (ULID-based)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TransferRequestId(ulid::Ulid);

impl TransferRequestId {
    pub fn new() -> Self {
        Self(ulid::Ulid::new())
    }
    
    pub fn to_string(&self) -> String {
        self.0.to_string()
    }
    
    pub fn from_string(s: &str) -> Result<Self, ulid::DecodeError> {
        Ok(Self(ulid::Ulid::from_string(s)?))
    }
}

impl std::fmt::Display for TransferRequestId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
```

### Other ID Types to Consider

| ID Type | Current | Recommended |
|---------|---------|-------------|
| `RequestId` (transfer) | `u64` | `TransferRequestId(Ulid)` |
| `OrderId` | `u64` | `OrderId(u64)` (keep u64 for performance) |
| `TradeId` | `u64` | `TradeId(u64)` |
| `UserId` | `u64` | `UserId(u64)` |
| `AssetId` | `u32` | `AssetId(u32)` |

### Benefits

1. **Type Safety**: Compiler prevents mixing different ID types
2. **Easy Swap**: Change internal representation without API changes
3. **Self-Documenting**: Clear what each function expects
4. **No Coordination**: ULID doesn't need machine_id coordination

