# AGENTS.md

> **Top-Level AI Agent Configuration for Zero X Infinity**
>
> This file is the primary entry point for all AI agents working on this codebase.

---

## 🎯 Project Overview

**Zero X Infinity** is a production-grade cryptocurrency matching engine achieving **1.3M orders/sec** on a single core.

| Aspect | Details |
|--------|---------|
| **Language** | Rust |
| **Architecture** | LMAX Disruptor-style Ring Buffer Pipeline |
| **Database** | PostgreSQL (config) + TDengine (trading data) |
| **API** | REST + WebSocket with Ed25519 authentication |
| **Current Phase** | 0x0C Trade Fee System (completed) |

---

## 📖 Essential Reading

Before making any changes, AI agents MUST read:

| Document | Purpose |
|----------|---------|
| [Project Roadmap](./docs/src/0x00-mvp-roadmap.md) | Current progress and planned phases |
| [API Conventions](./docs/standards/api-conventions.md) | REST API standards |
| [ID Specification](./docs/src/0x0A-b-id-specification.md) | Identity addressing rules |
| [Development Guidelines](./docs/standards/development-guidelines.md) | Coding standards |

---

## 🧭 Core Principle: Stay on Track

> **AI agents easily lose direction during complex tasks. To prevent this, ALL non-trivial work MUST follow the blueprint-first approach.**

### The Problem

During complex tasks, AI agents tend to:
- Forget the original goal mid-execution
- Go down rabbit holes on tangential issues
- Make changes that drift from the specification
- Lose context after multiple iterations

### The Solution: Blueprint-First Execution

```
┌─────────────────────────────────────────────────────────────────────┐
│                    BLUEPRINT-FIRST EXECUTION                       │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  1. 📋 WRITE BLUEPRINT FIRST                                       │
│     - Define the goal in 1-2 sentences                             │
│     - List all sub-tasks as checklist                              │
│     - Identify success criteria                                    │
│                                                                     │
│  2. ✅ CREATE PROGRESS CHECKLIST                                   │
│     - [ ] Task 1: ...                                              │
│     - [ ] Task 2: ...                                              │
│     - [ ] Task N: ...                                              │
│                                                                     │
│  3. 🔄 EXECUTE WITH CONTINUOUS ALIGNMENT                           │
│     - Before each action: "Does this serve the original goal?"     │
│     - After each step: Check off completed items                   │
│     - If drifting: STOP, re-read blueprint, realign               │
│                                                                     │
│  4. 🎯 FINAL VERIFICATION                                         │
│     - Compare result against original goal                         │
│     - Verify all checklist items completed                         │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### Alignment Checkpoints

At these moments, STOP and verify alignment with original goal:

| Checkpoint | Action |
|------------|--------|
| **Before starting** | Re-read the user's original request |
| **After each sub-task** | Check: "Did this move toward the goal?" |
| **When encountering blockers** | Ask: "Is this blocker on the critical path?" |
| **Before making new decisions** | Verify: "Is this decision in scope?" |
| **Before completing** | Confirm: "Did I achieve what was originally asked?" |

### Task Complexity Threshold

| Complexity | Blueprint Required? | Example |
|------------|---------------------|---------|
| **Simple** (1-2 files, <30 min) | Optional | Fix typo, add log line |
| **Medium** (3-10 files, 1-2 hrs) | **Required** | New API endpoint, refactor module |
| **Complex** (>10 files, >2 hrs) | **Mandatory + Review** | New feature, architectural change |

### Blueprint Template

```markdown
## Task: [Original User Request - verbatim or summary]

### Goal (1-2 sentences)
[What success looks like]

### Success Criteria
- [ ] Criterion 1
- [ ] Criterion 2

### Progress Checklist
- [ ] Step 1: ...
- [ ] Step 2: ...
- [ ] Step 3: ...

### Out of Scope (explicitly)
- [Things NOT to do]

### Alignment Notes
- [Key constraints to remember]
```

---

## 🎭 AI Role System

This project uses a **Multi-Role AI Review System**. Each role has specific responsibilities and review focus areas.

### Available Roles

| Role | File | Primary Focus |
|------|------|---------------|
| 🏛️ Architect | [architect.md](./docs/agents/architect.md) | System design, scalability |
| 💻 Developer | [developer.md](./docs/agents/developer.md) | Code quality, implementation |
| 🧪 QA Engineer | [qa-engineer.md](./docs/agents/qa-engineer.md) | Testing, edge cases |
| 🔒 Security | [security-reviewer.md](./docs/agents/security-reviewer.md) | Vulnerabilities, threats |
| 🔧 DevOps | [devops-engineer.md](./docs/agents/devops-engineer.md) | Deployment, operations |

### How to Activate a Role

```
I am acting as the [ROLE NAME] as defined in AGENTS.md.
My primary focus is [FOCUS AREA].
I will review/implement with [ROLE]'s perspective.
```

---

## 🔄 Workflow: Specification Mode

**Principle**: Plan First, Code Later. Zero risk during planning phase.

See: [Specification Mode Workflow](./docs/agents/specification-mode.md)

### Quick Summary

```
┌─────────────────────────────────────────────────────────────────────┐
│ 1. User Request (4-6 sentences)                                    │
│ 2. Agent READ-ONLY Analysis (no code changes)                      │
│ 3. Generate Specification                                          │
│ 4. Multi-Role Review (Architect → Developer → QA → Security → DevOps) │
│ 5. User Approval                                                    │
│ 6. Controlled Execution (Low/Medium/High autonomy)                 │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 🏗️ Architecture Quick Reference

```mermaid
graph TD
    Client[Client] -->|HTTP/WS| Gateway
    Gateway -->|RingBuffer| Ingestion
    subgraph "Trading Core (Single Thread)"
        Ingestion -->|SeqOrder| UBSCore["UBSCore (Risk/Balance)"]
        UBSCore -->|LockedOrder| ME["Matching Engine"]
        ME -->|Trade/OrderUpdate| Settlement
    end
    Settlement -->|Async| TDengine[TDengine]
    Settlement -->|Async| WS["WebSocket Push"]
```

---

## 📁 Key Directories

| Directory | Purpose |
|-----------|---------|
| `src/` | Rust source code |
| `src/gateway/` | HTTP API handlers |
| `src/persistence/` | TDengine queries |
| `src/pipeline/` | Ring Buffer implementation |
| `docs/` | mdBook documentation |
| `scripts/` | Build/test/deploy scripts |
| `config/` | YAML configuration files |
| `baseline/` | Golden test baselines |

---

## ⚠️ Critical Rules

### DO NOT

- ❌ Modify code during planning phase (Specification Mode)
- ❌ Use `f64` for financial calculations (use `u64` with 10^6 precision)
- ❌ Use `docker exec` in CI scripts (use REST API instead)
- ❌ Commit without running `cargo fmt` and `cargo clippy`
- ❌ Hardcode ports or credentials (use environment variables)

### MUST DO

- ✅ Read role definition before starting work
- ✅ Generate specification before implementation
- ✅ Run `./scripts/pre-commit.sh` before committing
- ✅ Source `scripts/lib/db_env.sh` in test scripts
- ✅ Follow [Pre-merge Checklist](./docs/src/standards/pre-merge-checklist.md)

---

## 🔗 Navigation

### Agent Configuration
- [AGENTS.md](./AGENTS.md) ← You are here
- [Specification Mode](./docs/agents/specification-mode.md)
- [Role: Architect](./docs/agents/architect.md)
- [Role: Developer](./docs/agents/developer.md)
- [Role: QA Engineer](./docs/agents/qa-engineer.md)
- [Role: Security Reviewer](./docs/agents/security-reviewer.md)
- [Role: DevOps Engineer](./docs/agents/devops-engineer.md)

### Project Standards
- [Development Guidelines](./docs/standards/development-guidelines.md)
- [API Conventions](./docs/standards/api-conventions.md)
- [CI Pitfalls](./docs/src/standards/ci-pitfalls.md)
- [Pre-merge Checklist](./docs/src/standards/pre-merge-checklist.md)

### Technical Documentation
- [Project Roadmap](./docs/src/0x00-mvp-roadmap.md)
- [Trade Fee System](./docs/src/0x0C-trade-fee.md)
- [ID Specification](./docs/src/0x0A-b-id-specification.md)
- [Database Selection](./docs/src/database-selection-tdengine.md)

---

*Last Updated: 2024-12-25*
