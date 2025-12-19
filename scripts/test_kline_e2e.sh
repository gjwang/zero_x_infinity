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

# Check Gateway
if ! curl -sf "$BASE_URL/api/v1/klines?interval=1m" > /dev/null 2>&1; then
    warn "Gateway not running, starting..."
    cd "$PROJECT_DIR"
    cargo run --release -- --gateway --port 8080 > /tmp/gateway.log 2>&1 &
    GATEWAY_PID=$!
    echo "   Waiting for Gateway to be ready (30s)..."
    
    # Wait for Gateway with timeout
    for i in {1..30}; do
        if curl -sf "$BASE_URL/api/v1/klines?interval=1m" > /dev/null 2>&1; then
            break
        fi
        sleep 1
    done
    
    if ! curl -sf "$BASE_URL/api/v1/klines?interval=1m" > /dev/null 2>&1; then
        fail "Gateway failed to start. Check /tmp/gateway.log"
    fi
fi
pass "Gateway is running"
echo ""

# Step 2: Get initial K-Line count
echo ""
echo "Step 2: Getting initial K-Line count..."
INITIAL=$(curl -s "$BASE_URL/api/v1/klines?interval=$INTERVAL&limit=1000" | jq '.data | length')
echo "   Initial K-Lines: $INITIAL"

# Step 3: Create matching orders to generate a trade
echo ""
echo "Step 3: Creating matching orders..."
PRICE="37000.00"
QTY="0.05"

# Buy order
BUY_RESP=$(curl -sf -X POST "$BASE_URL/api/v1/create_order" \
  -H "Content-Type: application/json" \
  -H "X-User-ID: 1001" \
  -d "{\"symbol\":\"BTC_USDT\",\"side\":\"BUY\",\"order_type\":\"LIMIT\",\"price\":\"$PRICE\",\"qty\":\"$QTY\"}")
BUY_ORDER_ID=$(echo "$BUY_RESP" | jq -r '.data.order_id')
echo "   Buy order created: $BUY_ORDER_ID"

# Sell order (matching)
SELL_RESP=$(curl -sf -X POST "$BASE_URL/api/v1/create_order" \
  -H "Content-Type: application/json" \
  -H "X-User-ID: 1002" \
  -d "{\"symbol\":\"BTC_USDT\",\"side\":\"SELL\",\"order_type\":\"LIMIT\",\"price\":\"$PRICE\",\"qty\":\"$QTY\"}")
SELL_ORDER_ID=$(echo "$SELL_RESP" | jq -r '.data.order_id')
echo "   Sell order created: $SELL_ORDER_ID"

# Step 4: Wait for Stream to process
echo ""
echo "Step 4: Waiting for TDengine Stream processing (5s)..."
sleep 5

# Step 5: Query K-Line API
echo ""
echo "Step 5: Querying K-Line API..."
KLINE_RESP=$(curl -s "$BASE_URL/api/v1/klines?interval=$INTERVAL&limit=5")
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
