# QA → Developer Handover: Blocking Issues

> **Date**: 2025-12-26 01:14  
> **From**: QA Engineer  
> **To**: Developer Team  
> **Priority**: P0 Critical Issues Identified

---

## 🔴 P0 - CRITICAL (Blocking Production Release)

### Issue #1: Transfer Idempotency NOT Implemented
**Component**: Internal Transfer (0x0B)  
**Severity**: 🔥 **CRITICAL** - Double-spend risk

**Problem**:
- API accepts same `cid` (client ID) multiple times
- Each request creates different `transfer_id`
- Funds are deducted **twice** for duplicate requests

**Evidence**:
```
Test: TC-P0-07 (Idempotency)
First request:  cid=client-idempotency-test-001 → transfer_id=01KDAZEZCAP9QWHPKRZG3BGYM9
Second request: cid=client-idempotency-test-001 → transfer_id=01KDAZF005AK1MYDJSQ6K7E2TP
Balance: 975 → 955 → 935 USDT (deducted twice!)
```

**Expected Behavior** (per `0x0B-a-transfer.md` Section 1.5.7):
> If `cid` provided, check if exists. Return `DUPLICATE_REQUEST` with original result.

**Impact**:
- User retry mechanisms cause double transfers
- Replay attack vector
- Financial integrity violation

**Recommended Fix**:
```sql
-- Add UNIQUE constraint
ALTER TABLE fsm_transfers_tb ADD CONSTRAINT unique_user_cid 
  UNIQUE (user_id, cid);

-- In transfer creation logic
IF EXISTS (SELECT 1 FROM fsm_transfers_tb WHERE cid = $1 AND user_id = $2) THEN
    RETURN existing_transfer;
END IF;
```

**Estimated Fix Time**: 3-4 hours  
**Test Report**: `docs/agents/sessions/qa/0x0B-transfer-p0-test-report.md`

---

## ⚠️ P1 - HIGH PRIORITY

### Issue #2: Fee E2E Test Path Error
**Component**: Trade Fee System (0x0C)  
**Severity**: ⚠️ **HIGH** - Blocks E2E verification

**Problem**:
```bash
# scripts/test_fee_e2e.sh:139
python3 "${SCRIPT_DIR}/lib/inject_orders.py" ...
# Error: No such file

# Correct path:
python3 "${SCRIPT_DIR}/inject_orders.py" ...
```

**Impact**:
- Cannot verify API integration
- Cannot verify TDengine persistence
- Cannot verify WebSocket push

**Recommended Fix**:
Remove `/lib` from path in `scripts/test_fee_e2e.sh` line 139

**Estimated Fix Time**: 5 minutes  
**Test Report**: `docs/agents/sessions/qa/0x0C-fee-test-report.md`

---

## ⏸️ P2 - PENDING IMPLEMENTATION

### Issue #3: 0x0D Snapshot/Recovery Not Implemented
**Component**: WAL & Snapshot (0x0D)  
**Severity**: ⏸️ **MEDIUM** - Feature incomplete

**Status**:
- ✅ Task 1.1 WAL Writer: Complete (11/11 tests pass)
- ❌ Task 1.2 Snapshot: Not implemented
- ❌ Task 1.3 Recovery: Not implemented

**Impact**:
- Cannot test snapshot creation
- Cannot test crash recovery
- Phase 0x0D incomplete

**Next Steps**:
1. Implement `src/ubscore_wal/snapshot.rs`
2. Implement `src/ubscore_wal/recovery.rs`
3. Add test functions: `test_snapshot_*`, `test_recovery_*`
4. Notify QA when ready for testing

**Test Report**: `docs/agents/sessions/qa/0x0D-retest-report.md`

---

## 📊 QA Sign-Off Summary

| Component | Status | Production Ready? | Blocker |
|-----------|--------|------------------|---------|
| 0x0D WAL | ✅ Pass | ✅ Yes | None |
| 0x0D Snapshot | ❌ Missing | ❌ No | P2: Not implemented |
| 0x0B Transfer | ⚠️ 80% | ❌ No | **P0: Idempotency bug** |
| 0x0C Fee Core | ✅ Pass | ✅ Yes | None |
| 0x0C Fee E2E | ❌ Blocked | ⚠️ Pending | P1: Test script fix |

**Overall Production Readiness**: ❌ **BLOCKED** by P0 issue

---

## 📋 All QA Test Reports

Located in: `docs/agents/sessions/qa/`

**0x0D (WAL & Snapshot)**:
- `0x0D-test-checklist.md` - Test acceptance checklist
- `0x0D-phase1-test-report.md` - Initial test (12-25)
- `0x0D-retest-report.md` - Re-test (12-26)

**0x0B (Transfer)**:
- `0x0B-transfer-coverage-review.md` - Coverage analysis
- `0x0B-transfer-p0-test-plan.md` - P0 test specifications  
- `0x0B-transfer-p0-test-report.md` - Final test results

**0x0C (Fee)**:
- `0x0C-trade-fee-test-checklist.md` - Test checklist
- `0x0C-fee-test-plan.md` - Test execution plan
- `0x0C-fee-test-report.md` - Final test results

---

## 🔄 Recommended Action Priority

1. **Immediate** (This sprint):
   - 🔥 Fix Transfer idempotency (P0 - 3-4 hours)
   - ⚠️ Fix Fee E2E script path (P1 - 5 minutes)
   - ✅ Re-test both features

2. **Next Sprint**:
   - ⏸️ Implement 0x0D Snapshot/Recovery
   - 📋 Implement remaining 32 E2E tests from Fee checklist

---

*QA Handover Generated: 2025-12-26 01:14*  
*Contact: QA Engineer AI Agent*
