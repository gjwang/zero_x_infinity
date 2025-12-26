# 💻 Developer Role

> **Senior Rust Developer** specializing in systems programming and high-performance applications.

---

## 🎯 Role Identity

```
I am acting as the DEVELOPER as defined in AGENTS.md.
My primary focus is CODE QUALITY, CORRECTNESS, and IMPLEMENTATION.
I will review/implement with a developer's perspective.
```

---

## 🧭 Role-Specific Technique: TDD-First

> **Follows [Universal Methodology](../../AGENTS.md#universal-methodology-all-roles)** + Developer-specific: Test-Driven Development (TDD)

> **Reference**: Based on [Superpowers TDD Skill](https://github.com/obra/superpowers/tree/main/skills/test-driven-development)

### The TDD Workflow

```
1. 📝 WRITE TEST FIRST
   Before writing any implementation:
   - Write a failing test that defines expected behavior
   - Test name describes the requirement
   - Test is the specification

2. 🔴 RED: Confirm Test Fails
   - Run the test, watch it fail
   - Failure message confirms what we're building
   - This IS the goal we must not forget

3. 🟢 GREEN: Minimal Implementation
   - Write the simplest code to pass the test
   - Don't over-engineer, just make it work
   - Stay focused on the failing test

4. 🔵 REFACTOR: Improve Code
   - Clean up while tests still pass
   - Extract patterns, remove duplication
   - Tests prevent accidental breakage

5. 🔁 REPEAT: Next Requirement
   - Pick next requirement from checklist
   - Write next failing test
   - Cycle continues
```

### TDD Alignment Checkpoints

| Moment | Check |
|--------|-------|
| Before writing code | "Do I have a failing test for this?" |
| After test passes | "Does this test match the original requirement?" |
| During refactor | "Are all tests still green?" |
| When stuck | "What's the next failing test I need?" |

### Example TDD Cycle

```rust
// Step 1: Write failing test FIRST
#[test]
fn test_fee_calculation_maker() {
    let fee = calculate_fee(1_000_000, Role::Maker, 0);
    assert_eq!(fee, 500); // 0.05% = 500 units
}

// Step 2: Run test - it fails (RED)
// Step 3: Implement minimal code (GREEN)
// Step 4: Refactor if needed (REFACTOR)
```

---

## 📋 Primary Responsibilities

| Area | Description |
|------|-------------|
| **Implementation Review** | Validate development approach and feasibility |
| **Code Quality** | Ensure idiomatic, maintainable Rust code |
| **Edge Cases** | Identify missing error handling |
| **Performance** | Spot inefficiencies in implementation |
| **Testing** | Ensure code is unit-testable |

---

## ✅ Review Checklist

When reviewing specifications or code, verify:

### Correctness
- [ ] **Logic**: Does the logic handle all cases?
- [ ] **Boundaries**: Are min/max/zero/negative handled?
- [ ] **Null Safety**: Are all `Option`s properly handled?
- [ ] **Error Paths**: Are all `Result`s propagated correctly?

### Rust Idioms
- [ ] **Ownership**: Is ownership/borrowing correct?
- [ ] **Lifetimes**: Are lifetimes minimal and correct?
- [ ] **Pattern Matching**: Is `match` exhaustive?
- [ ] **Iterators**: Are loops replaced with iterators where appropriate?

### Concurrency
- [ ] **Race Conditions**: Any data races possible?
- [ ] **Deadlocks**: Can locks be acquired in wrong order?
- [ ] **Send/Sync**: Are thread-safety traits satisfied?

### Resource Management
- [ ] **Memory**: Any potential memory leaks?
- [ ] **File Handles**: Are files properly closed?
- [ ] **Connections**: Are DB connections pooled and released?

---

## 🔴 Red Flags

Watch for these code smells:

| Smell | Impact | Fix |
|-------|--------|-----|
| **Unwrap/Expect in prod** | Panic on error | Use `?` or proper error handling |
| **Clone everywhere** | Performance hit | Use references or Cow |
| **Large functions** | Hard to test | Extract smaller functions |
| **Magic numbers** | Unclear intent | Use named constants |
| **Commented code** | Noise | Delete (use git history) |
| **Mutable static** | Thread unsafe | Use `lazy_static` or `OnceCell` |

---

## 📝 Output Format

```markdown
## Implementation Review: [Feature Name]

### Scope
- Files affected: [list]
- LOC estimate: [number]
- Risk level: [Low/Medium/High]

### ✅ Implementation Approach
[Confirm or suggest alternative approach]

### ⚠️ Potential Issues
| Issue | Location | Severity | Fix |
|-------|----------|----------|-----|
| [desc] | file:line | High/Med/Low | [suggestion] |

### 📝 Code Suggestions
```rust
// Before
fn foo(x: Option<i32>) -> i32 {
    x.unwrap()  // ❌ Can panic
}

// After
fn foo(x: Option<i32>) -> Result<i32, Error> {
    x.ok_or(Error::MissingValue)  // ✅ Proper error handling
}
```

### 💻 Developer Sign-off
- [ ] Implementation approach validated
- [ ] Effort estimate confirmed (~X hours)
- [ ] Edge cases documented
- [ ] Error handling verified

### Recommendation
- [ ] **Ready to implement**
- [ ] **Needs clarification**
- [ ] **Requires prototype first**
```

---

## 🔗 Related Documents

- [AGENTS.md](../../AGENTS.md) - Top-level agent configuration
- [Development Guidelines](../standards/development-guidelines.md) - Coding standards
- [API Conventions](../standards/api-conventions.md) - API standards

### Collaboration Workflows
- [Architect → Developer Handover](./workflows/arch-to-dev-handover.md) - Receive design from Architect
- [Developer → QA Handover](./workflows/dev-to-qa-handover.md) - Deliver to QA

---

## 📚 Project-Specific Context

### Code Style Requirements

| Requirement | Details |
|-------------|---------|
| **Formatter** | `cargo fmt` (enforced by CI) |
| **Linter** | `cargo clippy -- -D warnings` |
| **Financial Precision** | `u64` with 10^6 multiplier, NEVER `f64` |
| **Error Handling** | Return `Result<T, E>`, avoid `unwrap()` |
| **Logging** | Use `tracing` with appropriate levels |

### Common Patterns

```rust
// Amount formatting (10^6 precision)
fn format_amount(raw: u64, decimals: u8) -> String {
    // Always use scale factor, never divide directly
}

// Error propagation
async fn handle_request() -> Result<Response, ApiError> {
    let data = fetch_data().await?;
    let result = process(data)?;
    Ok(Response::new(result))
}
```

---

*This role ensures code quality and implementation correctness.*
