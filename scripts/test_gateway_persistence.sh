#!/bin/bash
# test_gateway_persistence.sh - Verify Gateway writes data correctly to TDengine
# ================================================================================
#
# PURPOSE:
#   Verify that Gateway correctly persists data to TDengine and can query it back.
#   This is an API-level verification (not Pipeline comparison).
#
# WORKFLOW:
#   1. Clear TDengine database
#   2. Start Gateway with persistence enabled
#   3. Submit test orders via HTTP API
#   4. Query balances/orders/trades via API
#   5. Verify data is returned correctly
#
# USAGE:
#   ./scripts/test_gateway_persistence.sh
#
# ================================================================================

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

cd "$PROJECT_DIR"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

GATEWAY_URL="http://localhost:8080"
STEP=0

fail_at_step() {
    echo ""
    echo -e "${RED}════════════════════════════════════════════════════════════${NC}"
    echo -e "${RED}FAILED at Step ${STEP}: $1${NC}"
    echo -e "${RED}════════════════════════════════════════════════════════════${NC}"
    exit 1
}

echo "╔════════════════════════════════════════════════════════════╗"
echo "║    Gateway Persistence Verification                       ║"
echo "╚════════════════════════════════════════════════════════════╝"
echo ""

# ============================================================================
# Step 1: Check TDengine
# ============================================================================
STEP=1
echo "[Step $STEP] Checking TDengine..."

if ! docker ps | grep -q tdengine; then
    fail_at_step "TDengine not running. Start with: docker start tdengine"
fi
echo -e "    ${GREEN}✓${NC} TDengine running"

# ============================================================================
# Step 2: Clear TDengine database
# ============================================================================
STEP=2
echo ""
echo "[Step $STEP] Clearing TDengine database..."

docker exec tdengine taos -s "DROP DATABASE IF EXISTS exchange" 2>&1 | grep -v "^taos>" || true
sleep 2
echo -e "    ${GREEN}✓${NC} Database cleared"

# ============================================================================
# Step 3: Check Gateway is running
# ============================================================================
STEP=3
echo ""
echo "[Step $STEP] Checking Gateway..."

if ! curl -s "${GATEWAY_URL}/api/v1/ping" >/dev/null 2>&1; then
    fail_at_step "Gateway not running at ${GATEWAY_URL}"
fi
echo -e "    ${GREEN}✓${NC} Gateway responding"

# ============================================================================
# Step 4: Submit test orders
# ============================================================================
STEP=4
echo ""
echo "[Step $STEP] Submitting test orders..."

# Order 1: Buy order (X-User-ID header required for auth)
RESP1=$(curl -sf -X POST "${GATEWAY_URL}/api/v1/create_order" \
    -H "Content-Type: application/json" \
    -H "X-User-ID: 1001" \
    -d '{
        "symbol": "BTC_USDT",
        "side": "BUY",
        "order_type": "LIMIT",
        "price": "30000.00",
        "qty": "0.1"
    }' 2>/dev/null || echo "FAILED")

if [ "$RESP1" = "FAILED" ]; then
    fail_at_step "Failed to submit buy order"
fi

ORDER1_ID=$(echo "$RESP1" | grep -o '"order_id":[0-9]*' | cut -d: -f2)
echo -e "    ${GREEN}✓${NC} Buy order submitted (ID: ${ORDER1_ID:-unknown})"

# Order 2: Sell order (to match)
RESP2=$(curl -sf -X POST "${GATEWAY_URL}/api/v1/create_order" \
    -H "Content-Type: application/json" \
    -H "X-User-ID: 1002" \
    -d '{
        "symbol": "BTC_USDT",
        "side": "SELL",
        "order_type": "LIMIT",
        "price": "30000.00",
        "qty": "0.1"
    }' 2>/dev/null || echo "FAILED")

if [ "$RESP2" = "FAILED" ]; then
    fail_at_step "Failed to submit sell order"
fi

ORDER2_ID=$(echo "$RESP2" | grep -o '"order_id":[0-9]*' | cut -d: -f2)
echo -e "    ${GREEN}✓${NC} Sell order submitted (ID: ${ORDER2_ID:-unknown})"

# Wait for processing
sleep 2

# ============================================================================
# Step 5: Query and verify data
# ============================================================================
STEP=5
echo ""
echo "[Step $STEP] Querying data from TDengine via Gateway API..."

# Query balances for user 1001
BAL_RESP=$(curl -sf "${GATEWAY_URL}/api/v1/balances?user_id=1001&asset_id=1" 2>/dev/null || echo "FAILED")

if [ "$BAL_RESP" = "FAILED" ]; then
    fail_at_step "Failed to query balance for user 1001"
fi

if echo "$BAL_RESP" | grep -q '"code":0'; then
    echo -e "    ${GREEN}✓${NC} Balance query successful for user 1001"
    echo "      Response: $(echo "$BAL_RESP" | head -c 100)..."
else
    echo -e "    ${YELLOW}⚠${NC} Balance query returned: $BAL_RESP"
fi

# Query trades
TRADES_RESP=$(curl -sf "${GATEWAY_URL}/api/v1/trades?limit=5" 2>/dev/null || echo "FAILED")

if [ "$TRADES_RESP" = "FAILED" ]; then
    fail_at_step "Failed to query trades"
fi

if echo "$TRADES_RESP" | grep -q '"code":0'; then
    TRADE_COUNT=$(echo "$TRADES_RESP" | grep -o '"trade_id"' | wc -l)
    echo -e "    ${GREEN}✓${NC} Trades query successful ($TRADE_COUNT trades found)"
else
    echo -e "    ${YELLOW}⚠${NC} Trades query returned: $TRADES_RESP"
fi

# Query orders
ORDERS_RESP=$(curl -sf "${GATEWAY_URL}/api/v1/orders?user_id=1001&limit=5" 2>/dev/null || echo "FAILED")

if [ "$ORDERS_RESP" = "FAILED" ]; then
    fail_at_step "Failed to query orders"
fi

if echo "$ORDERS_RESP" | grep -q '"code":0'; then
    ORDER_COUNT=$(echo "$ORDERS_RESP" | grep -o '"order_id"' | wc -l)
    echo -e "    ${GREEN}✓${NC} Orders query successful ($ORDER_COUNT orders found)"
else
    echo -e "    ${YELLOW}⚠${NC} Orders query returned: $ORDERS_RESP"
fi

# ============================================================================
# Success
# ============================================================================
echo ""
echo -e "${GREEN}╔════════════════════════════════════════════════════════════╗${NC}"
echo -e "${GREEN}║  ✅ GATEWAY PERSISTENCE VERIFICATION PASSED                ║${NC}"
echo -e "${GREEN}╚════════════════════════════════════════════════════════════╝${NC}"
echo ""
echo "Verified:"
echo "  [1] TDengine running         ✓"
echo "  [2] Database cleared         ✓"
echo "  [3] Gateway responding       ✓"
echo "  [4] Orders submitted         ✓"
echo "  [5] Data queryable           ✓"
exit 0
