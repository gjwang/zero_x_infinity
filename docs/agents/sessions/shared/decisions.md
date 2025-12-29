# Shared Decisions Log

> Cross-role architectural and design decisions that affect multiple team members.

---

## How to Use

When making a decision that impacts other roles:

1. Add an entry below with date, decision, and rationale
2. Tag which roles are affected
3. Notify affected roles via their `current-task.md`

---

## Decisions

### 2024-12-25 - AI Agent System Structure

- **Decided by**: User + AI
- **Decision**: Organize AI roles into separate files with individual session directories
- **Rationale**: Enable parallel work, fast handover, and clear separation of concerns
- **Impact on other roles**: All roles now have independent working directories
- **Files created**:
  - `AGENTS.md` - Top-level entry point
  - `docs/agents/*.md` - Role definitions
  - `docs/agents/sessions/*/` - Working directories

---

### 2025-12-26 - Phase 0x0D Persistence Layer Merged

- **Decided by**: Architect (AI Agent)
- **Decision**: Merge `0x0D-wal-snapshot-design` to `main` with tag `v0.0D-persistence`
- **Rationale**: QA approved all 3 services (UBSCore, Matching, Settlement). 289 tests pass. Adversarial audit verified crash recovery, zombie detection, corruption fallback.
- **Impact on other roles**: Phase 0x0E (Deposit/Withdraw) can now begin
- **Key commits**:
  - `a976c51` - Merge Phase 0x0D with CI tests
  - `cd1b5c6` - CI: add Phase 0x0D persistence layer unit tests
- **Tag**: `v0.0D-persistence`

---

### 2025-12-26 - 0x0E OpenAPI Integration Assigned to Developer

- **Decided by**: Architect (AI Agent)
- **Decision**: Assign 0x0E OpenAPI documentation implementation to Developer role
- **Rationale**: Production API missing formal documentation; blocks SDK generation and external integration
- **Impact on other roles**: 
  - Developer: New task (4 phases, ~5 days)
  - QA: Will need to verify Swagger UI accessibility + SDK generation
- **Key documents**:
  - Design: `docs/src/0x0E-openapi-integration.md`
  - Implementation Plan: `docs/agents/sessions/developer/0x0E-openapi-implementation-plan.md`
- **Technology**: utoipa v5.3 + utoipa-swagger-ui v8.0

---

### 2025-12-26 - Phase 0x0E OpenAPI Merged to Main

- **Decided by**: Architect (AI Agent)
- **Decision**: Merge `0x0E-openapi-integration` branch to `main` with tag `v0.0E-openapi`
- **Rationale**: QA approved all tests (293 unit, 17 E2E). Swagger UI at `/docs` verified.
- **Impact on other roles**: 
  - DevOps: New dependency `utoipa` + `utoipa-swagger-ui`
  - All: `/docs` now serves interactive API documentation
- **Key commits**:
  - `8d57365` - Merge 0x0E-openapi-integration
- **Tag**: `v0.0E-openapi`

---

### 2025-12-29 - Phase 0x11-b Sentinel Hardening Handover

- **Decided by**: Architect (AI Agent)
- **Decision**: Hand over Phase 0x11-b to Developer and QA for implementation and verification
- **Rationale**: Phase 0x11-a complete (12/12 mock tests pass). Two critical gaps identified:
  1. **DEF-002** (P0): Sentinel blind to P2WPKH (SegWit) BTC deposits
  2. **ETH Gap** (P1): `EthScanner` not implemented (ERC20 event parsing)
- **Impact on other roles**:
  - **Developer**: Fix `src/sentinel/btc.rs` for P2WPKH; Implement `src/sentinel/eth.rs`
  - **QA**: Verify fixes per test plan; Update `test_report.md` to close DEF-002
- **Key documents**:
  - Main Spec: `docs/src/0x11-b-sentinel-hardening.md`
  - Dev Handover: `docs/agents/sessions/shared/arch-to-dev-0x11-b-def-002.md`
  - QA Test Plan: `docs/agents/sessions/shared/arch-to-qa-0x11-b-test-plan.md`
- **Branch**: `0x11-b-sentinel-hardening`

---

<!-- Add new decisions above this line -->
