#!/bin/bash
# =============================================================================
# Phase 0x14-b: Order Commands Complete Functional QA Test
# =============================================================================
# Automates:
# 1. Database Initialization (PostgreSQL & TDengine)
# 2. Gateway Startup (Matching Engine in CI mode)
# 3. Execution of QA Test Suite (IOC, Reduce, Move, etc.)
# 4. Cleanup
# =============================================================================

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

# Source Unified Utilities
source "${SCRIPT_DIR}/lib/test_utils.sh"
source "${SCRIPT_DIR}/lib/test_helpers.sh"

# Setup Environment
detect_ci_env

# Define Log File
GATEWAY_LOG="${LOG_DIR}/gateway_0x14b_qa.log"

# Cleanup Trap
cleanup() {
    log_info "Cleaning up..."
    cleanup_gateway_process
}
trap cleanup EXIT

# 1. Initialize Database
log_info "Initializing Databases (PostgreSQL & TDengine)..."
"${SCRIPT_DIR}/db/init.sh" --reset

# 2. Build (Ensure fresh binary)
if [ "$CI" != "true" ]; then
    log_info "Building Binary..."
    cd "${PROJECT_ROOT}"
    cargo build --release --bin zero_x_infinity
fi

# 3. Start Gateway
log_info "Starting Gateway..."
source "${SCRIPT_DIR}/lib/db_env.sh"

# Ensure data directory exists for WAL
mkdir -p "${PROJECT_ROOT}/data/ubscore/wal"
mkdir -p "${PROJECT_ROOT}/data/matching-service"

# Start in background
./target/release/zero_x_infinity --gateway ${GATEWAY_ARGS} > "${GATEWAY_LOG}" 2>&1 &
GW_PID=$!

# Wait for Health Check
wait_for_gateway "${GATEWAY_PORT}"

# 4. Run QA Test Suite (Python)
log_info "Running QA 0x14-b functional test suite..."
cd "${PROJECT_ROOT}"
if command -v uv >/dev/null 2>&1; then
    uv run python3 "scripts/tests/0x14b_matching/run_all_qa_tests.py"
else
    python3 "scripts/tests/0x14b_matching/run_all_qa_tests.py"
fi

TEST_EXIT_CODE=$?

if [ $TEST_EXIT_CODE -eq 0 ]; then
    log_success "QA 0x14-b Functional Verification PASSED! ✅"
else
    log_err "QA 0x14-b Functional Verification FAILED! ❌"
    echo "Check logs at: ${GATEWAY_LOG}"
fi

exit $TEST_EXIT_CODE
