#!/bin/sh
#
# install_hooks.sh
# Installs git hooks to ensure quality before pushing.

HOOK_DIR=".git/hooks"
PRE_PUSH="$HOOK_DIR/pre-push"

echo "Installing git hooks..."

# Ensure hooks dir exists (it should in a git repo)
mkdir -p "$HOOK_DIR"

# Create pre-push hook
cat > "$PRE_PUSH" << 'EOF'
#!/bin/sh
#
# pre-push hook to enforce basic quality checks
#
# This hook runs:
# 1. cargo fmt --check (formatting)
# 2. cargo clippy (semantics/lints)
# 3. cargo test --workspace --no-run (compilation of tests)
#
# If any of these fail, the push will be aborted.

echo "🔍 Running pre-push checks..."

# 1. Formatting
echo "[1/3] cargo fmt --check"
if ! cargo fmt --all -- --check; then
    echo "❌ Code not formatted. Run 'cargo fmt' and commit."
    exit 1
fi

# 2. Clippy
echo "[2/3] cargo clippy -- -D warnings"
if ! cargo clippy --all-targets --all-features -- -D warnings; then
    echo "❌ Clippy warnings detected."
    exit 1
fi

# 3. Test Compilation (covers the issue we faced)
echo "[3/3] cargo test --workspace --no-run"
if ! cargo test --workspace --no-run; then
    echo "❌ Test compilation failed."
    exit 1
fi

echo "✅ All checks passed!"
exit 0
EOF

chmod +x "$PRE_PUSH"

echo "✅ Pre-push hook installed at $PRE_PUSH"
