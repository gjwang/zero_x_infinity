# QA to DEV Handover: Gateway Hot-Reload Required

**Date**: 2025-12-27 01:26  
**From**: QA Engineer  
**To**: DEV Team  
**Priority**: 🔴 **P0 Blocker**  
**Branch**: `0x0F-admin-dashboard`

---

## 🎯 Issue Summary

Gateway API does not return newly created Assets/Symbols from Admin Dashboard.  
**Root Cause**: Data is cached at startup, no runtime reload mechanism.

---

## 📋 Reproduction Steps

```bash
# 1. Start full environment
./scripts/test_admin_e2e_ci.sh

# 2. Observe output:
✅ Asset created: E2E_A_1766769688  # Admin writes to DB successfully
❌ Asset E2E_A_1766769688 not found in Gateway response  # Gateway returns stale data
```

---

## 🔍 Root Cause Analysis

| Component | File | Issue |
|-----------|------|-------|
| `get_assets()` | `src/gateway/handlers.rs:1258` | Reads from `state.pg_assets` (static cache) |
| `get_symbols()` | `src/gateway/handlers.rs:1290` | Reads from `state.pg_symbols` (static cache) |
| Cache Init | `src/main.rs:247` | Assets/Symbols loaded once at startup |

---

## ✅ Required Fix

Replace cached reads with live database queries:

```rust
// BEFORE (Line 1262-1274)
let assets: Vec<AssetApiData> = state.pg_assets.iter()...

// AFTER
let assets = if let Some(ref db) = state.pg_db {
    crate::AssetManager::load_all(db.pool()).await?
} else {
    state.pg_assets.iter()...  // fallback
};
```

Same pattern for `get_symbols()`.

---

## 📊 Test Verification

After fix, run:
```bash
./scripts/test_admin_e2e_ci.sh
# Expected: 4/4 E2E tests PASS
```

---

## 📎 Related Documents

- QA Approval Report: `docs/agents/sessions/qa/0x0F-qa-approval-report.md`
- Hot-Reload Design (existing): `docs/agents/sessions/shared/qa-to-dev-0x0F-gateway-hot-reload.md`

---

## ⏱️ Timeline

**Requested**: Immediate fix required for Phase 0x0F completion.

---

*QA Role per [AGENTS.md](../../AGENTS.md)*
