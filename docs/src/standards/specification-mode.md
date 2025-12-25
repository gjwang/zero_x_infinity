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

## 🔄 Workflow Summary

```
┌─────────────────────────────────────────────────────────────────┐
│                    SPECIFICATION MODE                          │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  Step 1: User provides 4-6 sentence feature request            │
│          ↓                                                      │
│  Step 2: Agent enters READ-ONLY mode                           │
│          - Analyze codebase                                     │
│          - Check AGENTS.md / conventions                        │
│          - NO code modifications                                │
│          ↓                                                      │
│  Step 3: Agent generates Specification                         │
│          - Acceptance Criteria                                  │
│          - Implementation Plan                                  │
│          - File Breakdown                                       │
│          - Test Strategy                                        │
│          - Security Checklist                                   │
│          ↓                                                      │
│  Step 4: User reviews and approves specification               │
│          ↓                                                      │
│  Step 5: User selects execution level (Low/Medium/High)        │
│          ↓                                                      │
│  Step 6: Agent executes with selected autonomy level           │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

---

## 💡 Benefits

| Benefit | Description |
|---------|-------------|
| **Architectural Integrity** | Design reviewed before implementation |
| **Code Quality** | Complete test coverage planned upfront |
| **Security Assurance** | Vulnerabilities caught in planning |
| **Controlled Execution** | No surprise file modifications |
| **Audit Trail** | Full documentation of intent and changes |
| **Scalability** | Works for large, complex agent projects |

---

## 📝 Example: Feature Request → Specification

### Input (4-6 sentences)
```
Add user login system with OAuth (Google, GitHub) and email/password 
authentication. Include email verification for new signups. Users should 
be able to reset their password via email. Store sessions securely and 
support "remember me" functionality.
```

### Output (Specification Summary)

**Acceptance Criteria:**
- [ ] User can sign up with email/password
- [ ] User receives verification email with unique link
- [ ] User can login with Google OAuth
- [ ] User can login with GitHub OAuth
- [ ] User can reset password via email
- [ ] Session persists for 30 days with "remember me"
- [ ] Invalid credentials return 401 with rate limiting

**File Changes:**
| Action | File | LOC Est. |
|--------|------|----------|
| NEW | `src/auth/mod.rs` | ~50 |
| NEW | `src/auth/oauth.rs` | ~150 |
| NEW | `src/auth/email.rs` | ~100 |
| NEW | `src/auth/session.rs` | ~80 |
| MODIFY | `src/routes.rs` | ~30 |
| NEW | `tests/auth_test.rs` | ~200 |

**Security Checklist:**
- [ ] Password hashed with Argon2
- [ ] OAuth tokens never logged
- [ ] Session tokens are cryptographically random
- [ ] Rate limiting on login attempts (5/min)
- [ ] CSRF protection on OAuth flow

---

## 🔗 Related Documents

- [AI Review Roles](./ai-review-roles.md) - Multi-persona review system
- [Pre-merge Checklist](./pre-merge-checklist.md) - Quality gates
- [Development Guidelines](../../standards/development-guidelines.md) - Coding standards

---

*This workflow ensures engineering rigor in AI-assisted development.*
