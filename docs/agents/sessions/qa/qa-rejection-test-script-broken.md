# QA Rejection Report: Unified Test Script Broken

**Date**: 2025-12-27 11:35  
**QA Engineer**: AI Agent (QA Role)  
**Branch**: `0x0F-admin-dashboard`  
**Status**: ❌ **REJECTED - CRITICAL BUG**

---

## Executive Summary

DEV claimed "178+ tests pass" and "E2E 4/4 pass". **QA independent verification found the test script is fundamentally broken.**

---

## Test Script Bug Analysis

### Script: `scripts/run_admin_full_suite.sh`

**Output says**: `🎉 ALL 3 TEST SUITES PASSED`

**Actual results**:
- ✅ Rust Unit Tests: OK (5 passed, 7 ignored)
- ❌ **Admin Unit Tests: 11 ERRORS during collection**
- ❌ **Admin E2E Tests: ModuleNotFoundError**

---

## Root Cause

### Bug 1: Script uses `|| true` to hide failures
```bash
pytest tests/ ... || true  # This always returns success!
```

### Bug 2: Script uses wrong Python
```
python3.7/lib/python3.7/importlib  # System Python 3.7
```
Should use: `admin/.venv/bin/python` (Python 3.11)

### Bug 3: Script uses `source venv/bin/activate` 
But directory is `.venv` not `venv`.

---

## Evidence

```
ModuleNotFoundError: No module named 'sqlalchemy.ext.asyncio'
ModuleNotFoundError: No module named 'pydantic'
...
!!!!!!!!!!!!!! Interrupted: 11 errors during collection !!!!!!!!!!!!!!
5 warnings, 11 errors in 2.35s
```

And yet script reports: `✅ Admin Unit Tests PASSED`

---

## Required Actions

1. **Fix `scripts/run_admin_full_suite.sh`**:
   - Remove `|| true` - let failures propagate
   - Use `.venv/bin/python` not system Python
   - Use `.venv` not `venv`

2. **Re-run tests truthfully**

3. **Do not claim success without actual verification**

---

## QA Verdict

**DEV handover REJECTED**. Test infrastructure is broken. Cannot verify claims.

---

*QA Role per [AGENTS.md](../../AGENTS.md)*
