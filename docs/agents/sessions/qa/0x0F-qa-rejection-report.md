# QA Rejection Report: Phase 0x0F Admin Dashboard Fixes

**Date**: 2025-12-27 00:58  
**QA Engineer**: AI Agent (QA Role)  
**Branch**: `0x0F-admin-dashboard`  
**Status**: ❌ **REJECTED - TESTS FAILING**

---

## Executive Summary

Developer claimed 177/177 unit tests pass. **QA independent verification found 3 failing tests**.

---

## Verification Results

### Unit Tests: ❌ **174/177 PASS (3 FAILED)**

```
============ 3 failed, 174 passed, 32 skipped, 36 warnings in 7.65s ============
```

### Failing Tests

| Test | Expected | Actual | Root Cause |
|------|----------|--------|------------|
| `test_asset_status_serialization` | `"ACTIVE"` | `1` | Serializer not returning string |
| `test_symbol_status_serialization` | `"ONLINE"` | `1` | Serializer not returning string |
| `test_invalid_status_inputs` | "Status must be a string" | Different error msg | Error message mismatch |

---

## Analysis

### Issue 1: Status Serialization NOT Working

Dev handover states:
> "API Simplification: Status Field... Now accepts **only strings**"

But test output shows:
```python
assert dump["status"] == "ONLINE"
E   AssertionError: assert 1 == 'ONLINE'
```

This means **status is being output as Integer, not String**. The `field_serializer` is either:
1. Not implemented
2. Commented out
3. Not being invoked

### Issue 2: Error Message Mismatch

Dev handover implies clear error messages, but actual error is:
```
"Status must be ONLINE, OFFLINE, or CLOSE_ONLY, got: 99"
```

Not: `"Status must be a string"`

---

## Required Actions Before Re-Handover

1. **Fix `AssetCreateSchema.serialize_status`** - Ensure it returns `value.name` not `value`
2. **Fix `SymbolCreateSchema.serialize_status`** - Same issue
3. **Update test assertions** - Align expected error messages with actual implementation
4. **Rerun ALL tests** - Verify 177/177 pass LOCALLY before handover

---

## QA Acceptance Criteria Status

| Criterion | Status |
|-----------|--------|
| All unit tests pass (177/177) | ❌ FAILED (3 failures) |
| Core E2E tests pass (6+/9) | ⏳ Not tested |
| Asset creation works via UI | ⏳ Not tested |
| Symbol creation works via UI | ⏳ Not tested |
| Status dropdowns show correct values | ❌ BLOCKED (API returns int) |
| No 422 errors during normal operations | ⏳ Not tested |
| Error messages are clear and actionable | ❌ FAILED (message mismatch) |

---

## Handover Response

**DEV**: Please fix the 3 failing tests and verify **locally** before re-submitting.

```bash
cd admin && pytest tests/test_ux08_status_strings.py -v
```

---

*QA Role per [AGENTS.md](../../AGENTS.md)*
