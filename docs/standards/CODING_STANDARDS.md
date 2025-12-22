# Coding Standards & Developer Guide

> **Last Updated**: 2025-12-23 | **Version**: 2.0

This document consolidates all coding standards, development guidelines, and delivery requirements.

---

## Table of Contents

- [1. Code Quality Rules](#1-code-quality-rules)
- [2. Commit Guidelines](#2-commit-guidelines)
- [3. Testing Requirements](#3-testing-requirements)
- [4. Delivery Checklist](#4-delivery-checklist)
- [5. Quick Reference](#5-quick-reference)
- [6. Common Commands](#6-common-commands)

---

## 1. Code Quality Rules

### 1.1 Language Requirements

> **CRITICAL**: All code must be written in **English only**.

- ✅ Comments in English
- ✅ Variable/function names in English
- ✅ Error messages in English
- ✅ Documentation in English
- ❌ No Chinese characters in source code (`.rs`, `.py`, `.sh`, etc.)

### 1.2 Rust Code Standards

- Follow [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- Use `cargo fmt` for formatting
- Use `cargo clippy -- -D warnings` for linting
- Prefer `Result<T, E>` over `panic!`/`unwrap()`

```rust
// ✅ Good
pub fn connect(url: &str) -> Result<Database, sqlx::Error> {
    let pool = PgPoolOptions::new().connect(url).await?;
    Ok(Database { pool })
}

// ❌ Bad
pub fn connect(url: &str) -> Database {
    let pool = PgPoolOptions::new().connect(url).await.unwrap();
    Database { pool }
}
```

### 1.3 Error Handling

- Use `Result<T, E>` for fallible operations
- Provide meaningful error messages
- Use `?` operator for error propagation
- Avoid bare `unwrap()` calls

### 1.4 Performance Considerations

- Avoid unnecessary `clone()`
- Use references for large structs
- Use `async/await` for I/O operations
- Use `Arc` for shared read-only data

---

## 2. Commit Guidelines

### 2.1 Atomic Commits

Each commit must be **minimal and atomic**:

- **Single responsibility**: One commit = one change
- **Compilable**: Code must compile after each commit
- **Testable**: Related tests must pass
- **Revertable**: Safe to revert without side effects

### 2.2 Commit Message Format

Use [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<scope>): <subject>

<body>

<footer>
```

**Types**: `feat`, `fix`, `docs`, `test`, `refactor`, `perf`, `chore`

**Example**:
```
feat(auth): add Ed25519 signature verification

- Implement verify_ed25519() function
- Add Base62 encoding/decoding for signatures
- Include comprehensive unit tests

Closes #123
```

### 2.3 Pre-Commit Checklist

```bash
cargo build           # Must compile
cargo test            # Tests must pass
cargo clippy -- -D warnings  # No warnings
cargo fmt --check     # Properly formatted
```

---

## 3. Testing Requirements

### 3.1 Coverage Requirements

- **Core functions**: 100% coverage
- **Edge cases**: Error handling, null values, boundary conditions
- **Public APIs**: All `pub` functions/methods

### 3.2 Test Organization

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_function_name() {
        // Arrange
        let input = ...;
        
        // Act
        let result = function(input);
        
        // Assert
        assert_eq!(result, expected);
    }
}
```

### 3.3 Test Markers

```rust
#[test]              // Unit test
#[tokio::test]       // Async test
#[ignore]            // Requires external dependencies
#[should_panic]      // Expected panic
```

---

## 4. Delivery Checklist

### 4.1 Before Commit

- [ ] `cargo build --release` passes
- [ ] `cargo test --lib` passes
- [ ] `cargo clippy -- -D warnings` clean
- [ ] `cargo fmt --check` formatted
- [ ] `mdbook build docs` succeeds (if docs changed)

### 4.2 Documentation Updates

- [ ] README.md updated (if usage/config changed)
- [ ] API docs updated (if endpoints changed)
- [ ] Architecture docs in `docs/src/` updated
- [ ] Code comments for complex logic

### 4.3 Testing Artifacts

- [ ] Unit tests written
- [ ] Integration test script in `scripts/`
- [ ] E2E test method documented

---

## 5. Quick Reference

### 5.1 API Conventions

| Rule | Example |
|------|---------|
| Enum values | `SCREAMING_CASE` (`NEW`, `FILLED`, `BUY`) |
| Numbers | Always strings (`"85000.50"`) |
| Assets | Names, not IDs (`"BTC"` not `1`) |
| Response | `{code, msg, data}` structure |

### 5.2 Naming Conventions

| Context | Convention | Example |
|---------|------------|---------|
| Rust struct fields | snake_case | `user_id` |
| Database columns | snake_case | `user_id` |
| Cross-table fields | Table prefix | `user_flags`, `asset_flags` |
| Chapter numbers | Hex format | `0x0A`, `0x0B` |

### 5.3 Architecture Principles

- **Minimal dependencies** - Logic cohesion
- **Auditability** - Complete event trail
- **Progressive enhancement** - Keep system runnable
- **Backward compatible** - Reuse core types

---

## 6. Common Commands

### Development

```bash
docker-compose up -d              # Start databases
cargo build                       # Dev build
cargo build --release             # Production build
cargo test                        # Run all tests
cargo fmt                         # Format code
cargo clippy -- -D warnings       # Lint code
```

### Running

```bash
cargo run -- --gateway --env dev  # Gateway mode
cargo run -- --pipeline           # Single-thread pipeline
cargo run -- --pipeline-mt        # Multi-thread pipeline
```

### Documentation

```bash
mdbook build docs                 # Build docs
mdbook serve docs                 # Preview at localhost:3000
```

### Troubleshooting

```bash
# Database issues
docker ps | grep postgres
docker-compose restart postgres

# Build issues
cargo clean
cargo update
cargo build

# Test debugging
RUST_LOG=debug cargo test -- --nocapture
```

---

## Related Documents

| Document | Description |
|----------|-------------|
| [api-conventions.md](api-conventions.md) | API response format |
| [gateway-api.md](gateway-api.md) | HTTP endpoints |
| [naming-convention.md](naming-convention.md) | Naming rules |
| [checklist.md](checklist.md) | Delivery checklist |

---

## References

- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- [Conventional Commits](https://www.conventionalcommits.org/)
- [Rust Testing Guide](https://doc.rust-lang.org/book/ch11-00-testing.html)
