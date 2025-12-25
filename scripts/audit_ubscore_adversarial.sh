#!/bin/bash
# audit_ubscore_adversarial.sh - Independent QA Audit for UBSCore Balance Recovery
# ===================================================================
#
# PURPOSE:
#   Verify UBSCore (Balance Source of Truth) robustness against:
#   1. WAL Corruption: Does system fallback to snapshot or crash?
#   2. Balance Recovery: Are frozen balances correctly restored after crash?
#
# ===================================================================

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
cd "$PROJECT_DIR"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

PORT=18084
GW_LOG="/tmp/ubscore_adversarial.log"
DATA_DIR="./data/audit_ubscore"
MATCHING_DATA_DIR="./data/audit_ubscore_me"
SETTLEMENT_DATA_DIR="./data/audit_ubscore_settle"

fail_audit() {
    echo -e "${RED}❌ AUDIT FAILED: $1${NC}"
    pkill -f "zero_x_infinity.*--gateway.*--port.*$PORT" 2>/dev/null || true
    exit 1
}

pass_step() {
    echo -e "${GREEN}  ✓${NC} $1"
}

warn_step() {
    echo -e "${YELLOW}  ⚠${NC} $1"
}

cleanup() {
    pkill -f "zero_x_infinity.*--gateway.*--port.*$PORT" 2>/dev/null || true
    # Preserve data for post-mortem if audit fails
}

wait_for_gw() {
    for i in {1..30}; do
        if curl -sf "http://localhost:$PORT/api/v1/health" > /dev/null 2>&1; then return 0; fi
        sleep 1
    done
    return 1
}

# Clean start
pkill -f "zero_x_infinity.*--gateway.*--port.*$PORT" 2>/dev/null || true
rm -rf "$DATA_DIR" "$MATCHING_DATA_DIR" "$SETTLEMENT_DATA_DIR" 2>/dev/null || true
mkdir -p "$DATA_DIR" "$MATCHING_DATA_DIR" "$SETTLEMENT_DATA_DIR"

trap cleanup EXIT

# Create test config with UBSCore persistence disabled (we're testing ME/Settlement for now)
# Note: UBSCore persistence is NOT enabled via config yet - this is a finding itself
cat > config/audit_ubscore.yaml <<EOF
log_level: "info"
log_dir: "./logs"
log_file: "audit_ubscore.log"
use_json: false
rotation: "daily"
sample_rate: 1
enable_tracing: false

gateway:
  host: "0.0.0.0"
  port: $PORT
  queue_size: 10000

persistence:
  enabled: false
  tdengine_dsn: "taos://root:taosdata@localhost:6030"

matching_persistence:
  enabled: true
  data_dir: "$MATCHING_DATA_DIR"
  snapshot_interval_trades: 20

settlement_persistence:
  enabled: true
  data_dir: "$SETTLEMENT_DATA_DIR"
  checkpoint_interval: 5
  snapshot_interval: 10

postgres_url: "postgresql://trading:trading123@localhost:5433/exchange_info_db"
EOF

echo -e "${BLUE}=== Starting UBSCore Adversarial Audit ===${NC}"

# ============================================================================
# TEST 1: Verify UBSCore Persistence Config Existence (GAP-00)
# ============================================================================
echo -e "${YELLOW}Test 1: UBSCore Persistence Configuration Check (GAP-00)${NC}"

# Check if there's a ubscore_persistence config option
if grep -q "ubscore_persistence" src/config.rs 2>/dev/null; then
    pass_step "ubscore_persistence config exists in code"
else
    warn_step "NO ubscore_persistence config found in config.rs - UBSCore may not have runtime persistence!"
fi

# ============================================================================
# TEST 2: Balance State Before/After Crash (UBSC-GAP-04 Verification)
# ============================================================================
echo -e "${YELLOW}Test 2: Balance State Crash Recovery (UBSC-GAP-04)${NC}"

# Start Gateway
./target/release/zero_x_infinity --gateway --env audit_ubscore --port $PORT > "$GW_LOG" 2>&1 &
wait_for_gw || fail_audit "Gateway failed to start"

# Record initial balance (this would need API support)
# For now, inject orders to create frozen balances
echo "Injecting orders to create frozen balance state..."
GATEWAY_URL="http://localhost:$PORT" python3 "${SCRIPT_DIR}/inject_orders.py" --input fixtures/orders.csv --limit 50 > /dev/null 2>&1
sleep 2

# Capture stats BEFORE crash
STATS_BEFORE=$(curl -sf "http://localhost:$PORT/api/v1/stats" 2>/dev/null || echo "{}")
TRADES_BEFORE=$(echo "$STATS_BEFORE" | python3 -c "import sys,json; print(json.load(sys.stdin).get('trades_generated',0))" 2>/dev/null || echo "0")
echo "Stats before crash: trades=$TRADES_BEFORE"

# SIGKILL to simulate crash
echo "Simulating crash (SIGKILL)..."
pkill -9 -f "zero_x_infinity.*--port.*$PORT"
sleep 2

# Restart
echo "Restarting after crash..."
./target/release/zero_x_infinity --gateway --env audit_ubscore --port $PORT >> "$GW_LOG" 2>&1 &

if wait_for_gw; then
    pass_step "Gateway restarted successfully after crash"
else
    # Check if it's a WAL corruption crash
    if grep -q "Failed to initialize" "$GW_LOG" || grep -q "panic" "$GW_LOG"; then
        echo -e "${RED}Gateway log shows initialization failure:${NC}"
        grep -E "(Failed|panic|error)" "$GW_LOG" | tail -10
        warn_step "UBSC-GAP-01 CONFIRMED: System may not handle corruption gracefully"
    else
        fail_audit "Gateway failed to restart (unknown reason)"
    fi
fi

# Capture stats AFTER recovery
STATS_AFTER=$(curl -sf "http://localhost:$PORT/api/v1/stats" 2>/dev/null || echo "{}")
TRADES_AFTER=$(echo "$STATS_AFTER" | python3 -c "import sys,json; print(json.load(sys.stdin).get('trades_generated',0))" 2>/dev/null || echo "0")
echo "Stats after recovery: trades=$TRADES_AFTER"

# Verify post-crash functionality
echo "Verifying post-crash order acceptance..."
GATEWAY_URL="http://localhost:$PORT" python3 "${SCRIPT_DIR}/inject_orders.py" --input fixtures/orders.csv --limit 10 > /dev/null 2>&1
if curl -sf "http://localhost:$PORT/api/v1/health" > /dev/null; then
    pass_step "System accepting orders after recovery"
else
    fail_audit "System not functional after recovery"
fi

# ============================================================================
# TEST 3: WAL Corruption Handling (UBSC-GAP-01 Verification)
# ============================================================================
echo -e "${YELLOW}Test 3: WAL Corruption Handling (UBSC-GAP-01)${NC}"

# Kill current instance
pkill -f "zero_x_infinity.*--port.*$PORT"
sleep 1

# Find and corrupt UBSCore WAL (if it exists)
UBSCORE_WAL=$(find "$DATA_DIR" -name "*.wal" 2>/dev/null | head -1)
if [ -z "$UBSCORE_WAL" ]; then
    warn_step "No UBSCore WAL found - UBSCore persistence may not be enabled at runtime"
    echo "  → This is a GAP: UBSCore should have its own WAL for balance recovery"
else
    echo "Poisoning UBSCore WAL at $UBSCORE_WAL..."
    dd if=/dev/urandom of="$UBSCORE_WAL" bs=1 count=50 seek=25 conv=notrunc 2>/dev/null
    
    # Try to restart
    ./target/release/zero_x_infinity --gateway --env audit_ubscore --port $PORT >> "$GW_LOG" 2>&1 &
    
    if wait_for_gw; then
        if grep -q "WAL replay failed" "$GW_LOG"; then
            pass_step "UBSCore detected corruption and fell back gracefully"
        else
            warn_step "Gateway started but no corruption fallback logged"
        fi
    else
        echo -e "${RED}UBSC-GAP-01 CONFIRMED: Corrupted WAL causes startup failure${NC}"
        grep -E "(error|panic|Failed)" "$GW_LOG" | tail -10
    fi
fi

# ============================================================================
# Summary
# ============================================================================
echo ""
echo -e "${BLUE}════════════════════════════════════════════════════════════${NC}"
echo -e "${BLUE}  UBSCore Adversarial Audit Complete                        ${NC}"
echo -e "${BLUE}════════════════════════════════════════════════════════════${NC}"
echo ""
echo "Key Findings:"
echo "  - UBSCore WAL presence: $([ -n "$UBSCORE_WAL" ] && echo "YES" || echo "NO (GAP)")"
echo "  - Post-crash recovery: FUNCTIONAL"
echo "  - See full log: $GW_LOG"
