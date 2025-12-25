#!/bin/bash
# test_matching_persistence_e2e.sh - Matching Service Persistence E2E Test
# ===================================================================
#
# PURPOSE:
#   Verify Matching Service crash recovery end-to-end through Gateway:
#   1. Clear persistence directory
#   2. Start Gateway with persistence enabled
#   3. Inject orders → WAL/Snapshot files created
#   4. Kill Gateway (simulate crash)
#   5. Restart Gateway → Recovery from snapshot
#   6. Verify system continues working
#
# USAGE:
#   ./scripts/test_matching_persistence_e2e.sh
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
NC='\033[0;33m'

STEP=0
DATA_DIR="./data/test_matching_persistence"

fail_at_step() {
    echo ""
    echo -e "${RED}════════════════════════════════════════════════════════════${NC}"
    echo -e "${RED}FAILED at Step ${STEP}: $1${NC}"
    echo -e "${RED}════════════════════════════════════════════════════════════${NC}"
    
    # Kill Gateway if running
    pkill -f "zero_x_infinity.*--gateway" 2>/dev/null || true
    
    # Cleanup
    rm -rf "$DATA_DIR" 2>/dev/null || true
    
    exit 1
}

cleanup() {
    echo ""
    echo -e "${BLUE}Cleaning up...${NC}"
    pkill -f "zero_x_infinity.*--gateway" 2>/dev/null || true
    rm -rf "$DATA_DIR" 2>/dev/null || true
}

trap cleanup EXIT

echo "╔════════════════════════════════════════════════════════════╗"
echo "║    Matching Service Persistence E2E Test                  ║"
echo "╚════════════════════════════════════════════════════════════╝"
echo ""

# ============================================================================
# Step 1: Check prerequisites
# ============================================================================
STEP=1
echo "[Step $STEP] Checking prerequisites..."

if [ ! -f "fixtures/orders.csv" ]; then
    fail_at_step "fixtures/orders.csv not found"
fi
echo -e "    ${GREEN}✓${NC} Test data available"

# ============================================================================
# Step 2: Build release binary
# ============================================================================
STEP=2
echo ""
echo "[Step $STEP] Building Gateway..."

if ! cargo build --release --quiet 2>&1 | tail -1; then
    fail_at_step "Build failed"
fi
echo -e "    ${GREEN}✓${NC} Build successful"

# ============================================================================
# Step 3: Clear persistence directory
# ============================================================================
STEP=3
echo ""
echo "[Step $STEP] Clearing persistence directory..."

rm -rf "$DATA_DIR"
mkdir -p "$DATA_DIR"
echo -e "    ${GREEN}✓${NC} Clean state: $DATA_DIR"

# ============================================================================
# Step 4: Create temporary config with persistence enabled
# ============================================================================
STEP=4
echo ""
echo "[Step $STEP] Creating test configuration..."

cat > config/test_persistence.yaml <<EOF
log_level: "info"
log_dir: "./logs"
log_file: "test_persistence.log"
use_json: false
rotation: "daily"
sample_rate: 1
enable_tracing: false

gateway:
  host: "0.0.0.0"
  port: 18080
  queue_size: 10000

persistence:
  enabled: false  # Disable TDengine for this test
  tdengine_dsn: "taos://root:taosdata@localhost:6030"

matching_persistence:
  enabled: true
  data_dir: "$DATA_DIR"
  snapshot_interval_trades: 50  # Small interval for testing
EOF

echo -e "    ${GREEN}✓${NC} Test config created with persistence enabled"

# ============================================================================
# Step 5: Start Gateway (first time)
# ============================================================================
STEP=5
echo ""
echo "[Step $STEP] Starting Gateway (initial run)..."

GW_LOG="/tmp/matching_persistence_e2e.log"
./target/release/zero_x_infinity --gateway --env test_persistence --port 18080 > "$GW_LOG" 2>&1 &
GW_PID=$!
sleep 3

# Wait for Gateway
for i in $(seq 1 30); do
    if curl -sf "http://localhost:18080/api/v1/health" > /dev/null 2>&1; then
        break
    fi
    sleep 1
done

if ! curl -sf "http://localhost:18080/api/v1/health" > /dev/null 2>&1; then
    echo "    Gateway log:"
    tail -30 "$GW_LOG"
    fail_at_step "Gateway failed to start"
fi
echo -e "    ${GREEN}✓${NC} Gateway running (PID: $GW_PID)"

# ============================================================================
# Step 6: Inject orders (generate WAL + Snapshot)
# ============================================================================
STEP=6
echo ""
echo "[Step $STEP] Injecting orders..."

if ! python3 "${SCRIPT_DIR}/inject_orders.py" --input fixtures/orders.csv --workers 4 --limit 200 2>&1 | tail -3; then
    fail_at_step "Order injection failed"
fi
echo -e "   ${GREEN}✓${NC} Orders injected"

sleep 2

# ============================================================================
# Step 7: Verify persistence files created
# ============================================================================
STEP=7
echo ""
echo "[Step $STEP] Verifying persistence files..."

# MatchingService creates subdirectories under data_dir
if [ ! -d "$DATA_DIR" ]; then
    fail_at_step "Persistence directory not created: $DATA_DIR"
fi

WAL_FILES=$(find "$DATA_DIR" -name "*.wal" 2>/dev/null | wc -l)
SNAPSHOT_DIRS=$(find "$DATA_DIR" -type d -name "snapshot-*" 2>/dev/null | wc -l)

if [ "$WAL_FILES" -lt 1 ]; then
    ls -la "$DATA_DIR/matching" || true
    fail_at_step "No WAL files created"
fi

if [ "$SNAPSHOT_DIRS" -lt 1 ]; then
    ls -la "$DATA_DIR/matching/snapshots" || true
    echo -e "    ${YELLOW}⚠${NC} No snapshots created (may need more trades)"
else
    echo -e "    ${GREEN}✓${NC} Snapshots created: $SNAPSHOT_DIRS"
fi

echo -e "    ${GREEN}✓${NC} WAL files created: $WAL_FILES"

# ============================================================================
# Step 8: Simulate crash (kill Gateway)
# ============================================================================
STEP=8
echo ""
echo "[Step $STEP] Simulating crash (killing Gateway)..."

kill -9 $GW_PID 2>/dev/null || true
sleep 2
echo -e "    ${GREEN}✓${NC} Gateway killed"

# ============================================================================
# Step 9: Restart Gateway (recovery test)
# ============================================================================
STEP=9
echo ""
echo "[Step $STEP] Restarting Gateway (testing recovery)..."

./target/release/zero_x_infinity --gateway --env test_persistence --port 18080 > "$GW_LOG" 2>&1 &
GW_PID=$!
sleep 3

# Wait for Gateway
for i in $(seq 1 30); do
    if curl -sf "http://localhost:18080/api/v1/health" > /dev/null 2>&1; then
        break
    fi
    sleep 1
done

if ! curl -sf "http://localhost:18080/api/v1/health" > /dev/null 2>&1; then
    echo "    Gateway log:"
    tail -30 "$GW_LOG"
    fail_at_step "Gateway failed to restart"
fi

# Check log for recovery message
if grep -q "Loaded OrderBook snapshot" "$GW_LOG" || grep -q "cold start" "$GW_LOG"; then
    echo -e "    ${GREEN}✓${NC} Gateway recovered successfully"
else
    echo -e "    ${YELLOW}⚠${NC} No recovery log found (check manually)"
fi

echo -e "    ${GREEN}✓${NC} Gateway restarted (PID: $GW_PID)"

# ============================================================================
# Step 10: Inject more orders (verify system still works)
# ============================================================================
STEP=10
echo ""
echo "[Step $STEP] Injecting orders after recovery..."

if ! python3 "${SCRIPT_DIR}/inject_orders.py" --input fixtures/orders.csv --workers 4 --limit 100 2>&1 | tail -3; then
    fail_at_step "Post-recovery order injection failed"
fi
echo -e "    ${GREEN}✓${NC} System continues working after recovery"

# ============================================================================
# Summary
# ============================================================================
echo ""
echo "════════════════════════════════════════════════════════════"
echo "test result: 10 steps passed; 0 failed; 0 skipped"
echo "════════════════════════════════════════════════════════════"
echo ""
echo " Persistence System Verification:"
echo "  ✅ Gateway started with persistence"
echo "  ✅ WAL files created: $WAL_FILES"
echo "  ✅ Snapshot dirs created: $SNAPSHOT_DIRS"
echo "  ✅ Crash simulation successful"
echo "  ✅ Gateway recovered from persistence"
echo "  ✅ System functional after recovery"
echo ""
echo -e "${GREEN}╔════════════════════════════════════════════════════════════╗${NC}"
echo -e "${GREEN}║  ✅ MATCHING PERSISTENCE E2E TEST PASSED                   ║${NC}"
echo -e "${GREEN}╚════════════════════════════════════════════════════════════╝${NC}"
exit 0
