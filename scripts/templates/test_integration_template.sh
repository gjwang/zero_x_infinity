#!/bin/bash
# =============================================================================
# [TEMPLATE] Integration Test Script Standard
# =============================================================================
#
# PURPOSE:
#   Reference implementation for new integration tests.
#   Copy this file when creating `scripts/test_new_feature.sh`.
#
# KEY FEATURES:
#   1. CI/Dev Environment Switching (Fixes 5433 vs 5432 port mismatch)
#   2. Safe Process Management (No `pkill -f`, uses PID tracking)
#   3. Robust Readiness Checks (Wait loops for Gateway & DB)
#   4. Database Cleaning (Idempotency)
#   5. uv-based Python execution
#
# USAGE:
#   cp scripts/templates/test_integration_template.sh scripts/test_my_feature.sh
#   chmod +x scripts/test_my_feature.sh
#
# =============================================================================

set -e

# 1. Setup Paths
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Robust Project Root Detection
if [ -f "$SCRIPT_DIR/../../Cargo.toml" ]; then
    PROJECT_DIR="$(cd "$SCRIPT_DIR/../../" && pwd)"
elif [ -f "$SCRIPT_DIR/../Cargo.toml" ]; then
    PROJECT_DIR="$(cd "$SCRIPT_DIR/../" && pwd)"
else
    echo "ERROR: Could not locate project root (Cargo.toml). Please adjust PROJECT_DIR in script."
    exit 1
fi
cd "$PROJECT_DIR"

# 2. Global Constants
# Default to 5433 (Dev), CI will override to 5432 via config/ci.yaml
BASE_URL="http://localhost:8080"
TEST_TIMEOUT=180

# Colors for Logging
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m'

# Logging Helpers
log_info() { echo -e "${GREEN}[INFO]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_err()  { echo -e "${RED}[ERROR]${NC} $1"; }

# 3. Cleanup Trap
# Ensures Gateway is killed even if test fails
cleanup() {
    if [ -n "$GW_PID" ]; then
        log_info "Stopping Gateway (PID $GW_PID)..."
        kill "$GW_PID" 2>/dev/null || true
        wait "$GW_PID" 2>/dev/null || true
    fi
}
trap cleanup EXIT

echo "╔════════════════════════════════════════════════════════════╗"
echo "║    Integration Test: [Feature Name]                       ║"
echo "╚════════════════════════════════════════════════════════════╝"

# =============================================================================
# Step 1: Environment Preparation
# =============================================================================
log_info "Step 1: Preparing Environment"

# 1.1 Stop existing Gateway (Safe Method)
# DO NOT use `pkill -f zero_x_infinity` - it kills the IDE!
EXISTING_PID=$(pgrep -f "target/release/zero_x_infinity" | head -1)
if [ -n "$EXISTING_PID" ]; then
    log_warn "Stopping existing Gateway (PID $EXISTING_PID)..."
    kill "$EXISTING_PID" 2>/dev/null || true
    sleep 2
fi

# 1.2 Check Port Availability
if lsof -Pi :8080 -sTCP:LISTEN -t >/dev/null ; then
    log_err "Port 8080 is still in use! Please free it manually."
    exit 1
fi

# =============================================================================
# Step 2: Database Initialization (Optional)
# =============================================================================
# log_info "Step 2: Resetting Database..."
# ./scripts/db/init.sh pg  # Use standard init scripts

# =============================================================================
# Step 3: Start Gateway
# =============================================================================
log_info "Step 3: Starting Gateway"

# CRITICAL: Switch Config based on CI Environment
# CI uses config/ci.yaml (Port 5432)
# Dev uses config/dev.yaml (Port 5433)
GATEWAY_ARGS="--gateway"
if [ "$CI" = "true" ]; then
    log_info "CI Environment Detected: Using 'ci' profile"
    GATEWAY_ARGS="$GATEWAY_ARGS --env ci"
else
    log_info "Dev Environment Detected: Using default profile"
    # Default is 'dev', no flag needed, or explicit --env dev
fi

# Build binary (skip in CI to save time)
if [ "$CI" != "true" ] && [ ! -f "./target/release/zero_x_infinity" ]; then
    log_info "Building release binary..."
    cargo build --release --quiet
fi

# Start Process
nohup ./target/release/zero_x_infinity $GATEWAY_ARGS > /tmp/gateway_test.log 2>&1 &
GW_PID=$!
log_info "Gateway started with PID $GW_PID"

# Wait for Readiness (Health Check Loop)
log_info "Waiting for Gateway readiness..."
READY=false
for i in {1..30}; do
    if curl -s "${BASE_URL}/api/v1/health" | grep -q "ok"; then
        READY=true
        log_info "Gateway is READY!"
        break
    fi
    sleep 1
done

if [ "$READY" = "false" ]; then
    log_err "Gateway failed to start!"
    cat /tmp/gateway_test.log
    exit 1
fi

# =============================================================================
# Step 4: Run Tests
# =============================================================================
log_info "Step 4: Running Test Logic"

# Example: Run Python test script using uv
# export SCRIPT_DIR for python scripts to locate resources
export SCRIPT_DIR

if command -v uv >/dev/null; then
    # Run python script with dependencies
    uv run --with requests --with pynacl python3 scripts/tests/my_test_script.py
else
    # Fallback for systems without uv (not recommended for Agents)
    python3 scripts/tests/my_test_script.py
fi

# =============================================================================
# Success
# =============================================================================
log_info "✅ Test Passed Successfully"
exit 0
