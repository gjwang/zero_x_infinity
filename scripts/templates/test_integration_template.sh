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



# ==============================================================================
# 2. Setup Environment
# ==============================================================================
# Source unified test utilities for shared checks
source "$PROJECT_DIR/scripts/lib/test_utils.sh"

# Define Log Directory (Absolute)
LOG_DIR="${PROJECT_DIR}/logs"
# Export for test_utils.sh to use if needed
export LOG_DIR

# Ensure log directory
ensure_log_dir "$LOG_DIR"

# Wait for DB
wait_for_postgres

# Cleanup any existing instances
cleanup_gateway_process

# ==============================================================================
# 3. Start Gateway
# ==============================================================================
echo "[TEST] Starting Gateway..."
export RUST_LOG=info,gateway=debug,zero_x_infinity=debug

# Config parities handled by test_utils.sh (detect_ci_env -> GATEWAY_ARGS)

CMD="${GATEWAY_BINARY:-./target/release/zero_x_infinity}"
# Redirect stdout/stderr to log file
$CMD $GATEWAY_ARGS > "$LOG_FILE" 2>&1 &
GATEWAY_PID=$!
echo "[TEST] Gateway PID: $GATEWAY_PID"

# Wait for readiness
if ! wait_for_gateway 8080; then
    echo "Logs:"
    tail -n 20 "$LOG_FILE"
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
