#!/bin/bash
# Setup script: installs git hooks
# Run: ./scripts/setup-hooks.sh

echo "Installing git hooks..."

# Link pre-commit hook
ln -sf ../../scripts/hooks/pre-commit .git/hooks/pre-commit

echo "✅ Git hooks installed"
echo ""
echo "Hooks will run:"
echo "  - pre-commit: fmt check, cargo test, cargo clippy (same flags as CI)"
