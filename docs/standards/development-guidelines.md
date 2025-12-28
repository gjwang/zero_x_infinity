# 开发规范 (Development Guidelines)

> **Core Principle**: Standardize environments to eliminate "works on my machine" issues.

---

## 🐍 Python Environment

We use **uv** for strict dependency management and execution speed.

### 1. The Golden Rule
**NEVER** use system `python3` or `pip` directly for project scripts.
**ALWAYS** use `uv run` to execute scripts.

### 2. Standard Workflow
```bash
# 1. Sync dependencies (like npm install)
uv sync

# 2. Run script (like npm run)
uv run python3 scripts/my_script.py
```

### 3. Adding Dependencies
```bash
# Add new package
uv add requests
```

---

## 🦀 Rust Environment

- **Format**: `cargo fmt` must pass.
- **Lint**: `cargo clippy` must pass (no warnings).
- **Tests**: `cargo test` must pass.
