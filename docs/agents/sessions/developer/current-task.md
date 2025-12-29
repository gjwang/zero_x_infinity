# Developer Current Task

## Session Info
- **Date**: 2025-12-29
- **Role**: Developer
- **Task**: Phase 0x11-b: Fix DEF-002 & Implement ETH Sentinel

## Original Goal
Fix Sentinel blindness to SegWit (P2WPKH) deposits and implement ETH/ERC20 event log parsing.

## Progress Checklist
- [ ] **DEF-002 Fix (BTC P2WPKH)**
  - [ ] Add unit test `test_p2wpkh_extraction` in `src/sentinel/btc.rs`
  - [ ] Fix `extract_address` to handle P2WPKH scripts
  - [ ] Verify with E2E: deposit to `bcrt1...` address detected
- [ ] **ETH Sentinel Implementation**
  - [ ] Implement `EthScanner` in `src/sentinel/eth.rs`
  - [ ] Add `eth_getLogs` polling with Transfer topic filter
  - [ ] Parse ERC20 Transfer events (topic[2] = recipient, data = amount)
  - [ ] Add unit test `test_erc20_transfer_parsing`
  - [ ] Verify with E2E: MockUSDT deposit detected
- [ ] **ADR-004 Implementation (Chain Asset Binding)**
  - [ ] Create migration: `chains_tb` and `chain_assets_tb`
  - [ ] Refactor `BtcScanner`/`EthScanner` to load config from DB (Hot Reload)
  - [ ] Remove hardcoded tokens from `config.toml`

## Key Decisions Made
| Decision | Rationale | Alternatives Rejected |
|----------|-----------|----------------------|
| TBD | TBD | TBD |

## Blockers / Dependencies
- [ ] None currently - ready for implementation

## Handover Notes
**From Architect (2025-12-29)**:
- Main spec: `docs/src/0x11-b-sentinel-hardening.md`
- Detailed handover: `docs/agents/sessions/shared/arch-to-dev-0x11-b-def-002.md`
- Branch: `0x11-b-sentinel-hardening`

**Priority**: P0 = DEF-002 (BTC), P1 = ETH Sentinel
