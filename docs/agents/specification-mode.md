# AI Agent Specification Mode Workflow

> **Principle**: Plan First, Code Later. Zero risk during planning phase.
> 
> **原则**: 先规划，后编码。规划阶段零风险。

---

## 🚨 Problem Statement

Traditional "Vibe Coding" with AI agents leads to:

| Issue | Impact |
|-------|--------|
| **Incomplete Planning** | Features miss edge cases, breaking in production |
| **Architecture Drift** | Inconsistent patterns, technical debt accumulation |
| **Security Gaps** | Vulnerabilities introduced without proper review |
| **Missing Tests** | Low coverage, regression bugs |
| **Uncontrolled Changes** | Agent modifies files unexpectedly, breaking builds |

---

## ✅ Solution: Specification Mode

### Core Principle

**The agent MUST NOT modify any code during the planning phase.**

Instead, the agent operates in **Read-Only Analysis Mode**:
1. Deep analysis of the entire codebase
2. Reference project standards (AGENTS.md, conventions)
3. Generate comprehensive specification before any implementation

---

## 📋 Specification Output Requirements

When given a feature request (4-6 sentences of natural language), the agent must produce:

### 1. Feature Summary
- Clear description of what will be built
- Scope boundaries (what's included / excluded)

### 2. Acceptance Criteria
```markdown
## Acceptance Criteria

- [ ] User can [action] with [expected result]
- [ ] System handles [edge case] by [behavior]
- [ ] API returns [response format] for [endpoint]
- [ ] Error case [X] displays [Y] message
```

### 3. Implementation Plan
```markdown
## Implementation Plan

### Phase 1: [Component Name]
- [ ] Step 1.1: [Description]
- [ ] Step 1.2: [Description]

### Phase 2: [Component Name]
- [ ] Step 2.1: [Description]
```

### 4. File-Level Breakdown
```markdown
## File Changes

| Action | File | Description |
|--------|------|-------------|
| MODIFY | `src/handlers.rs` | Add new endpoint handler |
| NEW | `src/models/user.rs` | Create user model struct |
| MODIFY | `tests/integration.rs` | Add E2E test cases |
```

### 5. Test Strategy
```markdown
## Test Strategy

### Unit Tests
- [ ] Test: [function] returns [expected] when [condition]

### Integration Tests
- [ ] Test: API endpoint [path] with [method] returns [status]

### E2E Tests
- [ ] Test: User flow [A → B → C] completes successfully
```

### 6. Security & Compliance Checklist
```markdown
## Security Review

- [ ] Input validation implemented
- [ ] Authentication required for private endpoints
- [ ] No secrets logged or exposed
- [ ] Rate limiting considered
- [ ] Audit trail for sensitive operations
```

---

## 🎭 Multi-Role Specification Review

Each specification must be reviewed through **5 distinct perspectives** before approval:

### Role Review Flow

```
Specification ──► 🏛️ Architect ──► 💻 Developer ──► 🧪 QA ──► 🔒 Security ──► 🔧 DevOps ──► ✅ Approved
     │                 │               │              │            │             │
     ▼                 ▼               ▼              ▼            ▼             ▼
   Draft          Design OK?      Feasible?     Testable?     Secure?      Deployable?
```

---

### 🏛️ Architect Review Focus

| Focus Area | Key Questions |
|------------|---------------|
| **System Boundaries** | Does this fit within existing architecture? |
| **Component Coupling** | Will this create tight coupling? |
| **Scalability** | Can this scale to 10x load? |
| **Data Flow** | Is data flow clear and efficient? |
| **Technical Debt** | Does this add or reduce debt? |

**Specification Section to Review**: Implementation Plan, File Breakdown

**Output**:
```markdown
### 🏛️ Architect Sign-off
- [ ] Architecture alignment verified
- [ ] No new anti-patterns introduced
- [ ] Scalability considered
- Concerns: [if any]
```

---

### 💻 Developer Review Focus

| Focus Area | Key Questions |
|------------|---------------|
| **Implementation Feasibility** | Can this be built as specified? |
| **Effort Estimation** | Is LOC estimate realistic? |
| **Edge Cases** | Are all edge cases identified? |
| **Error Handling** | Is error handling specified? |
| **Dependencies** | Are all dependencies identified? |

**Specification Section to Review**: Implementation Plan, File Breakdown, Acceptance Criteria

**Output**:
```markdown
### 💻 Developer Sign-off
- [ ] Implementation approach validated
- [ ] Effort estimate confirmed (~X hours)
- [ ] Edge cases documented
- Concerns: [if any]
```

---

### 🧪 QA Engineer Review Focus

| Focus Area | Key Questions |
|------------|---------------|
| **Test Coverage** | Are all acceptance criteria testable? |
| **Edge Cases** | Are boundary conditions covered? |
| **Regression Risk** | What existing tests might break? |
| **E2E Scenarios** | Is the happy path fully testable? |
| **Performance Tests** | Are load tests needed? |

**Specification Section to Review**: Acceptance Criteria, Test Strategy

**Output**:
```markdown
### 🧪 QA Sign-off
- [ ] All acceptance criteria testable
- [ ] Edge cases covered in test plan
- [ ] Regression test scope defined
- Missing tests: [if any]
```

---

### 🔒 Security Reviewer Focus

| Focus Area | Key Questions |
|------------|---------------|
| **Authentication** | Are auth requirements specified? |
| **Authorization** | Are permissions checked correctly? |
| **Input Validation** | Is all input validated? |
| **Data Protection** | Is sensitive data protected? |
| **Audit Logging** | Are security events logged? |

**Specification Section to Review**: Security Checklist, API Endpoints

**Output**:
```markdown
### 🔒 Security Sign-off
- [ ] No obvious vulnerabilities
- [ ] Auth/authz requirements clear
- [ ] Input validation specified
- Vulnerabilities found: [if any]
```

---

### 🔧 DevOps Engineer Review Focus

| Focus Area | Key Questions |
|------------|---------------|
| **Deployment Impact** | Can this be deployed with zero downtime? |
| **Configuration** | Are new configs documented? |
| **Monitoring** | Are new metrics needed? |
| **Rollback** | What's the rollback plan? |
| **Resource Requirements** | Memory/CPU/storage impact? |

**Specification Section to Review**: File Breakdown, Dependencies

**Output**:
```markdown
### 🔧 DevOps Sign-off
- [ ] Deployment strategy clear
- [ ] Rollback plan defined
- [ ] Monitoring requirements identified
- Operational risks: [if any]
```

---

## ✅ Consolidated Approval Template

```markdown
# Specification Approval: [Feature Name]

## Review Status

| Role | Reviewer | Status | Notes |
|------|----------|--------|-------|
| 🏛️ Architect | [name/AI] | ✅/⚠️/❌ | [notes] |
| 💻 Developer | [name/AI] | ✅/⚠️/❌ | [notes] |
| 🧪 QA | [name/AI] | ✅/⚠️/❌ | [notes] |
| 🔒 Security | [name/AI] | ✅/⚠️/❌ | [notes] |
| 🔧 DevOps | [name/AI] | ✅/⚠️/❌ | [notes] |

## Decision

- [ ] **APPROVED** - Proceed to execution
- [ ] **CONDITIONALLY APPROVED** - Address concerns first
- [ ] **REJECTED** - Requires redesign

## Execution Level Selected

- [ ] Low (confirm each change)
- [ ] Medium (batch confirm by phase)
- [ ] High (auto-execute with checkpoints)
```

---

## 🎮 Execution Control Levels

After specification approval, choose execution autonomy level:

| Level | Behavior | Use When |
|-------|----------|----------|
| **Low** | Confirm each file modification before applying | High-risk changes, unfamiliar codebase |
| **Medium** | Batch confirm by component/phase | Standard features, moderate complexity |
| **High** | Auto-execute with commit checkpoints | Trusted patterns, low-risk additions |

### Level Selection Guide

```
Is this a critical system (auth, payments, data)?
  └─ Yes → Low
  └─ No → Is this touching >10 files?
            └─ Yes → Medium
            └─ No → High (with tests passing gate)
```

---

## 🔄 Complete Workflow

```
┌─────────────────────────────────────────────────────────────────────┐
│                    SPECIFICATION MODE WORKFLOW                      │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  Step 1: User provides 4-6 sentence feature request                │
│          ↓                                                          │
│  Step 2: Agent enters READ-ONLY mode                               │
│          - Analyze codebase                                         │
│          - Check AGENTS.md / conventions                            │
│          - NO code modifications                                    │
│          ↓                                                          │
│  Step 3: Agent generates Specification                             │
│          - Feature Summary                                          │
│          - Acceptance Criteria                                      │
│          - Implementation Plan                                      │
│          - File Breakdown                                           │
│          - Test Strategy                                            │
│          - Security Checklist                                       │
│          ↓                                                          │
│  Step 4: Multi-Role Review                                         │
│          🏛️ Architect → 💻 Developer → 🧪 QA → 🔒 Security → 🔧 DevOps │
│          ↓                                                          │
│  Step 5: Consolidated Approval                                     │
│          - All roles sign off                                       │
│          - Select execution level                                   │
│          ↓                                                          │
│  Step 6: Agent executes with selected autonomy level               │
│          - Commits at checkpoints                                   │
│          - Tests run after each phase                               │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 💡 Benefits

| Benefit | Description |
|---------|-------------|
| **Architectural Integrity** | Design reviewed by Architect role before implementation |
| **Code Quality** | Developer role validates feasibility and edge cases |
| **Test Coverage** | QA role ensures complete test strategy |
| **Security Assurance** | Security role catches vulnerabilities in planning |
| **Operational Readiness** | DevOps role ensures deployability |
| **Controlled Execution** | No surprise file modifications |
| **Audit Trail** | Full documentation with role sign-offs |

---

## 📝 Quick Reference Card

| Phase | Actions | Roles Involved |
|-------|---------|----------------|
| **Request** | User describes feature (4-6 sentences) | User |
| **Analysis** | Agent reads codebase (NO writes) | Agent (Read-Only) |
| **Specification** | Generate detailed spec | Agent |
| **Review** | Multi-role sign-off | 5 AI Roles |
| **Approval** | Consolidated decision | User |
| **Execution** | Implement with controls | Agent (Level-based) |

---

## 🔗 Related Documents

- [AI Review Roles](./ai-review-roles.md) - Detailed persona definitions
- [Pre-merge Checklist](./pre-merge-checklist.md) - Quality gates
- [Development Guidelines](../../standards/development-guidelines.md) - Coding standards

---

*This workflow ensures engineering rigor in AI-assisted development through multi-perspective review.*

