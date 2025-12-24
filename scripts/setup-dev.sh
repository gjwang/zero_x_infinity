#!/bin/bash
# =============================================================================
# Development Environment Setup Script
# =============================================================================
#
# Run this script once after cloning the repository to configure your
# local development environment with team-standard settings.
#
# Usage:
#   ./scripts/setup-dev.sh
#
# =============================================================================

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
HOOKS_DIR="$SCRIPT_DIR/hooks"

echo "========================================"
echo " Development Environment Setup"
echo "========================================"
echo ""

# =============================================================================
# Step 1: Configure Git Hooks Path
# =============================================================================
echo "[1/3] Configuring Git hooks..."

# Set hooks path to versioned directory (team-standard hooks)
git config core.hooksPath scripts/hooks

echo "  ✓ Git hooks path set to: scripts/hooks"
echo "  ✓ All team members will use the same hooks"

# =============================================================================
# Step 2: Verify Rust Toolchain
# =============================================================================
echo ""
echo "[2/3] Verifying Rust toolchain..."

if ! command -v rustc &> /dev/null; then
    echo "  ❌ Rust not found. Install from https://rustup.rs"
    exit 1
fi

RUST_VERSION=$(rustc --version | awk '{print $2}')
echo "  ✓ Rust version: $RUST_VERSION"

# Ensure required components are installed
rustup component add rustfmt clippy 2>/dev/null || true
echo "  ✓ rustfmt and clippy installed"

# =============================================================================
# Step 3: Verify Python Dependencies
# =============================================================================
echo ""
echo "[3/3] Verifying Python dependencies..."

if ! command -v python3 &> /dev/null; then
    echo "  ⚠ Python3 not found. Some test scripts may not work."
else
    PYTHON_VERSION=$(python3 --version | awk '{print $2}')
    echo "  ✓ Python version: $PYTHON_VERSION"
    
    # Check for required packages
    MISSING_PKGS=""
    for pkg in pandas pynacl requests psycopg2; do
        if ! python3 -c "import $pkg" 2>/dev/null; then
            MISSING_PKGS="$MISSING_PKGS $pkg"
        fi
    done
    
    if [ -n "$MISSING_PKGS" ]; then
        echo "  ⚠ Missing Python packages:$MISSING_PKGS"
        echo "  → Run: pip install$MISSING_PKGS"
    else
        echo "  ✓ Required Python packages installed"
    fi
fi

# =============================================================================
# Summary
# =============================================================================
echo ""
echo "========================================"
echo " Setup Complete!"
echo "========================================"
echo ""
echo "Your development environment is now configured with:"
echo "  • Git hooks: Enforces fmt + clippy before push"
echo "  • Rust toolchain: rustfmt + clippy"
echo ""
echo "Before pushing code, these checks will run automatically:"
echo "  1. cargo fmt --check"
echo "  2. cargo clippy -- -D warnings"
echo ""
echo "If you encounter issues, re-run this script."
echo ""
