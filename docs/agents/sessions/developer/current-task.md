# Developer Current Task

## Session Info
- **Date**: 2025-12-30
- **Role**: Developer
- **Task**: Phase 0x11-b: Unified Asset Architecture + DEF-002 Fix

## Original Goal
Implement the 3-Layer Asset Architecture (ADR-005/006) and fix Sentinel BTC detection.

## Progress Checklist
- [ ] **Database Migration (012_unified_assets.sql)**
  - [ ] Create `chains_tb` table (chain_slug, rpc_urls, confirmation_blocks)
  - [ ] Create `chain_assets_tb` table (contract_address, decimals, fees)
  - [ ] Create `user_chain_addresses` table (user_id, chain_slug, address)
  - [ ] Add foreign keys and constraints
- [ ] **Sentinel Refactoring**
  - [ ] Refactor `BtcScanner` to load config from `chain_assets_tb`
  - [ ] Implement `EthScanner` with Transfer event parsing
  - [ ] Implement Hot Reload (60s interval refresh from DB)
  - [ ] Implement Dual-Lookup: Contract Address -> Asset, User Address -> User
- [ ] **DEF-002 Fix (BTC P2WPKH)**
  - [ ] Add unit test `test_p2wpkh_extraction`
  - [ ] Fix `extract_address` to handle P2WPKH scripts
  - [ ] Verify with E2E: deposit to `bcrt1...` address detected

## Key Architecture References
| Document | Path |
|----------|------|
| **ADR-005** (Unified Asset Schema) | `docs/src/architecture/decisions/ADR-005-unified-asset-schema.md` |
| **ADR-006** (User Address Decoupling) | `docs/src/architecture/decisions/ADR-006-user-address-decoupling.md` |
| **Token Listing SOP** | `docs/src/manuals/0x0F-token-listing-sop.md` |
| **Main Spec** | `docs/src/0x11-b-sentinel-hardening.md` |

## Blockers / Dependencies
- [ ] None - Ready for implementation

## Handover Notes
**From Architect (2025-12-30)**:
- **Priority**: P0 = DEF-002 (BTC), P1 = DB Migration, P2 = ETH Sentinel
- **Branch**: `0x11-b-sentinel-hardening`
- **Schema Default**: `is_active = FALSE` for new chain_assets (Safety First)
