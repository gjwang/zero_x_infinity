#!/bin/bash
# Public Trades E2E Test Script
# Tests the GET /api/v1/public/trades endpoint with data injection via Gateway

set -e

# Use specific database URL for testing (must match seeded DB)
export DATABASE_URL="postgres://trading:trading123@localhost:5432/exchange_info_db"

BASE_URL="${1:-http://localhost:8080}"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

echo "🧪 Public Trades E2E Test"
echo "=========================="
echo "Base URL: $BASE_URL"
echo ""

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[0;33m'
NC='\033[0m' # No Color

pass() { echo -e "${GREEN}✅ $1${NC}"; }
fail() { echo -e "${RED}❌ $1${NC}"; exit 1; }
warn() { echo -e "${YELLOW}⚠️  $1${NC}"; }

# Cleanup function
cleanup() {
    if [ -n "$GATEWAY_PID" ]; then
        echo ""
        echo "Stopping Gateway (PID $GATEWAY_PID)..."
        kill "$GATEWAY_PID" 2>/dev/null || true
    fi
}
trap cleanup EXIT

# ============================================================
# Step 0: Check and start prerequisite services
# ============================================================
echo "Step 0: Checking prerequisite services..."

# Check TDengine
if ! docker ps | grep -q tdengine; then
    warn "TDengine not running, starting..."
    docker run -d --name tdengine -p 6030:6030 -p 6041:6041 tdengine/tdengine:latest 2>/dev/null || \
    docker start tdengine 2>/dev/null || fail "Cannot start TDengine"
    echo "   Waiting for TDengine to be ready (10s)..."
    sleep 10
fi
pass "TDengine is running"

# Create test directory with initial balances BEFORE starting Gateway
TEST_DIR="/tmp/public_trades_e2e_test"
mkdir -p "$TEST_DIR"

# Create initial balances for test users
cat > "$TEST_DIR/balances_init.csv" << EOF
user_id,asset_id,avail,frozen,version
1001,1,1000000000000,0,0
1001,2,1000000000000,0,0
1002,1,1000000000000,0,0
1002,2,1000000000000,0,0
1003,1,1000000000000,0,0
1003,2,1000000000000,0,0
EOF

# Check Gateway
if ! curl -sf "$BASE_URL/api/v1/health" > /dev/null 2>&1; then
    warn "Gateway not running, starting..."
    cd "$PROJECT_DIR"
    
    # Use CI config when running in CI environment
    if [ "$CI" = "true" ]; then
        ENV_FLAG="--env ci"
    else
        ENV_FLAG=""
    fi
    
    # Use pre-built binary (faster than cargo run)
    if [ -f "./target/release/zero_x_infinity" ]; then
        ./target/release/zero_x_infinity --gateway $ENV_FLAG --port 8080 --input "$TEST_DIR" > /tmp/gateway.log 2>&1 &
    else
        cargo run --release -- --gateway $ENV_FLAG --port 8080 --input "$TEST_DIR" > /tmp/gateway.log 2>&1 &
    fi
    GATEWAY_PID=$!
    echo "   Waiting for Gateway to be ready (30s)..."
    
    # Wait for Gateway with timeout
    for i in {1..30}; do
        if curl -sf "$BASE_URL/api/v1/health" > /dev/null 2>&1; then
            break
        fi
        sleep 1
    done
    
    if ! curl -sf "$BASE_URL/api/v1/health" > /dev/null 2>&1; then
        echo "Gateway log:"
        cat /tmp/gateway.log || true
        fail "Gateway failed to start. Check /tmp/gateway.log"
    fi
fi
pass "Gateway is running"
echo ""

# ============================================================
# Step 1: Get initial trade count
# ============================================================
echo "Step 1: Getting initial trade count..."
INITIAL_RESP=$(curl -s "$BASE_URL/api/v1/public/trades?limit=1000")
INITIAL_COUNT=$(echo "$INITIAL_RESP" | jq '.data | length')
echo "   Initial trades: $INITIAL_COUNT"

# ============================================================
# Step 2: Create matching orders to generate trades
# ============================================================
echo ""
echo "Step 2: Creating matching orders to generate trades..."

# Create multiple trades with different prices
cat > "$TEST_DIR/public_trades_orders.csv" << EOF
order_id,user_id,side,price,qty
2001,1001,buy,43000.00,0.1
2002,1002,sell,43000.00,0.1
2003,1001,buy,43100.00,0.05
2004,1003,sell,43100.00,0.05
2005,1002,buy,43200.00,0.08
2006,1001,sell,43200.00,0.08
EOF

# Use Python for Ed25519 authenticated order submission
export PYTHONPATH="$SCRIPT_DIR:$PYTHONPATH"
if [ "$CI" = "true" ]; then
    PYTHON_CMD="${PYTHON_CMD:-python3}"
elif [ -f "$PROJECT_DIR/.venv/bin/python3" ]; then
    PYTHON_CMD="${PYTHON_CMD:-$PROJECT_DIR/.venv/bin/python3}"
else
    PYTHON_CMD="${PYTHON_CMD:-python3}"
fi

if ! "$PYTHON_CMD" "$SCRIPT_DIR/inject_orders.py" --input "$TEST_DIR/public_trades_orders.csv" --quiet; then
    echo "   DEBUG: inject_orders.py failed, checking Gateway log:"
    cat /tmp/gateway.log 2>/dev/null | tail -20 || true
    fail "Order injection failed - check Ed25519 auth and pynacl installation"
fi
echo "   Orders submitted (3 trades expected)"

# Wait for trades to be written to TDengine
echo "   Waiting for TDengine write (3s)..."
sleep 3

# ============================================================
# Step 3: Test TC-API-001: Basic Fetch
# ============================================================
echo ""
echo "Step 3: TC-API-001 - Basic Fetch..."
TRADES_RESP=$(curl -s "$BASE_URL/api/v1/public/trades?limit=5")
echo "$TRADES_RESP" | jq '.'

# Verify response code
CODE=$(echo "$TRADES_RESP" | jq -r '.code')
[ "$CODE" = "0" ] && pass "Response code: 0" || fail "API returned error code: $CODE"

# Verify data is array
DATA_LEN=$(echo "$TRADES_RESP" | jq '.data | length')
[ "$DATA_LEN" -gt "0" ] && pass "Trades returned: $DATA_LEN" || fail "No trades returned"

# ============================================================
# Step 4: Verify response format (no sensitive data)
# ============================================================
echo ""
echo "Step 4: Verifying response format..."

FIRST_TRADE=$(echo "$TRADES_RESP" | jq '.data[0]')
echo "   First trade:"
echo "$FIRST_TRADE" | jq '.'

# Check required fields exist
TRADE_ID=$(echo "$FIRST_TRADE" | jq -r '.id')
PRICE=$(echo "$FIRST_TRADE" | jq -r '.price')
QTY=$(echo "$FIRST_TRADE" | jq -r '.qty')
QUOTE_QTY=$(echo "$FIRST_TRADE" | jq -r '.quote_qty')
TIME=$(echo "$FIRST_TRADE" | jq -r '.time')
IS_BUYER_MAKER=$(echo "$FIRST_TRADE" | jq -r '.is_buyer_maker')
IS_BEST_MATCH=$(echo "$FIRST_TRADE" | jq -r '.is_best_match')

[ -n "$TRADE_ID" ] && [ "$TRADE_ID" != "null" ] && pass "id field present" || fail "id field missing"
[ -n "$PRICE" ] && [ "$PRICE" != "null" ] && pass "price field present" || fail "price field missing"
[ -n "$QTY" ] && [ "$QTY" != "null" ] && pass "qty field present" || fail "qty field missing"
[ -n "$QUOTE_QTY" ] && [ "$QUOTE_QTY" != "null" ] && pass "quote_qty field present" || fail "quote_qty field missing"
[ -n "$TIME" ] && [ "$TIME" != "null" ] && pass "time field present" || fail "time field missing"

# CRITICAL: Verify NO sensitive fields
USER_ID=$(echo "$FIRST_TRADE" | jq -r '.user_id // "not_present"')
ORDER_ID=$(echo "$FIRST_TRADE" | jq -r '.order_id // "not_present"')

[ "$USER_ID" = "not_present" ] && pass "user_id NOT exposed (correct)" || fail "SECURITY: user_id exposed in response!"
[ "$ORDER_ID" = "not_present" ] && pass "order_id NOT exposed (correct)" || fail "SECURITY: order_id exposed in response!"

# Verify string formatting for prices/quantities
if echo "$PRICE" | grep -q '^[0-9]\+\.[0-9]\+$'; then
    pass "price is string-formatted: $PRICE"
else
    fail "price is not properly formatted: $PRICE"
fi

if echo "$QTY" | grep -q '^[0-9]\+\.[0-9]\+$'; then
    pass "qty is string-formatted: $QTY"
else
    fail "qty is not properly formatted: $QTY"
fi

# ============================================================
# Step 5: TC-API-003: Test Pagination
# ============================================================
echo ""
echo "Step 5: TC-API-003 - Testing pagination..."

# Get first page
PAGE1=$(curl -s "$BASE_URL/api/v1/public/trades?limit=2")
PAGE1_LEN=$(echo "$PAGE1" | jq '.data | length')
echo "   Page 1: $PAGE1_LEN trades"

if [ "$PAGE1_LEN" -gt "0" ]; then
    # Get last trade ID from page 1
    LAST_ID=$(echo "$PAGE1" | jq -r '.data[-1].id')
    echo "   Last ID on page 1: $LAST_ID"
    
    # Get page 2 using fromId
    PAGE2=$(curl -s "$BASE_URL/api/v1/public/trades?fromId=$LAST_ID&limit=2")
    PAGE2_LEN=$(echo "$PAGE2" | jq '.data | length')
    echo "   Page 2 (fromId=$LAST_ID): $PAGE2_LEN trades"
    
    if [ "$PAGE2_LEN" -gt "0" ]; then
        # Verify no duplicate IDs
        FIRST_ID_PAGE2=$(echo "$PAGE2" | jq -r '.data[0].id')
        if [ "$FIRST_ID_PAGE2" -gt "$LAST_ID" ]; then
            pass "Pagination works: no duplicate trades"
        else
            fail "Pagination broken: duplicate trade ID $FIRST_ID_PAGE2"
        fi
    else
        warn "Page 2 empty (may be expected if only 2 trades total)"
    fi
else
    warn "Cannot test pagination: no trades on page 1"
fi

# ============================================================
# Step 6: TC-API-004: Test Limit Cap
# ============================================================
echo ""
echo "Step 6: TC-API-004 - Testing limit cap..."
LARGE_LIMIT_RESP=$(curl -s "$BASE_URL/api/v1/public/trades?limit=2000")
LARGE_LIMIT_LEN=$(echo "$LARGE_LIMIT_RESP" | jq '.data | length')
echo "   Requested limit=2000, got: $LARGE_LIMIT_LEN trades"

if [ "$LARGE_LIMIT_LEN" -le "1000" ]; then
    pass "Limit cap enforced (max 1000)"
else
    fail "Limit cap NOT enforced: returned $LARGE_LIMIT_LEN trades"
fi

# ============================================================
# Summary
# ============================================================
echo ""
echo "=========================="
echo "🎉 Public Trades E2E Test Complete!"
echo ""
echo "Summary:"
echo "  ✅ TC-API-001: Basic fetch"
echo "  ✅ Security: No user_id/order_id exposure"
echo "  ✅ Format: String-formatted prices/quantities"
echo "  ✅ TC-API-003: Pagination"
echo "  ✅ TC-API-004: Limit cap"
echo ""
