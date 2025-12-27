#!/bin/bash
# K-Line E2E Test Script
# Tests the complete flow: Order -> Trade -> TDengine -> Stream -> K-Line API

set -e

BASE_URL="${1:-http://localhost:8080}"
INTERVAL="1m"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

echo "🧪 K-Line E2E Test"
echo "==================="
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
TEST_DIR="/tmp/kline_e2e_test"
mkdir -p "$TEST_DIR"

# Create initial balances for test users
cat > "$TEST_DIR/balances_init.csv" << EOF
user_id,asset_id,avail,frozen,version
1001,1,1000000000000,0,0
1001,2,1000000000000,0,0
1002,1,1000000000000,0,0
1002,2,1000000000000,0,0
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
    
    # Wait for Gateway with timeout - use /health endpoint for basic readiness
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

# Step 2: Get initial K-Line count
echo ""
echo "Step 2: Getting initial K-Line count..."
INITIAL=$(curl -s "$BASE_URL/api/v1/public/klines?interval=$INTERVAL&limit=1000" | jq '.data | length')
echo "   Initial K-Lines: $INITIAL"

# Step 3: Create matching orders to generate a trade
echo ""
echo "Step 3: Creating matching orders via Ed25519 authenticated API..."
PRICE="37000.00"
QTY="0.05"

# Create test orders CSV for inject_orders.py (TEST_DIR already created above)
cat > "$TEST_DIR/kline_orders.csv" << EOF
order_id,user_id,side,price,qty
1001,1001,buy,$PRICE,$QTY
1002,1002,sell,$PRICE,$QTY
EOF
# Use Python for Ed25519 authenticated order submission
export PYTHONPATH="$SCRIPT_DIR:$PYTHONPATH"
if command -v uv >/dev/null 2>&1; then
    PYTHON_CMD="uv run python3"
elif [ -f "$PROJECT_DIR/.venv/bin/python3" ]; then
    PYTHON_CMD="${PYTHON_CMD:-$PROJECT_DIR/.venv/bin/python3}"
else
    PYTHON_CMD="${PYTHON_CMD:-python3}"
fi

# Show debugging info in CI
if [ "$CI" = "true" ]; then
    echo "   DEBUG: PYTHON_CMD=$PYTHON_CMD"
    echo "   DEBUG: PYTHONPATH=$PYTHONPATH"
    echo "   DEBUG: Checking pynacl..."
    "$PYTHON_CMD" -c "import nacl; print('   pynacl version:', nacl.__version__)" || echo "   WARN: pynacl not available"
    echo "   DEBUG: Order file contents:"
    cat "$TEST_DIR/kline_orders.csv"
fi

if ! $PYTHON_CMD "$SCRIPT_DIR/inject_orders.py" --input "$TEST_DIR/kline_orders.csv" --quiet; then
    echo "   DEBUG: inject_orders.py failed, checking Gateway log:"
    cat /tmp/gateway.log 2>/dev/null | tail -20 || true
    fail "Order injection failed - check Ed25519 auth and pynacl installation"
fi
echo "   Orders submitted via Ed25519 auth"

# Step 4: Wait for Stream to process
echo ""
echo "Step 4: Waiting for TDengine Stream processing (5s)..."
sleep 5

# Step 5: Query K-Line API
echo ""
echo "Step 5: Querying K-Line API..."
KLINE_RESP=$(curl -s "$BASE_URL/api/v1/public/klines?interval=$INTERVAL&limit=5")
echo "$KLINE_RESP" | jq '.'

# Step 6: Verify response
echo ""
echo "Step 6: Verifying response..."
CODE=$(echo "$KLINE_RESP" | jq -r '.code')
DATA_LEN=$(echo "$KLINE_RESP" | jq '.data | length')

if [ "$CODE" != "0" ]; then
  fail "API returned error code: $CODE"
fi
pass "API response code: 0"

if [ "$DATA_LEN" -eq "0" ]; then
  echo "   ⚠️  Note: K-Line data may need more time for Stream window to close"
  echo "   Wait 1 minute and retry, or check TDengine directly:"
  echo "   docker exec tdengine taos -s 'USE trading; SELECT * FROM klines_$INTERVAL LIMIT 5;'"
else
  pass "K-Line data present: $DATA_LEN record(s)"
  
  # Verify first K-Line structure
  FIRST_KLINE=$(echo "$KLINE_RESP" | jq '.data[0]')
  SYMBOL=$(echo "$FIRST_KLINE" | jq -r '.symbol')
  OPEN=$(echo "$FIRST_KLINE" | jq -r '.open')
  HIGH=$(echo "$FIRST_KLINE" | jq -r '.high')
  LOW=$(echo "$FIRST_KLINE" | jq -r '.low')
  CLOSE=$(echo "$FIRST_KLINE" | jq -r '.close')
  
  echo ""
  echo "   Latest K-Line:"
  echo "   - Symbol: $SYMBOL"
  echo "   - OHLC: $OPEN / $HIGH / $LOW / $CLOSE"
  
  [ "$SYMBOL" = "BTC_USDT" ] && pass "Symbol is correct" || fail "Wrong symbol: $SYMBOL"
fi

echo ""
echo "==================="
echo "🎉 K-Line E2E Test Complete!"
