# AI Agent Sessions Directory

This directory contains **working documents** for each AI role. Each role maintains its own process documents to enable:

- ✅ **Fast Handover**: Next session can quickly pick up context
- ✅ **Parallel Work**: Different roles work independently  
- ✅ **Shared Context**: Important decisions visible to all roles

---

## 📁 Directory Structure

```
docs/agents/sessions/
├── README.md           # This file
├── shared/             # 🔗 Cross-role shared documents
│   ├── decisions.md    # Architectural decisions log
│   └── blockers.md     # Current blockers & status
├── architect/          # 🏛️ Architect working notes
│   └── current-task.md
├── developer/          # 💻 Developer working notes
│   └── current-task.md
├── qa/                 # 🧪 QA Engineer working notes
│   └── current-task.md
├── security/           # 🔒 Security Reviewer working notes
│   └── current-task.md
└── devops/             # 🔧 DevOps Engineer working notes
    └── current-task.md
```

---

## 📋 Document Templates

### Per-Role: `current-task.md`

Each role maintains a `current-task.md` file:

```markdown
# [Role] Current Task

## Session Info
- **Date**: YYYY-MM-DD
- **Role**: [Architect/Developer/QA/Security/DevOps]
- **Task**: [Brief description]

## Original Goal
[Copy of the original user request or task]

## Progress Checklist
- [x] Completed step 1
- [x] Completed step 2  
- [ ] In progress: step 3
- [ ] Pending: step 4

## Key Decisions Made
| Decision | Rationale | Alternatives Rejected |
|----------|-----------|----------------------|
| [Choice] | [Why] | [What else considered] |

## Blockers / Dependencies
- [ ] Blocker: [description] - assigned to: [role]

## Handover Notes
[What the next session needs to know to continue]
```

### Shared: `decisions.md`

Log of important cross-role decisions:

```markdown
# Shared Decisions Log

## [Date] - [Decision Title]
- **Decided by**: [Role]
- **Decision**: [What was decided]
- **Rationale**: [Why]
- **Impact on other roles**: [Who needs to know]
```

### Shared: `blockers.md`

Current blockers affecting multiple roles:

```markdown
# Active Blockers

## [BLOCKER-001] - [Title]
- **Status**: 🔴 Open / 🟡 In Progress / 🟢 Resolved
- **Reported by**: [Role]
- **Affects**: [Which roles]
- **Description**: [Details]
- **Resolution**: [How it was/will be fixed]
```

---

## 🔄 Workflow

### Starting a Session

1. Read `shared/decisions.md` and `shared/blockers.md`
2. Read your role's `current-task.md`
3. Continue from where last session left off

### During a Session

1. Update your `current-task.md` as you progress
2. Log important decisions to `shared/decisions.md`
3. Report blockers to `shared/blockers.md`

### Ending a Session

1. Update progress checklist (mark completed items)
2. Write handover notes for next session
3. Commit all session documents

---

## 🔗 Related Documents

- [AGENTS.md](../../../AGENTS.md) - Top-level agent configuration
- [Specification Mode](../specification-mode.md) - Planning workflow

---

*This system enables continuous progress across multiple AI sessions.*
