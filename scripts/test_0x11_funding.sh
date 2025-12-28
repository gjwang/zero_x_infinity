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
LOG_DIR="${PROJECT_ROOT}/logs"
GATEWAY_LOG="${LOG_DIR}/gateway_0x11.log"

mkdir -p "$LOG_DIR"

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
NC='\033[0m'

log() { echo -e "${GREEN}[TEST]${NC} $1"; }
error() { echo -e "${RED}[ERROR]${NC} $1"; }

# Cleanup Function
cleanup() {
    log "Cleaning up..."
    pkill -x "zero_x_infinity" || true
}
trap cleanup EXIT

# 1. Initialize Database
log "Initializing Database (PostgreSQL)..."
"$SCRIPT_DIR/db/init.sh" pg

# 2. Build (Ensure fresh binary)
log "Building Binary..."
cd "$PROJECT_ROOT"
cargo build --bin zero_x_infinity

# 3. Start Gateway
log "Starting Gateway..."
# Source env vars
source "$SCRIPT_DIR/lib/db_env.sh"

# Start in background
./target/debug/zero_x_infinity --gateway > "$GATEWAY_LOG" 2>&1 &
GATEWAY_PID=$!

# Wait for Health Check
log "Waiting for Gateway to be ready..."
MAX_RETRIES=30
for i in $(seq 1 $MAX_RETRIES); do
    if curl -s http://localhost:8080/api/v1/health >/dev/null; then
        log "Gateway is READY!"
        break
    fi
    if ! kill -0 $GATEWAY_PID 2>/dev/null; then
        error "Gateway process died! Check logs:"
        tail -n 20 "$GATEWAY_LOG"
        exit 1
    fi
    echo -n "."
    sleep 2
done

if ! curl -s http://localhost:8080/api/v1/health >/dev/null; then
    error "Gateway failed to start within timeout."
    tail -n 20 "$GATEWAY_LOG"
    exit 1
fi

# 4. Run Verification (Python)
log "Running Verification Logic..."
if command -v uv >/dev/null 2>&1; then
    uv run "$SCRIPT_DIR/verify_funding_flow.py"
else
    python3 "$SCRIPT_DIR/verify_funding_flow.py"
fi

echo ""
log "✅ One-Click Verification PASSED!"
