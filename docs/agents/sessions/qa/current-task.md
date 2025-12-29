# QA Current Task

## Session Info
- **Date**: 2025-12-29
- **Role**: QA Engineer
- **Task**: Phase 0x11-b: Verify Sentinel Hardening Fixes

## Original Goal
Verify that DEF-002 (BTC SegWit blindness) is fixed and ETH/ERC20 Sentinel is correctly implemented.

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
- [ ] **Chaos Testing (Re-org)**
  - [ ] Simulate block re-org after deposit detection
  - [ ] Verify: Sentinel detects hash mismatch and rolls back

## Key Decisions Made
| Decision | Rationale | Alternatives Rejected |
|----------|-----------|----------------------|
| TBD | TBD | TBD |

## Blockers / Dependencies
- [ ] **BLOCKED**: Waiting for Developer to complete DEF-002 fix
- [ ] **BLOCKED**: Waiting for Developer to implement EthScanner

## Handover Notes
**From Architect (2025-12-29)**:
- Test plan: `docs/agents/sessions/shared/arch-to-qa-0x11-b-test-plan.md`
- Main spec: `docs/src/0x11-b-sentinel-hardening.md`
- Branch: `0x11-b-sentinel-hardening`

**Acceptance Metric**: DEF-002 marked CLOSED in `docs/qa/0x11a_real_chain/test_report.md`
