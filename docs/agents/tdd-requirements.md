# TDD Requirements for QA Testing

> **Test-Driven Development (TDD) Iron Laws for QA Engineers**

---

## 🔴 The Iron Law: Red-Green-Refactor

```
NO PRODUCTION CODE WITHOUT A FAILING TEST FIRST
```

### TDD Cycle

```
1. 🔴 RED - Write Failing Test
   ├─ Write minimal test showing what should happen
   ├─ VERIFY: Watch it fail (MANDATORY)
   └─ Confirm failure is expected, not error

2. 🟢 GREEN - Write Minimal Code  
   ├─ Write simplest code to pass the test
   ├─ Don't add features beyond the test
   └─ VERIFY: Watch it pass (MANDATORY)

3. 🔵 REFACTOR - Clean Up
   ├─ Remove duplication
   ├─ Improve names
   └─ Keep tests green

4. ↻ REPEAT for next feature
```

---

## 🚨 TDD Iron Laws

### Law 1: NO PRODUCTION CODE WITHOUT FAILING TEST FIRST

**Violation Examples:**
- ❌ Writing code before test
- ❌ Adapting existing code while writing tests
- ❌ Keeping code "as reference" 

**Consequence:** **DELETE THE CODE. START OVER.**

### Law 2: NEVER TEST MOCK BEHAVIOR

**Violation Example:**
```python
# ❌ BAD: Testing that mock exists
def test_renders_sidebar(self, mock_sidebar):
    page = Page()
    assert mock_sidebar is not None
```

**Correct:**
```python
# ✅ GOOD: Test real behavior
def test_renders_sidebar(self):
    page = Page()  # Use real sidebar
    assert page.has_navigation()
```

### Law 3: WATCH IT FAIL, WATCH IT PASS

**MANDATORY Steps:**
1. Run test → must FAIL (not ERROR)
2. Write code
3. Run test → must PASS
4. If test passes immediately → you're testing wrong thing

**Why:** If you didn't see it fail, you don't know it tests the right thing.

---

## ❌ Testing Anti-Patterns to Avoid

### Anti-Pattern 1: Testing Mock Behavior ❌

**Bad:**
```python
# Testing the mock, not the code
assert mock.called_once()
```

**Good:**
```python
# Testing actual behavior
assert result == expected_value
```

### Anti-Pattern 2: Test-Only Methods in Production ❌

**Bad:**
```python
class Asset:
    def _test_only_get_internal_state(self):  # ❌
        return self._state
```

**Good:**
```python
# Move to test utilities
def extract_state_for_testing(asset):
    return asset._state
```

### Anti-Pattern 3: Mocking Without Understanding ❌

**Question:** "Do we need to mock this?"

**Good:**
- Mock external APIs (slow, unreliable)
- Mock file I/O in unit tests

**Bad:**
- Mocking internal classes "just to be safe"
- Mocking because test is hard

### Anti-Pattern 4: Incomplete Mocks ❌

**Bad:**
```python
# Mock missing methods real class has
mock_symbol = Mock()
mock_symbol.get_price.return_value = 100
# Real Symbol also has .get_volume() - MISSING
```

**Good:**
```python
# Mock mirrors real API completely
mock_symbol = Mock(spec=Symbol)
mock_symbol.get_price.return_value = 100
mock_symbol.get_volume.return_value = 1000
```

### Anti-Pattern 5: Tests as Afterthought ❌

**Bad:**
```
✅ Implementation complete
❌ No tests written
"Ready for testing"
```

**Good:**
```
TDD Cycle:
1. Write failing test
2. Implement to pass
3. Refactor
4. THEN claim complete
```

---

## ✅ Good Test Checklist

| Quality | ✅ Good | ❌ Bad |
|---------|---------|--------|
| **Minimal** | Tests one thing | `test_validates_email_and_domain_and_whitespace()` |
| **Clear** | Name describes behavior | `test_test1()` |
| **Intent** | Shows desired API | Obscures what code should do |
| **No Mocks** | Tests real behavior | Tests mock calls |

---

## 🚩 Red Flags - STOP and Start Over

If you see ANY of these, **DELETE CODE and restart with TDD**:

- [ ] Code written before test
- [ ] Test passes immediately (didn't see it fail)
- [ ] Can't explain why test failed
- [ ] Testing mock behavior
- [ ] Test-only methods in production code
- [ ] Mocking without understanding why
- [ ] Tests added "later"
- [ ] "I already manually tested it"
- [ ] "Keep code as reference"
- [ ] "Deleting X hours is wasteful"
- [ ] "TDD is dogmatic, I'm being pragmatic"

---

## 📋 TDD Verification Checklist

Before marking any test as complete:

### RED Phase
- [ ] Test written before implementation
- [ ] Test run and FAILED (not errored)
- [ ] Failure message is expected
- [ ] Failure is due to missing feature (not typos)

### GREEN Phase  
- [ ] Minimal code written to pass
- [ ] Test run and PASSED
- [ ] All other tests still pass
- [ ] No warnings or errors in output

### REFACTOR Phase
- [ ] Code cleaned up
- [ ] Tests stayed green during refactor
- [ ] No new behavior added

---

## 🎯 TDD for Bug Fixes

**Example: Empty email accepted (bug)**

### 1. RED - Write Failing Test
```python
def test_rejects_empty_email(self):
    """Bug: System accepts empty email"""
    result = submit_form(email="")
    assert result.error == "Email required"
```

### 2. Verify RED
```bash
$ pytest test_form.py::test_rejects_empty_email
FAIL: Expected 'Email required', got None
```

### 3. GREEN - Fix Bug
```python
def submit_form(email: str):
    if not email.strip():
        return FormResult(error="Email required")
    # ...
```

### 4. Verify GREEN
```bash
$ pytest test_form.py::test_rejects_empty_email
PASS
```

---

## 🔍 When Mock Setup Becomes Too Complex

**Warning Signs:**
- Mock setup longer than test logic
- Mocking everything to make test pass
- Test breaks when mock changes

**Solution:** Consider integration tests with real components

---

## 📖 References

- [TDD Skill](https://github.com/obra/superpowers/blob/main/skills/test-driven-development/SKILL.md)
- [Testing Anti-Patterns](https://github.com/obra/superpowers/blob/main/skills/test-driven-development/testing-anti-patterns.md)

---

**Remember:** TDD is pragmatic, not dogmatic.  
It finds bugs before commit, prevents regressions, documents behavior, and enables refactoring.
