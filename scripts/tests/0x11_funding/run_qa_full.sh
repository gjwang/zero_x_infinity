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
    if [ -n "$GATEWAY_PID" ]; then
        kill "$GATEWAY_PID" 2>/dev/null || true
        wait "$GATEWAY_PID" 2>/dev/null || true
    else
        pkill -x "zero_x_infinity" || true
    fi
}
trap cleanup EXIT

# Source DB environment variables
source "$PROJECT_ROOT/scripts/lib/db_env.sh"

# 1. Init DB
log "Initializing DB..."
"$PROJECT_ROOT/scripts/db/init.sh" pg

# Wait for DB to be ready
log "Waiting for PostgreSQL to be ready..."
for i in {1..30}; do
    if pg_check; then
        log "PostgreSQL is ready"
        break
    fi
    log "Waiting for PostgreSQL... ($i/30)"
    sleep 2
done

if ! pg_check; then
    error "PostgreSQL failed to become ready."
    exit 1
fi

# 2. Start Gateway
log "Starting Gateway..."
cd "$PROJECT_ROOT"
# Determine Gateway environment arguments
GATEWAY_ARGS="--gateway"
if [ "$CI" = "true" ]; then
    GATEWAY_ARGS="$GATEWAY_ARGS --env ci"
fi

# Use GATEWAY_BINARY if set, otherwise default to debug build
GATEWAY_BIN="${GATEWAY_BINARY:-./target/debug/zero_x_infinity}"
$GATEWAY_BIN $GATEWAY_ARGS > "$GATEWAY_LOG" 2>&1 &
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
