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

GREEN='\033[0;32m'
RED='\033[0;31m'
NC='\033[0m'

log() { echo -e "${GREEN}[QA-WRAPPER]${NC} $1"; }
error() { echo -e "${RED}[ERROR]${NC} $1"; }

cleanup() {
    log "Cleaning up Gateway..."
    pkill -x "zero_x_infinity" || true
}
trap cleanup EXIT

# 1. Init DB
log "Initializing DB..."
"$PROJECT_ROOT/scripts/db/init.sh" pg

# 2. Start Gateway
log "Starting Gateway..."
cd "$PROJECT_ROOT"
# Use GATEWAY_BINARY if set, otherwise default to debug build
GATEWAY_BIN="${GATEWAY_BINARY:-./target/debug/zero_x_infinity}"
$GATEWAY_BIN --gateway > "$GATEWAY_LOG" 2>&1 &
GATEWAY_PID=$!

log "Waiting for Gateway..."
for i in $(seq 1 30); do
    if curl -s http://localhost:8080/api/v1/health >/dev/null; then
        log "Gateway UP!"
        break
    fi
    sleep 1
done

if ! curl -s http://localhost:8080/api/v1/health >/dev/null; then
    error "Gateway failed to start. Logs:"
    tail -n 20 "$GATEWAY_LOG"
    exit 1
fi

# 3. Run QA Suite
log "Executing QA Master Suite..."
cd "$SCRIPT_DIR"
./run_tests.sh
