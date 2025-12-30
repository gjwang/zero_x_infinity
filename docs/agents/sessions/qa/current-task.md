# QA Current Task

## Session Info
- **Date**: 2025-12-30
- **Role**: QA Engineer
- **Task**: Phase 0x11-b: Verify Unified Asset Architecture & Sentinel Fixes

## Original Goal
Verify that DEF-002 (BTC SegWit blindness) is fixed, ETH/ERC20 Sentinel works, and the new Asset Schema enables Hot Listing.

## Progress Checklist
- [ ] **Regression Testing (Phase 0x11-a)**
  - [ ] Run `scripts/run_0x11a_verification.sh` - all tests pass
  - [ ] Verify Legacy P2PKH deposits still work
- [ ] **DEF-002 Verification (BTC P2WPKH)**
  - [ ] Execute TC-B01: Deposit to `bcrt1...` address
  - [ ] Verify: Status transitions DETECTED → CONFIRMING → FINALIZED
  - [ ] Verify: User balance correctly credited
- [ ] **ETH/ERC20 Verification**
  - [ ] Execute TC-B02: ERC20 Transfer to user address
  - [ ] Verify: `DetectedDeposit` created with correct precision
  - [ ] Verify: USDT amount scaled correctly (6 decimals)
- [ ] **Hot Listing Verification (NEW)**
  - [ ] Add new token via Admin (follow `token_listing_sop.md`)
  - [ ] Verify: Sentinel detects new token deposit within 60s (no restart)
  - [ ] Verify: User deposit uses EXISTING chain address (no new address generated)
- [ ] **Chaos Testing (Re-org)**
  - [ ] Simulate block re-org after deposit detection
  - [ ] Verify: Sentinel detects hash mismatch and rolls back

## Key Architecture References
| Document | Path |
|----------|------|
| **ADR-005** (Unified Asset Schema) | `docs/src/architecture/decisions/ADR-005-unified-asset-schema.md` |
| **ADR-006** (User Address Decoupling) | `docs/src/architecture/decisions/ADR-006-user-address-decoupling.md` |
| **Token Listing SOP** | `docs/src/manuals/0x0F-token-listing-sop.md` |
| **Test Plan** | `docs/agents/sessions/shared/arch-to-qa-0x11-b-test-plan.md` |

## Blockers / Dependencies
- [ ] **BLOCKED**: Waiting for Developer to complete DB Migration (012)
- [ ] **BLOCKED**: Waiting for Developer to implement Hot Reload in Sentinel

## Handover Notes
**From Architect (2025-12-30)**:
- **Branch**: `0x11-b-sentinel-hardening`
- **Acceptance Metric**: DEF-002 marked CLOSED + Hot Listing verified
