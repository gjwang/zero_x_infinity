# Handover: Architect -> Developer (Phase 0x11-b)

**Date**: 2025-12-30
**Phase**: 0x11-b (Sentinel Hardening & Hot Listing)
**Context**: Phase 0x11-b expands "Sentinel Hardening" to include a complete **Unified Chain-Asset Architecture**, enabling "Hot Listing" without code redeploys.

## 1. Objectives

| Priority | Task | Description |
| :--- | :--- | :--- |
| **P0 (Blocker)** | **Fix DEF-002 (BTC P2WPKH)** | Sentinel must detect SegWit (`bcrt1...`) deposits. |
| **P0 (Arch)** | **Implement Unified Schema** | Implement [ADR-005] and [ADR-006] (Chains, ChainAssets, UserChainAddresses). |
| **P1 (feat)** | **Implement Hot Listing** | Sentinel must refresh config from DB and support `eth_getLogs` for new tokens dynamically. |

---

## 2. Technical Specification: Unified Architecture (The Core)

### 2.1 Database Schema (ADR-005 & ADR-006)
You must create migration `012_unified_assets.sql`:
1.  **`chains_tb`**: Configuration for chains (RPC, Confirmations).
2.  **`chain_assets_tb`**: Binding `contract_address` to `asset_id`.
3.  **`user_chain_addresses`**: Binding `address` to `user_id` (Per Chain).

**Reference**: `docs/src/architecture/decisions/ADR-005-unified-asset-schema.md`

### 2.2 Hot Listing Workflow (The Loop)
Sentinel's `EthScanner` must:
1.  **Poll Config**: Every 60s, read `chain_assets_tb`.
2.  **Refresh Filters**: Update `watched_contracts`.
3.  **Scan**: Use `eth_getLogs` with updated filters.

---

## 3. Technical Specification: DEF-002 (BTC Fix)
*   **Problem**: `extract_address` fails on P2WPKH.
*   **Fix**: Identify correct `rust-bitcoin` network type and parsing logic.
*   **Test**: `test_segwit_p2wpkh_extraction_def_002` MUST PASS.

---

## 4. Acceptance Criteria
- [ ] **Schema Migration**: `012_unified_assets.sql` applied successfully.
- [ ] **BTC Fix**: `test_debug_segwit_extraction` passes.
- [ ] **Hot Listing E2E**:
    1.  Start System (No UNI).
    2.  Insert UNI into DB (Simulate Admin).
    3.  Wait 60s.
    4.  Deposit UNI -> **Detected & Credited**.

## 5. Next Steps for Developer
1.  **Schema**: Create `migrations/012_unified_assets.sql`.
2.  **Sentinel Core**: Refactor `EthScanner` to use `ChainManager` for config.
3.  **Bug Fix**: Solve DEF-002 in `btc.rs`.

---

## 6. Resources
- **Master Plan**: `implementation_plan.md` (Unified Architecture)
- **ADR-005**: Unified Asset Schema
- **ADR-006**: User Address Decoupling
