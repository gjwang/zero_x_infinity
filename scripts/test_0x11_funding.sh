#!/bin/bash
set -e

# =============================================================================
# 0x11 Funding Flow Verification Script (One-Click)
# =============================================================================
# Automates:
# 1. Database Initialization (PostgreSQL)
# 2. Gateway Startup
# 3. Execution of verify_funding_flow.py
# 4. Cleanup
# =============================================================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

# Source Unified Utilities
source "${SCRIPT_DIR}/lib/test_utils.sh"

# Setup Environment
detect_ci_env

# Define Log File (Absolute Path)
GATEWAY_LOG="${LOG_DIR}/gateway_0x11.log"

# Cleanup Trap
cleanup() {
    log_info "Cleaning up..."
    cleanup_gateway_process
}
trap cleanup EXIT

# 1. Initialize Database
log_info "Initializing Database (PostgreSQL)..."
"${SCRIPT_DIR}/db/init.sh" pg

# 2. Build (Ensure fresh binary)
log_info "Building Binary..."
cd "${PROJECT_ROOT}"
cargo build --bin zero_x_infinity

# 3. Start Gateway
log_info "Starting Gateway..."
# Source DB env vars
source "${SCRIPT_DIR}/lib/db_env.sh"

# Start in background
./target/debug/zero_x_infinity --gateway ${GATEWAY_ARGS} > "${GATEWAY_LOG}" 2>&1 &
GATEWAY_PID=$!

# Wait for Health Check
wait_for_gateway "${BASE_URL}"

# 4. Run Verification (Python)
log_info "Running Verification Logic..."
if command -v uv >/dev/null 2>&1; then
    uv run "${SCRIPT_DIR}/verify_funding_flow.py"
else
    python3 "${SCRIPT_DIR}/verify_funding_flow.py"
fi

log_success "One-Click Verification PASSED!"
