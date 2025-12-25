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

<!-- Add new decisions above this line -->
