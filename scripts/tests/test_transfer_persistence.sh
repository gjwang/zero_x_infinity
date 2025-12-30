#!/bin/bash
# =============================================================================
# Transfer Persistence E2E Test - One-shot verification
# =============================================================================
# Usage: ./scripts/tests/test_transfer_persistence.sh
# =============================================================================

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
cd "$PROJECT_ROOT"

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
NC='\033[0m'

log() { echo -e "${GREEN}[TEST]${NC} $1"; }
fail() { echo -e "${RED}[FAIL]${NC} $1"; exit 1; }

cleanup() {
    log "Cleaning up..."
    pkill -x zero_x_infinity 2>/dev/null || true
}
trap cleanup EXIT

# 1. Kill any existing gateway
log "Stopping existing gateway..."
lsof -ti:8080 | xargs kill -9 2>/dev/null || true

# 2. Clean WAL data
log "Cleaning WAL data..."
rm -rf data/ubscore/* data/matching/* data/settlement/* 2>/dev/null || true

# 3. Reset databases
log "Resetting databases..."
./scripts/db/init.sh --reset > /dev/null 2>&1 || fail "DB init failed"

# 4. Build
log "Building..."
cargo build --bin zero_x_infinity 2>&1 | tail -n 1

# 5. Start gateway
log "Starting gateway (waiting 60s)..."
./target/debug/zero_x_infinity --gateway --env dev > logs/gateway_test.log 2>&1 &
GATEWAY_PID=$!
sleep 60

# 6. Run test
log "Running transfer persistence test..."
if uv run python3 scripts/tests/verify_total_balance.py; then
    echo ""
    echo -e "${GREEN}════════════════════════════════════════${NC}"
    echo -e "${GREEN}  ✅ TRANSFER PERSISTENCE TEST PASSED   ${NC}"
    echo -e "${GREEN}════════════════════════════════════════${NC}"
    exit 0
else
    echo ""
    echo -e "${RED}════════════════════════════════════════${NC}"
    echo -e "${RED}  ❌ TRANSFER PERSISTENCE TEST FAILED   ${NC}"
    echo -e "${RED}════════════════════════════════════════${NC}"
    echo "Check logs/gateway_test.log for details"
    exit 1
fi
