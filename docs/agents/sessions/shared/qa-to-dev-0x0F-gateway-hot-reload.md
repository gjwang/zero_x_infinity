# QA to Dev Handover: Gateway Hot-Reload Requirement (Phase 0x0F Post-Mortem)

**From**: QA Team (Gemini Agent)  
**To**: Development Team  
**Date**: 2025-12-26  
**Priority**: P0 (Architectural Blocker)  
**Related Feature**: Admin Dashboard (Phase 0x0F) -> Gateway Propagation

---

## 🚨 Issue Summary
The Admin Dashboard functionalities (Asset/Symbol creation) are **Verified working** on the database level. However, the **End-to-End (E2E) flow fails** because the Gateway Service does not detect these changes at runtime.

**Status**: ❌ Test Failure (Expected Architectural Gap)

## 🛠 Reproduction
A dedicated CI-ready script has been committed to the repo to reproduce this issue reliably.

1. **Run the Script**:
   ```bash
   ./scripts/test_admin_e2e_ci.sh
   ```
2. **Observe Failure**:
   - Admin creates Asset (Status 200, DB Insert Success).
   - Script queries Gateway `GET /api/v1/public/assets`.
   - **Result**: New asset is NOT returned (Stale Data).

## 🔍 Root Cause Analysis
The Gateway loads `assets` and `symbols` into memory **only once** at startup.

**File**: `src/gateway/state.rs`
```rust
pub struct AppState {
    // ...
    pub pg_assets: Arc<Vec<Asset>>,   // <--- Static, Immutable Cache
    pub pg_symbols: Arc<Vec<Symbol>>, // <--- Static, Immutable Cache
    // ...
}
```

**File**: `src/gateway/main.rs`
- Data is loaded via `Database::get_all_assets()` during `main()` initialization.
- There is no mechanism to refresh `state.pg_assets` without restarting the process.

## 📋 Requirements for Developer (Next Phase)
To resolve this, you must implement a **Hot-Reload Mechanism** (Phase 0x10).

### Recommended Approaches:
1.  **Passive Polling**: Gateway background task refreshing cache every N seconds.
2.  **Active Signal**: Admin sends a "Signal" (e.g., specific HTTP Call or Redis PubSub) to Gateway to trigger reload.
3.  **On-Demand**: Change `get_assets` to query DB directly (Performance cost?).

**Goal**: `scripts/test_admin_e2e_ci.sh` must pass GREEN.

## 🔮 Phase 0x11: UI E2E (Playwright) Requirements
**Context**: Once Hot-Reload is fixed, we need a minimal UI test.
**Scope**: "Longest Path" Verification (Happy Path Only).
1.  **Action**: Playwright clicks "Create Asset" in Browser.
2.  **Verification**: Poll Gateway API (`/api/v1/public/assets`) until the asset appears.
**Rationale**: Ensures the UI Form -> API Payload mapping is correct, but offloads edge-case testing to the cheaper API E2E job.

---

**Artifacts Included**:
- `scripts/test_admin_e2e_ci.sh`: Automation script.
- `admin/test_admin_gateway_e2e.py`: The Python test logic.
