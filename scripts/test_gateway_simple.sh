#!/bin/bash
# Simple Gateway Test Script

set -e

PORT=8080
BASE_URL="http://localhost:$PORT"

echo "=== Gateway API Test ==="
echo ""

# Test 1: Create LIMIT order
echo "[Test 1] Creating LIMIT order..."
curl -s -X POST "$BASE_URL/api/v1/create_order" \
  -H "Content-Type: application/json" \
  -H "X-User-ID: 1001" \
  -d '{
    "symbol": "BTC_USDT",
    "side": "BUY",
    "order_type": "LIMIT",
    "price": "85000.00",
    "qty": "0.001",
    "cid": "client-order-001"
  }' | jq .

echo ""

# Test 2: Create MARKET order
echo "[Test 2] Creating MARKET order..."
curl -s -X POST "$BASE_URL/api/v1/create_order" \
  -H "Content-Type: application/json" \
  -H "X-User-ID: 1002" \
  -d '{
    "symbol": "BTC_USDT",
    "side": "SELL",
    "order_type": "MARKET",
    "qty": "0.002"
  }' | jq .

echo ""

# Test 3: Cancel order
echo "[Test 3] Canceling order..."
curl -s -X POST "$BASE_URL/api/v1/cancel_order" \
  -H "Content-Type: application/json" \
  -H "X-User-ID: 1001" \
  -d '{
    "order_id": 1
  }' | jq .

echo ""

# Test 4: Missing X-User-ID (should return 401)
echo "[Test 4] Testing missing X-User-ID (expect 401)..."
HTTP_CODE=$(curl -s -o /dev/null -w "%{http_code}" \
  -X POST "$BASE_URL/api/v1/create_order" \
  -H "Content-Type: application/json" \
  -d '{"symbol":"BTC_USDT","side":"BUY","type":"LIMIT","price":"85000","qty":"0.001"}')

if [ "$HTTP_CODE" = "401" ]; then
    echo "✅ Correctly returned 401"
else
    echo "❌ Expected 401, got $HTTP_CODE"
fi

echo ""

# Test 5: Invalid side (should return 400)
echo "[Test 5] Testing invalid side (expect 400)..."
curl -s -X POST "$BASE_URL/api/v1/create_order" \
  -H "Content-Type: application/json" \
  -H "X-User-ID: 1001" \
  -d '{
    "symbol": "BTC_USDT",
    "side": "INVALID",
    "order_type": "LIMIT",
    "price": "85000",
    "qty": "0.001"
  }' | jq .

echo ""

# Test 6: Missing price for LIMIT order (should return 400)
echo "[Test 6] Testing missing price for LIMIT order (expect 400)..."
curl -s -X POST "$BASE_URL/api/v1/create_order" \
  -H "Content-Type: application/json" \
  -H "X-User-ID: 1001" \
  -d '{
    "symbol": "BTC_USDT",
    "side": "BUY",
    "order_type": "LIMIT",
    "qty": "0.001"
  }' | jq .

echo ""
echo "=== Tests Complete ==="
