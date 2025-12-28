#!/bin/bash
set -e

# =============================================================================
# QA Full Verification Wrapper for Phase 0x11
# =============================================================================
# 1. Sets up the environment (DB, Gateway)
# 2. Runs the QA Master Verification Suite (run_tests.sh)
# 3. Tears down
# =============================================================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$SCRIPT_DIR/../../.."
LOG_DIR="${PROJECT_ROOT}/logs"
GATEWAY_LOG="${LOG_DIR}/gateway_qa_0x11.log"

mkdir -p "$LOG_DIR"

# Source unified test utilities
source "$PROJECT_ROOT/scripts/lib/test_utils.sh"
# Source DB vars
source "$PROJECT_ROOT/scripts/lib/db_env.sh"

LOG_DIR="${PROJECT_ROOT}/logs"
GATEWAY_LOG="${LOG_DIR}/gateway_qa_0x11.log"

ensure_log_dir "$LOG_DIR"

cleanup() {
    log_info "Cleaning up..."
    if [ -n "$GATEWAY_PID" ]; then
        kill "$GATEWAY_PID" 2>/dev/null || true
        wait "$GATEWAY_PID" 2>/dev/null || true
    fi
}
trap cleanup EXIT

# 1. Init DB
log_info "Initializing DB..."
"$PROJECT_ROOT/scripts/db/init.sh" pg

# Wait for DB
wait_for_postgres

# 2. Start Gateway
cleanup_gateway_process

log_info "Starting Gateway..."
cd "$PROJECT_ROOT"

# Use GATEWAY_BINARY if set, otherwise default to debug build (for QA)
GATEWAY_BIN="${GATEWAY_BINARY:-./target/debug/zero_x_infinity}"
# Redirect to log file
$GATEWAY_BIN $GATEWAY_ARGS > "$GATEWAY_LOG" 2>&1 &
GATEWAY_PID=$!
log_info "Gateway PID: $GATEWAY_PID"

wait_for_gateway 8080

# 3. Run QA Suite
log "Executing QA Master Suite..."
cd "$SCRIPT_DIR"
./run_tests.sh
