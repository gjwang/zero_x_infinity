# UBSCore Persistence Critical Audit Report

> **Severity**: 🔴 **CRITICAL** (Production Blocking)  
> **Date**: 2025-12-26  
> **Auditor**: AI QA Engineer  
> **Status**: ⚠️ **RE-VERIFICATION: STILL FAILING**

---

## TL;DR

UBSCore is the **Single Source of Truth for Balances**. 

### 🔴 RE-VERIFICATION (Post Developer Fix)

| Finding | Before Fix | After Fix | Status |
|---------|------------|-----------|--------|
| `ubscore_persistence` config | ❌ Missing | ✅ Added | FIXED |
| UBSCore WAL at runtime | ❌ None | ❌ **Still None** | 🔴 FAIL |
| Persistence LOG | ❌ No log | ❌ **No log** | 🔴 FAIL |
| `./data/audit_ubscore/` | ❌ Empty | ❌ **Still Empty** | 🔴 FAIL |

### Evidence from Logs

```
[Persistence] Disabled
[ME] Persistence enabled: dir=./data/audit_ubscore_me  ✅
[Settlement] Persistence enabled: ...                   ✅
# NO "[UBSCore] Persistence enabled" LOG!               ❌
```

**Conclusion**: Config option added but **code path not wired**. UBSCore persistence initialization is NOT being called.

---

## 🔍 ROOT CAUSE (QA Finding)

| 文件 | `ubscore_persistence` 引用 | 被使用 |
|------|---------------------------|--------|
| `main.rs:324` | ✅ 存在 | 直接模式 |
| `pipeline_mt.rs` | ❌ **不存在** | **--gateway 模式** |

**问题**: 审计使用 `--gateway` 模式，走 `pipeline_mt.rs` 但该文件未读取 `ubscore_persistence`。

**修复**: 在 `pipeline_mt.rs` 中添加与 `main.rs:324-329` 相同的逻辑。

---

## Identified Gaps (vs Arch Spec & Settlement)

| ID | Gap | Settlement Behavior | UBSCore Behavior | Risk |
|----|-----|---------------------|------------------|------|
| **UBSC-GAP-01** | WAL Corruption Handling | Falls back to snapshot, logs warning | **FATAL ERROR** - process refuses to start | 🔴 HIGH |
| **UBSC-GAP-02** | Zombie Snapshot | via `SettlementSnapshotter.load_latest()` | Present (via `UBSCoreSnapshotter`) | ✅ OK |
| **UBSC-GAP-03** | Order Replay | N/A (only checkpoints) | Does NOT replay Order/Cancel | ⚠️ MEDIUM |
| **UBSC-GAP-04** | Balance Lock Replay | N/A | Only Deposit replayed, Lock events SKIPPED | 🔴 HIGH |

---

## Detailed Analysis

### UBSC-GAP-01: Fatal Error on WAL Corruption

**Location**: [recovery.rs L110](file:///Users/gjwang/eclipse-workspace/rust_source/zero_x_infinity_test/src/ubscore_wal/recovery.rs#L75-L110)

```rust
// Current behavior: propagates error up, causing process panic
reader.replay(next_seq_id, |entry| { ... })?;
```

**Expected Behavior**: Fall back to snapshot and log warning (like Settlement).

**Impact**: If any single byte in WAL is corrupted, the entire node becomes unbootable until manual intervention.

---

### UBSC-GAP-04: Incomplete WAL Replay

**Location**: [recovery.rs L75-L110](file:///Users/gjwang/eclipse-workspace/rust_source/zero_x_infinity_test/src/ubscore_wal/recovery.rs#L75-L110)

```rust
match WalEntryType::try_from(entry.header.entry_type) {
    Ok(WalEntryType::Order) => {
        // ONLY tracks seq progression, does NOT replay balance lock!
        next_seq_id = entry.header.seq_id + 1;
    }
    ...
}
```

**Problem**: When an Order is replayed, the corresponding `lock()` on user balance is NOT re-applied. This causes:
- Frozen balances to be **lost** after recovery
- Orders in Matching Engine may still exist, but funds are **not locked**

**Consequence**: Potential for **over-selling** or **double-spend**.

---

## Remediation Roadmap

### Immediate (P0)
1. Add `catch_unwind` or `match` wrapper around WAL replay to convert errors to fallback
2. Log corruption warning instead of fatal error

### Short-term (P1)
1. Implement Order replay: deserialize `OrderPayload`, re-call `lock_funds()`
2. Add integration test for corrupted WAL recovery

### Medium-term (P2)
1. Ensure WAL contains all necessary info for complete balance reconstruction
2. Consider "full balance event sourcing" where every mutation is logged

---

## Recommendation

> [!CAUTION]
> **DO NOT PROMOTE TO PRODUCTION** until UBSC-GAP-01 and UBSC-GAP-04 are fixed.
> The current UBSCore recovery logic can lead to unrecoverable data loss scenarios.

---

*Verified by AI QA Auditor*
