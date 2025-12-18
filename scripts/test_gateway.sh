#!/bin/bash
# Gateway Integration Test Script

set -e

echo "=== Gateway Integration Test ==="
echo ""

# 1. Start Gateway in background
echo "[1] Starting Gateway..."
cargo run --release -- --gateway --port 8080 --input fixtures &
GATEWAY_PID=$!

# Wait for server to start
echo "    Waiting for server to start..."
sleep 3

# Check if process is still running
if ! kill -0 $GATEWAY_PID 2>/dev/null; then
    echo "❌ Gateway failed to start"
    exit 1
fi

echo "✅ Gateway started (PID: $GATEWAY_PID)"
echo ""

# 2. Test create_order endpoint
echo "[2] Testing POST /api/v1/create_order..."
RESPONSE=$(curl -s -X POST http://localhost:8080/api/v1/create_order \
  -H "Content-Type: application/json" \
  -H "X-User-ID: 1001" \
  -d '{
    "symbol": "BTC_USDT",
    "side": "BUY",
    "type": "LIMIT",
    "price": "85000.00",
    "qty": "0.001"
  }')

echo "Response: $RESPONSE"

# Check if response contains order_id
if echo "$RESPONSE" | grep -q "order_id"; then
    echo "✅ Create order successful"
else
    echo "❌ Create order failed"
    kill $GATEWAY_PID
    exit 1
fi
echo ""

# 3. Test cancel_order endpoint
echo "[3] Testing POST /api/v1/cancel_order..."
RESPONSE=$(curl -s -X POST http://localhost:8080/api/v1/cancel_order \
  -H "Content-Type: application/json" \
  -H "X-User-ID: 1001" \
  -d '{"order_id": 1}')

echo "Response: $RESPONSE"

if echo "$RESPONSE" | grep -q "CANCEL_PENDING"; then
    echo "✅ Cancel order successful"
else
    echo "❌ Cancel order failed"
    kill $GATEWAY_PID
    exit 1
fi
echo ""

# 4. Test missing X-User-ID (should return 401)
echo "[4] Testing missing X-User-ID (expect 401)..."
HTTP_CODE=$(curl -s -o /dev/null -w "%{http_code}" -X POST http://localhost:8080/api/v1/create_order \
  -H "Content-Type: application/json" \
  -d '{"symbol": "BTC_USDT", "side": "BUY", "type": "LIMIT", "price": "85000", "qty": "0.001"}')

if [ "$HTTP_CODE" = "401" ]; then
    echo "✅ Correctly returned 401 for missing X-User-ID"
else
    echo "❌ Expected 401, got $HTTP_CODE"
    kill $GATEWAY_PID
    exit 1
fi
echo ""

# 5. Test invalid parameters (should return 400)
echo "[5] Testing invalid side parameter (expect 400)..."
HTTP_CODE=$(curl -s -o /dev/null -w "%{http_code}" -X POST http://localhost:8080/api/v1/create_order \
  -H "Content-Type: application/json" \
  -H "X-User-ID: 1001" \
  -d '{"symbol": "BTC_USDT", "side": "INVALID", "type": "LIMIT", "price": "85000", "qty": "0.001"}')

if [ "$HTTP_CODE" = "400" ]; then
    echo "✅ Correctly returned 400 for invalid side"
else
    echo "❌ Expected 400, got $HTTP_CODE"
    kill $GATEWAY_PID
    exit 1
fi
echo ""

# 6. Cleanup
echo "[6] Cleaning up..."
kill $GATEWAY_PID
wait $GATEWAY_PID 2>/dev/null || true
echo "✅ Gateway stopped"
echo ""

echo "=== All Tests Passed! ==="
