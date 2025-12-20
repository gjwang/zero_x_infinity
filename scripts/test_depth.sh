#!/bin/bash
# Test script for Order Book Depth API
# Tests the /api/v1/depth endpoint with various scenarios

set -e

BASE_URL="http://localhost:8080"
SYMBOL="BTC_USDT"

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
RED='\033[0;31m'
NC='\033[0m' # No Color

echo -e "${BLUE}=== Order Book Depth API Test ===${NC}\n"

# Test 1: Query empty depth
echo -e "${BLUE}Test 1: Query empty depth${NC}"
curl -s "${BASE_URL}/api/v1/depth?symbol=${SYMBOL}&limit=5" | jq .
echo -e "\n"

# Test 2: Submit buy orders
echo -e "${BLUE}Test 2: Submit buy orders${NC}"
for i in {1..3}; do
    price=$((30000 - i * 100))
    qty="0.$i"
    echo "Submitting BUY order: ${price} @ ${qty}"
    curl -s -X POST "${BASE_URL}/api/v1/create_order" \
      -H "Content-Type: application/json" \
      -H "X-User-Id: 1" \
      -d "{\"symbol\": \"${SYMBOL}\", \"side\": \"BUY\", \"order_type\": \"LIMIT\", \"price\": \"${price}.00\", \"qty\": \"${qty}\"}" \
      | jq -r '.data.order_id // "ERROR"'
done
echo -e "\n"

# Test 3: Submit sell orders
echo -e "${BLUE}Test 3: Submit sell orders${NC}"
for i in {1..3}; do
    price=$((30100 + i * 100))
    qty="0.$i"
    echo "Submitting SELL order: ${price} @ ${qty}"
    curl -s -X POST "${BASE_URL}/api/v1/create_order" \
      -H "Content-Type: application/json" \
      -H "X-User-Id: 2" \
      -d "{\"symbol\": \"${SYMBOL}\", \"side\": \"SELL\", \"order_type\": \"LIMIT\", \"price\": \"${price}.00\", \"qty\": \"${qty}\"}" \
      | jq -r '.data.order_id // "ERROR"'
done
echo -e "\n"

# Wait for depth update (100ms interval)
echo -e "${BLUE}Waiting 200ms for depth update...${NC}"
sleep 0.2

# Test 4: Query depth with orders
echo -e "${BLUE}Test 4: Query depth (should show bids and asks)${NC}"
curl -s "${BASE_URL}/api/v1/depth?symbol=${SYMBOL}&limit=5" | jq .
echo -e "\n"

# Test 5: Test different limits
echo -e "${BLUE}Test 5: Query depth with limit=2${NC}"
curl -s "${BASE_URL}/api/v1/depth?symbol=${SYMBOL}&limit=2" | jq .
echo -e "\n"

# Test 6: Performance test - submit many orders quickly
echo -e "${BLUE}Test 6: Performance test (10 orders rapidly)${NC}"
for i in {1..10}; do
    price=$((29500 + i * 10))
    curl -s -X POST "${BASE_URL}/api/v1/create_order" \
      -H "Content-Type: application/json" \
      -H "X-User-Id: 3" \
      -d "{\"symbol\": \"${SYMBOL}\", \"side\": \"BUY\", \"order_type\": \"LIMIT\", \"price\": \"${price}.00\", \"qty\": \"0.01\"}" \
      > /dev/null &
done
wait

echo "Waiting 200ms for depth update..."
sleep 0.2

echo -e "${BLUE}Query depth after rapid orders:${NC}"
curl -s "${BASE_URL}/api/v1/depth?symbol=${SYMBOL}&limit=10" | jq '.data | {bids: .bids | length, asks: .asks | length, last_update_id}'
echo -e "\n"

echo -e "${GREEN}=== All tests completed ===${NC}"
