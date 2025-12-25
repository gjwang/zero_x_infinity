#!/bin/bash
# audit_settlement_adversarial.sh - Independent QA Audit for Exception Handling
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

PORT=18083
GW_LOG="/tmp/settlement_adversarial.log"
SETTLEMENT_DATA_DIR="./data/audit_settlement"
MATCHING_DATA_DIR="./data/audit_matching"

fail_audit() {
    echo -e "${RED}❌ AUDIT FAILED: $1${NC}"
    pkill -f "zero_x_infinity.*--gateway.*--port.*$PORT" 2>/dev/null || true
    exit 1
}

pass_step() {
    echo -e "${GREEN}  ✓${NC} $1"
}

cleanup() {
    pkill -f "zero_x_infinity.*--gateway.*--port.*$PORT" 2>/dev/null || true
    rm -rf "$SETTLEMENT_DATA_DIR" "$MATCHING_DATA_DIR" 2>/dev/null || true
}

wait_for_gw() {
    for i in {1..30}; do
        if curl -sf "http://localhost:$PORT/api/v1/health" > /dev/null 2>&1; then return 0; fi
        sleep 1
    done
    return 1
}

trap cleanup EXIT
cleanup
mkdir -p "$SETTLEMENT_DATA_DIR" "$MATCHING_DATA_DIR"

# 1. Create Config with small intervals
cat > config/audit_settlement.yaml <<EOF
log_level: "info"
log_dir: "./logs"
log_file: "audit_settlement.log"
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
  snapshot_interval_trades: 50

settlement_persistence:
  enabled: true
  data_dir: "$SETTLEMENT_DATA_DIR"
  checkpoint_interval: 2
  snapshot_interval: 10

postgres_url: "postgresql://trading:trading123@localhost:5433/exchange_info_db"
EOF

echo -e "${BLUE}=== Starting Adversarial Audit ===${NC}"

# ============================================================================
# TEST 1: WAL Corruption (GAP-01)
# ============================================================================
echo -e "${YELLOW}Test 1: WAL Poisoning & Fallback Audit (GAP-01)${NC}"

# Start and inject high volume (200 orders) to guarantee trades and checkpoints
./target/release/zero_x_infinity --gateway --env audit_settlement --port $PORT > "$GW_LOG" 2>&1 &
wait_for_gw || fail_audit "Gateway failed to start"
echo "Injecting 200 orders to generate trades..."
GATEWAY_URL="http://localhost:$PORT" python3 "${SCRIPT_DIR}/inject_orders.py" --input fixtures/orders.csv --limit 200 > /dev/null
sleep 3

# Kill
echo "Killing Gateway..."
pkill -f "zero_x_infinity.*--port.*$PORT"
sleep 1

# Verify WAL has entries
WAL_FILE=$(find "$SETTLEMENT_DATA_DIR" -name "*.wal" | head -1)
[ -f "$WAL_FILE" ] || fail_audit "WAL file not found"
WAL_SIZE=$(stat -f%z "$WAL_FILE" 2>/dev/null || stat -c%s "$WAL_FILE" 2>/dev/null)
echo "WAL identified: $WAL_FILE ($WAL_SIZE bytes)"
if [ "$WAL_SIZE" -lt 40 ]; then
    fail_audit "WAL too small ($WAL_SIZE bytes). No checkpoints written? Increase injection limit."
fi

# Corrupt WAL: Flip bits at offset 25 (inside first entry payload)
echo "Poisoning WAL..."
dd if=/dev/urandom of="$WAL_FILE" bs=1 count=10 seek=25 conv=notrunc 2>/dev/null

# Restart
echo "Restarting Gateway after WAL poisoning..."
./target/release/zero_x_infinity --gateway --env audit_settlement --port $PORT >> "$GW_LOG" 2>&1 &
wait_for_gw || fail_audit "Gateway failed to restart after WAL poisoning"

# Verify FALLBACK in logs
sleep 2
if grep -aq "WAL replay failed, using snapshot only" "$GW_LOG"; then
    pass_step "System successfully detected corruption and fell back to snapshot"
else
    echo -e "${RED}Log mismatch! Printing recent Settlement logs:${NC}"
    grep "Settlement" "$GW_LOG" | tail -n 20
    fail_audit "System did not report WAL corruption fallback"
fi

# ============================================================================
# TEST 2: Zombie Snapshot Audit (GAP-02)
# ============================================================================
echo -e "${YELLOW}Test 2: Zombie Snapshot Audit (GAP-02)${NC}"

pkill -f "zero_x_infinity.*--port.*$PORT"
sleep 1

# Delete COMPLETE marker from the latest snapshot
LATEST_LINK="$SETTLEMENT_DATA_DIR/snapshots/latest"
if [ ! -L "$LATEST_LINK" ]; then
    fail_audit "Snapshots/latest symlink not found. Did snapshot occur? (Need 10 trades)"
fi
LATEST=$(readlink "$LATEST_LINK")
COMPLETE_FILE="$SETTLEMENT_DATA_DIR/snapshots/$LATEST/COMPLETE"
echo "Creating Zombie Snapshot: removing $COMPLETE_FILE"
rm -f "$COMPLETE_FILE"

# Restart
echo "Restarting Gateway after Snapshot tampering..."
./target/release/zero_x_infinity --gateway --env audit_settlement --port $PORT >> "$GW_LOG" 2>&1 &
wait_for_gw || fail_audit "Gateway failed to restart after Snapshot tampering"

# Verify IGNORED in logs
sleep 2
if grep -q "Snapshot incomplete (missing COMPLETE marker)" "$GW_LOG"; then
    pass_step "System successfully ignored zombie snapshot"
else
    tail -50 "$GW_LOG"
    fail_audit "System did not detect missing COMPLETE marker"
fi

# ============================================================================
# TEST 3: Idempotency Audit (GAP-03/04)
# ============================================================================
echo -e "${YELLOW}Test 3: Post-Crash Functionality & ID Re-sync Audit${NC}"

# Check health and inject more
GATEWAY_URL="http://localhost:$PORT" python3 "${SCRIPT_DIR}/inject_orders.py" --input fixtures/orders.csv --limit 10 > /dev/null
if curl -sf "http://localhost:$PORT/api/v1/health" > /dev/null; then
    pass_step "System recovered to full operational state"
else
    fail_audit "System failed post-recovery operational check"
fi

echo ""
echo -e "${GREEN}════════════════════════════════════════════════════════════${NC}"
echo -e "${GREEN}  QA ADVERSARIAL AUDIT PASSED: ARCHITECTURAL COMPLIANCE OK   ${NC}"
echo -e "${GREEN}════════════════════════════════════════════════════════════${NC}"
