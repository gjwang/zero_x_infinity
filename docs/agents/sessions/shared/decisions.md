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

<!-- Add new decisions above this line -->
