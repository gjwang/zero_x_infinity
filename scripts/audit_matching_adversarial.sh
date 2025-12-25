#!/bin/bash
# audit_matching_adversarial.sh - Independent QA Audit for Matching Engine Recovery
# ===============================================================================
#
# PURPOSE:
#   Verify Matching Engine OrderBook recovery robustness against:
#   1. Zombie Snapshot: Missing COMPLETE marker - should fallback or fail gracefully?
#   2. Corrupted Snapshot: Checksum mismatch - should system crash?
#   3. Order Continuity: Are resting orders preserved after crash?
#
# ===============================================================================

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

PORT=18085
GW_LOG="/tmp/me_adversarial.log"
DATA_DIR="./data/audit_me"
SETTLEMENT_DATA_DIR="./data/audit_me_settle"

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
rm -rf "$DATA_DIR" "$SETTLEMENT_DATA_DIR" 2>/dev/null || true
mkdir -p "$DATA_DIR" "$SETTLEMENT_DATA_DIR"

trap cleanup EXIT

# Create test config
cat > config/audit_me.yaml <<EOF
log_level: "info"
log_dir: "./logs"
log_file: "audit_me.log"
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
  data_dir: "$DATA_DIR"
  snapshot_interval_trades: 10

settlement_persistence:
  enabled: true
  data_dir: "$SETTLEMENT_DATA_DIR"
  checkpoint_interval: 5
  snapshot_interval: 10

postgres_url: "postgresql://trading:trading123@localhost:5433/exchange_info_db"
EOF

echo -e "${BLUE}=== Starting Matching Engine Adversarial Audit ===${NC}"

# ============================================================================
# TEST 1: Create Baseline with Resting Orders
# ============================================================================
echo -e "${YELLOW}Test 1: Baseline - Create OrderBook with Resting Orders${NC}"

./target/release/zero_x_infinity --gateway --env audit_me --port $PORT > "$GW_LOG" 2>&1 &
wait_for_gw || fail_audit "Gateway failed to start"

# Inject orders to create matched trades and resting orders
echo "Injecting orders..."
GATEWAY_URL="http://localhost:$PORT" python3 "${SCRIPT_DIR}/inject_orders.py" --input fixtures/orders.csv --limit 100 > /dev/null 2>&1
sleep 3

# Check if snapshots were created
SNAPSHOT_COUNT=$(find "$DATA_DIR/snapshots" -maxdepth 1 -type d -name "snapshot-*" 2>/dev/null | wc -l || echo "0")
echo "Snapshots created: $SNAPSHOT_COUNT"

if [ "$SNAPSHOT_COUNT" -gt 0 ]; then
    pass_step "ME Snapshots created: $SNAPSHOT_COUNT"
else
    fail_audit "No ME snapshots created after 100 orders"
fi

# Get order count before crash
ORDERS_BEFORE=$(curl -sf "http://localhost:$PORT/api/v1/stats" | python3 -c "import sys,json; print(json.load(sys.stdin).get('orders_accepted',0))" 2>/dev/null || echo "0")
echo "Orders accepted before crash: $ORDERS_BEFORE"

# ============================================================================
# TEST 2: Crash and Verify OrderBook Recovery
# ============================================================================
echo -e "${YELLOW}Test 2: Crash Recovery - Verify OrderBook Restored${NC}"

# SIGKILL to simulate crash
pkill -9 -f "zero_x_infinity.*--port.*$PORT"
sleep 2

# Restart
./target/release/zero_x_infinity --gateway --env audit_me --port $PORT >> "$GW_LOG" 2>&1 &

if wait_for_gw; then
    pass_step "Gateway restarted after crash"
    
    # Check recovery logs
    if grep -q "Loaded OrderBook snapshot" "$GW_LOG"; then
        ORDER_COUNT=$(grep "Loaded OrderBook snapshot" "$GW_LOG" | tail -1 | grep -oP 'order_count=\K\d+' || echo "?")
        pass_step "OrderBook restored with $ORDER_COUNT orders"
    else
        warn_step "No OrderBook snapshot load log found"
    fi
else
    fail_audit "Gateway failed to restart after crash"
fi

# Verify system is functional
GATEWAY_URL="http://localhost:$PORT" python3 "${SCRIPT_DIR}/inject_orders.py" --input fixtures/orders.csv --limit 5 > /dev/null 2>&1
if [ $? -eq 0 ]; then
    pass_step "Post-crash order acceptance OK"
else
    warn_step "Post-crash order acceptance may have issues"
fi

# ============================================================================
# TEST 3: Zombie Snapshot Test (Missing COMPLETE marker)
# ============================================================================
echo -e "${YELLOW}Test 3: Zombie Snapshot - Missing COMPLETE Marker${NC}"

pkill -f "zero_x_infinity.*--port.*$PORT"
sleep 1

# Find latest snapshot and remove COMPLETE
LATEST_LINK="$DATA_DIR/snapshots/latest"
if [ -L "$LATEST_LINK" ] || [ -d "$LATEST_LINK" ]; then
    LATEST_DIR=$(readlink "$LATEST_LINK" 2>/dev/null || echo "")
    if [ -n "$LATEST_DIR" ]; then
        COMPLETE_FILE="$DATA_DIR/snapshots/$LATEST_DIR/COMPLETE"
        if [ -f "$COMPLETE_FILE" ]; then
            echo "Removing COMPLETE marker: $COMPLETE_FILE"
            rm -f "$COMPLETE_FILE"
            
            # Try to restart
            ./target/release/zero_x_infinity --gateway --env audit_me --port $PORT >> "$GW_LOG" 2>&1 &
            
            if wait_for_gw; then
                # Check how it handled zombie snapshot
                if grep -q "Incomplete snapshot" "$GW_LOG" || grep -q "cold start" "$GW_LOG"; then
                    pass_step "System detected zombie snapshot and fell back"
                else
                    warn_step "System started but unclear how zombie was handled"
                fi
            else
                echo -e "${RED}ME-GAP-01 CONFIRMED: Zombie snapshot causes startup FAILURE${NC}"
                grep -E "(error|panic|Failed|Incomplete)" "$GW_LOG" | tail -10
            fi
        else
            warn_step "COMPLETE file not found (already missing?)"
        fi
    else
        warn_step "Could not resolve latest symlink"
    fi
else
    warn_step "No snapshot latest link found"
fi

# ============================================================================
# TEST 4: Corrupted Snapshot (Checksum Mismatch)
# ============================================================================
echo -e "${YELLOW}Test 4: Corrupted Snapshot - Checksum Mismatch${NC}"

pkill -f "zero_x_infinity.*--port.*$PORT" 2>/dev/null || true
sleep 1

# Restore COMPLETE but corrupt orderbook.bin
if [ -n "$LATEST_DIR" ]; then
    COMPLETE_FILE="$DATA_DIR/snapshots/$LATEST_DIR/COMPLETE"
    ORDERBOOK_FILE="$DATA_DIR/snapshots/$LATEST_DIR/orderbook.bin"
    
    # Restore COMPLETE
    touch "$COMPLETE_FILE"
    
    # Corrupt orderbook.bin
    if [ -f "$ORDERBOOK_FILE" ]; then
        echo "Corrupting orderbook.bin..."
        dd if=/dev/urandom of="$ORDERBOOK_FILE" bs=1 count=100 seek=50 conv=notrunc 2>/dev/null
        
        # Try to restart
        ./target/release/zero_x_infinity --gateway --env audit_me --port $PORT >> "$GW_LOG" 2>&1 &
        
        if wait_for_gw; then
            if grep -q "Checksum mismatch" "$GW_LOG"; then
                warn_step "ME-GAP-02 CONFIRMED: Corrupted snapshot causes warning but system continues?"
            else
                warn_step "System started - corruption handling unclear"
            fi
        else
            echo -e "${RED}ME-GAP-02 CONFIRMED: Corrupted snapshot causes startup FAILURE${NC}"
            grep -E "(Checksum|error|panic|Failed)" "$GW_LOG" | tail -10
        fi
    else
        warn_step "orderbook.bin not found"
    fi
else
    warn_step "Skipping corruption test (no snapshot dir)"
fi

# ============================================================================
# Summary
# ============================================================================
echo ""
echo -e "${BLUE}════════════════════════════════════════════════════════════${NC}"
echo -e "${BLUE}  Matching Engine Adversarial Audit Complete                ${NC}"
echo -e "${BLUE}════════════════════════════════════════════════════════════${NC}"
echo ""
echo "Key Audit Points:"
echo "  - Snapshot creation: $SNAPSHOT_COUNT snapshots"
echo "  - Crash recovery: Verified"
echo "  - See full log: $GW_LOG"
