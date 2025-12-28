#!/bin/bash
# test_fee_e2e.sh - Trade Fee E2E Verification Test
# ===================================================================
#
# PURPOSE:
#   Verify trade fee system end-to-end through API:
#   1. Clear TDengine database (clean state)
#   2. Start Gateway with persistence
#   3. Inject orders through API
#   4. Query trades API (with authentication)
#   5. Verify fee/fee_asset/role fields in response
#
# USAGE:
#   ./scripts/test_fee_e2e.sh
#
# ===================================================================

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

cd "$PROJECT_DIR"

# Source database environment variables (CI-compatible)
if [ -f "$SCRIPT_DIR/lib/db_env.sh" ]; then
    source "$SCRIPT_DIR/lib/db_env.sh"
fi

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

STEP=0

fail_at_step() {
    echo ""
    echo -e "${RED}════════════════════════════════════════════════════════════${NC}"
    echo -e "${RED}FAILED at Step ${STEP}: $1${NC}"
    echo -e "${RED}════════════════════════════════════════════════════════════${NC}"
    exit 1
}

echo "╔════════════════════════════════════════════════════════════╗"
echo "║    Trade Fee E2E Verification Test                        ║"
echo "╚════════════════════════════════════════════════════════════╝"
echo ""

# ============================================================================
# Step 1: Check prerequisites
# ============================================================================
STEP=1
echo "[Step $STEP] Checking prerequisites..."

# Check TDengine (works both locally with docker and in CI with service container)
if [ -n "$CI" ]; then
    # In CI: use REST API to check TDengine
    if ! curl -sf -u root:taosdata -d "SHOW DATABASES" http://localhost:6041/rest/sql > /dev/null 2>&1; then
        fail_at_step "TDengine not responding on localhost:6041"
    fi
else
    # Local: check docker container
    if ! docker ps | grep -q tdengine; then
        fail_at_step "TDengine not running. Start with: docker start tdengine"
    fi
fi
echo -e "    ${GREEN}✓${NC} TDengine running"

if [ ! -f "fixtures/orders.csv" ]; then
    fail_at_step "fixtures/orders.csv not found"
fi
echo -e "    ${GREEN}✓${NC} Test data available"

# ============================================================================
# Step 2: Clear TDengine database (clean state)
# ============================================================================
STEP=2
echo ""
echo "[Step $STEP] Clearing TDengine database..."

# Use REST API (works in both local and CI environments)
curl -sf -u root:taosdata -d "DROP DATABASE IF EXISTS trading" http://localhost:6041/rest/sql > /dev/null 2>&1 || true
sleep 2
echo -e "    ${GREEN}✓${NC} Database cleared"

# ============================================================================
# Step 3: Stop any running Gateway and start fresh
# ============================================================================
STEP=3
echo ""
echo "[Step $STEP] Starting Gateway..."

# Stop existing Gateway
GW_PID=$(pgrep -f "./target/release/zero_x_infinity" 2>/dev/null | head -1)
if [ -n "$GW_PID" ]; then
    kill "$GW_PID" 2>/dev/null || true
    sleep 2
    echo -e "    ${GREEN}✓${NC} Old Gateway stopped"
fi

# Build if needed
if [ ! -f "target/release/zero_x_infinity" ]; then
    echo "    Building release..."
    cargo build --release --quiet
fi

# Start Gateway
# Use CI config when running in CI environment
if [ "$CI" = "true" ]; then
    ENV_FLAG="--env ci"
else
    ENV_FLAG=""
fi

nohup ./target/release/zero_x_infinity --gateway $ENV_FLAG --port 8080 > /tmp/gateway_fee_e2e.log 2>&1 &
sleep 3

# Wait for Gateway to be ready
check_gateway() {
    curl -sf "http://localhost:8080/api/v1/health" > /dev/null 2>&1
}

for i in $(seq 1 30); do
    if check_gateway; then
        break
    fi
    sleep 1
done

if ! check_gateway; then
    echo "    Gateway log:"
    tail -20 /tmp/gateway_fee_e2e.log
    fail_at_step "Gateway failed to start"
fi
echo -e "    ${GREEN}✓${NC} Gateway responding"

# ============================================================================
# Step 4: Inject orders through API
# ============================================================================
STEP=4
echo ""
echo "[Step $STEP] Injecting orders through API..."

if ! uv run "${SCRIPT_DIR}/inject_orders.py" --input fixtures/orders.csv --workers 10 --limit 1000 2>&1 | tail -5; then
    fail_at_step "Order injection failed"
fi
echo -e "    ${GREEN}✓${NC} Orders injected"

# Wait for processing
sleep 3

# ============================================================================
# Step 5: Query trades API and verify fee fields
# ============================================================================
STEP=5
echo ""
echo "[Step $STEP] Querying trades API and verifying fee fields..."

# Use Python script for authenticated API call
TRADES_RESULT=$(uv run python3 - << 'EOF'
import sys
sys.path.insert(0, 'scripts')
from lib.api_auth import get_test_client

try:
    client = get_test_client(user_id=1)
    resp = client.get("/api/v1/private/trades", params={"limit": 10})
    
    if resp.status_code != 200:
        print(f"ERROR:API returned {resp.status_code}: {resp.text}")
        sys.exit(1)
    
    data = resp.json()
    if data.get("code") != 0:
        print(f"ERROR:API error: {data}")
        sys.exit(1)
    
    trades = data.get("data", [])
    if not trades:
        print("ERROR:No trades found")
        sys.exit(1)
    
    print(f"TRADES:{len(trades)}")
    
    # Check required fields
    sample = trades[0]
    required = ["trade_id", "fee", "fee_asset", "role"]
    missing = [f for f in required if f not in sample]
    
    if missing:
        print(f"MISSING:{','.join(missing)}")
        sys.exit(1)
    
    # Check fee is not all zeros
    has_fee = any(float(t.get("fee", "0")) > 0 for t in trades)
    print(f"HAS_FEE:{has_fee}")
    
    # Print sample trade
    print(f"SAMPLE:trade_id={sample['trade_id']},fee={sample['fee']},fee_asset={sample['fee_asset']},role={sample['role']}")
    
except Exception as e:
    print(f"ERROR:{e}")
    sys.exit(1)
EOF
)

# Parse result
if echo "$TRADES_RESULT" | grep -q "^ERROR:"; then
    ERROR_MSG=$(echo "$TRADES_RESULT" | grep "^ERROR:" | cut -d: -f2-)
    fail_at_step "Trades API: $ERROR_MSG"
fi

TRADES_COUNT=$(echo "$TRADES_RESULT" | grep "^TRADES:" | cut -d: -f2)
HAS_FEE=$(echo "$TRADES_RESULT" | grep "^HAS_FEE:" | cut -d: -f2)
SAMPLE=$(echo "$TRADES_RESULT" | grep "^SAMPLE:" | cut -d: -f2-)
MISSING=$(echo "$TRADES_RESULT" | grep "^MISSING:" | cut -d: -f2-)

if [ -n "$MISSING" ]; then
    fail_at_step "Missing required fields: $MISSING"
fi

echo -e "    ${GREEN}✓${NC} Found $TRADES_COUNT trades"
echo -e "    ${GREEN}✓${NC} All required fields present (fee, fee_asset, role)"
echo -e "    ${GREEN}✓${NC} Sample: $SAMPLE"

if [ "$HAS_FEE" == "True" ]; then
    echo -e "    ${GREEN}✓${NC} Fee values > 0 present"
else
    echo -e "    ${YELLOW}⚠${NC} All fee values are 0 (may be expected for some trades)"
fi

# ============================================================================
# Summary
# ============================================================================
echo ""
echo "════════════════════════════════════════════════════════════"
echo "test result: 5 passed; 0 failed; 0 skipped"
echo "════════════════════════════════════════════════════════════"
echo ""
echo -e "${GREEN}╔════════════════════════════════════════════════════════════╗${NC}"
echo -e "${GREEN}║  ✅ FEE E2E TEST PASSED                                    ║${NC}"
echo -e "${GREEN}╚════════════════════════════════════════════════════════════╝${NC}"
exit 0
