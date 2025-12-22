#!/bin/bash
# Test script for Order Book Depth API
# Tests the /api/v1/depth endpoint with various scenarios

set -e

BASE_URL="http://localhost:8080"
SYMBOL="BTC_USDT"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
RED='\033[0;31m'
NC='\033[0m' # No Color

echo -e "${BLUE}=== Order Book Depth API Test ===${NC}\n"

# Check if Gateway is running, start if needed
if ! curl -sf "$BASE_URL/api/v1/health" > /dev/null 2>&1; then
    echo -e "${BLUE}Gateway not running, starting...${NC}"
    cd "$PROJECT_DIR"
    
    # Use CI config when running in CI environment
    if [ "$CI" = "true" ]; then
        ENV_FLAG="--env ci"
    else
        ENV_FLAG=""
    fi
    
    cargo run --release -- --gateway $ENV_FLAG --port 8080 > /tmp/gateway_depth.log 2>&1 &
    GATEWAY_PID=$!
    
    # Wait for Gateway
    for i in {1..30}; do
        if curl -sf "$BASE_URL/api/v1/health" > /dev/null 2>&1; then
            break
        fi
        sleep 1
    done
    
    if ! curl -sf "$BASE_URL/api/v1/health" > /dev/null 2>&1; then
        echo "Gateway failed to start"
        cat /tmp/gateway_depth.log || true
        exit 1
    fi
    echo -e "${GREEN}Gateway started${NC}"
fi

# Test 1: Query empty depth
echo -e "${BLUE}Test 1: Query depth${NC}"
curl -s "${BASE_URL}/api/v1/depth?symbol=${SYMBOL}&limit=5" | jq .
echo -e "\n"

# Test 2: Create test orders via inject_orders.py (Ed25519 authenticated)
echo -e "${BLUE}Test 2: Submit test orders (Ed25519 auth)${NC}"

# Create temporary orders file
TEST_DIR="/tmp/depth_test"
mkdir -p "$TEST_DIR"

cat > "$TEST_DIR/depth_orders.csv" << EOF
order_id,user_id,side,price,qty
1,1001,buy,29900,0.1
2,1001,buy,29800,0.2
3,1001,buy,29700,0.3
4,1002,sell,30100,0.1
5,1002,sell,30200,0.2
6,1002,sell,30300,0.3
EOF

# Determine Python command
export PYTHONPATH="$SCRIPT_DIR:$PYTHONPATH"
if [ "$CI" = "true" ]; then
    PYTHON_CMD="${PYTHON_CMD:-python3}"
elif [ -f "$PROJECT_DIR/.venv/bin/python3" ]; then
    PYTHON_CMD="${PYTHON_CMD:-$PROJECT_DIR/.venv/bin/python3}"
else
    PYTHON_CMD="${PYTHON_CMD:-python3}"
fi

if ! "$PYTHON_CMD" "$SCRIPT_DIR/inject_orders.py" --input "$TEST_DIR/depth_orders.csv" --quiet 2>/dev/null; then
    echo -e "${RED}Order injection failed${NC}"
    exit 1
fi
echo -e "${GREEN}Orders submitted successfully${NC}"

# Wait for depth update
echo -e "${BLUE}Waiting 500ms for depth update...${NC}"
sleep 0.5

# Test 3: Query depth with orders
echo -e "${BLUE}Test 3: Query depth (should show bids and asks)${NC}"
DEPTH_RESULT=$(curl -s "${BASE_URL}/api/v1/depth?symbol=${SYMBOL}&limit=5")
echo "$DEPTH_RESULT" | jq .

# Verify depth has data
BIDS_LEN=$(echo "$DEPTH_RESULT" | jq '.data.bids | length')
ASKS_LEN=$(echo "$DEPTH_RESULT" | jq '.data.asks | length')

if [ "$BIDS_LEN" -gt 0 ] && [ "$ASKS_LEN" -gt 0 ]; then
    echo -e "${GREEN}✅ Depth API working - bids: $BIDS_LEN, asks: $ASKS_LEN${NC}"
else
    echo -e "${RED}❌ Depth API not returning expected data${NC}"
fi

echo -e "\n${GREEN}=== All tests completed ===${NC}"
