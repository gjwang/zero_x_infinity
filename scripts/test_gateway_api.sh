#!/bin/bash

# Gateway API Integration Test Script
# Tests all endpoints with unified {code, msg, data} response format

set -e

# Configuration
GATEWAY_URL="${GATEWAY_URL:-http://localhost:8080}"
VERBOSE="${VERBOSE:-0}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Counters
TESTS_RUN=0
TESTS_PASSED=0
TESTS_FAILED=0

# Helper functions
log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[PASS]${NC} $1"
    TESTS_PASSED=$((TESTS_PASSED + 1))
}

log_error() {
    echo -e "${RED}[FAIL]${NC} $1"
    TESTS_FAILED=$((TESTS_FAILED + 1))
}

log_test() {
    echo -e "${YELLOW}[TEST]${NC} $1"
    TESTS_RUN=$((TESTS_RUN + 1))
}

# Check if jq is installed
if ! command -v jq &> /dev/null; then
    log_error "jq is not installed. Please install it first: brew install jq"
    exit 1
fi

# Check if Gateway is running
log_info "Checking if Gateway is running at $GATEWAY_URL..."
HEALTH_CHECK=$(curl -s -X POST "$GATEWAY_URL/api/v1/create_order" \
    -H "Content-Type: application/json" \
    -H "X-User-ID: 1" \
    -d '{"symbol":"BTC_USDT","side":"BUY","order_type":"LIMIT","price":1,"qty":0.001}' 2>&1)

if echo "$HEALTH_CHECK" | grep -q -E '(code|error|Failed to deserialize)'; then
    log_success "Gateway is running"
else
    log_error "Gateway is not running at $GATEWAY_URL"
    log_info "Start it with: cargo run --release -- --gateway --port 8080"
    exit 1
fi

echo ""
echo "=========================================="
echo "  Gateway API Integration Tests"
echo "=========================================="
echo ""

# Test 1: Create LIMIT order (success)
log_test "Create LIMIT order"
RESPONSE=$(curl -s -X POST "$GATEWAY_URL/api/v1/create_order" \
    -H "Content-Type: application/json" \
    -H "X-User-ID: 1001" \
    -d '{
        "symbol": "BTC_USDT",
        "side": "BUY",
        "order_type": "LIMIT",
        "price": 85000.00,
        "qty": 0.001
    }')

CODE=$(echo "$RESPONSE" | jq -r '.code')
ORDER_ID=$(echo "$RESPONSE" | jq -r '.data.order_id')
STATUS=$(echo "$RESPONSE" | jq -r '.data.order_status')

if [[ "$CODE" == "0" && "$STATUS" == "ACCEPTED" ]]; then
    log_success "LIMIT order created (order_id: $ORDER_ID)"
    [[ "$VERBOSE" == "1" ]] && echo "$RESPONSE" | jq .
else
    log_error "LIMIT order creation failed"
    echo "$RESPONSE" | jq .
fi

# Test 2: Create MARKET order (success)
log_test "Create MARKET order"
RESPONSE=$(curl -s -X POST "$GATEWAY_URL/api/v1/create_order" \
    -H "Content-Type: application/json" \
    -H "X-User-ID: 1002" \
    -d '{
        "symbol": "BTC_USDT",
        "side": "SELL",
        "order_type": "MARKET",
        "qty": 0.002
    }')

CODE=$(echo "$RESPONSE" | jq -r '.code')
ORDER_ID=$(echo "$RESPONSE" | jq -r '.data.order_id')
STATUS=$(echo "$RESPONSE" | jq -r '.data.order_status')

if [[ "$CODE" == "0" && "$STATUS" == "ACCEPTED" ]]; then
    log_success "MARKET order created (order_id: $ORDER_ID)"
    [[ "$VERBOSE" == "1" ]] && echo "$RESPONSE" | jq .
else
    log_error "MARKET order creation failed"
    echo "$RESPONSE" | jq .
fi

# Test 3: Cancel order (success)
log_test "Cancel order"
RESPONSE=$(curl -s -X POST "$GATEWAY_URL/api/v1/cancel_order" \
    -H "Content-Type: application/json" \
    -H "X-User-ID: 1001" \
    -d '{
        "order_id": 1
    }')

CODE=$(echo "$RESPONSE" | jq -r '.code')
STATUS=$(echo "$RESPONSE" | jq -r '.data.order_status')

if [[ "$CODE" == "0" && "$STATUS" == "CANCEL_PENDING" ]]; then
    log_success "Order cancelled successfully"
    [[ "$VERBOSE" == "1" ]] && echo "$RESPONSE" | jq .
else
    log_error "Order cancellation failed"
    echo "$RESPONSE" | jq .
fi

# Test 4: Missing X-User-ID header (error 2001)
log_test "Missing X-User-ID header (expect error 2001)"
RESPONSE=$(curl -s -X POST "$GATEWAY_URL/api/v1/create_order" \
    -H "Content-Type: application/json" \
    -d '{
        "symbol": "BTC_USDT",
        "side": "BUY",
        "order_type": "LIMIT",
        "price": 85000,
        "qty": 0.001
    }')

CODE=$(echo "$RESPONSE" | jq -r '.code')
MSG=$(echo "$RESPONSE" | jq -r '.msg')

if [[ "$CODE" == "2001" && "$MSG" == *"X-User-ID"* ]]; then
    log_success "Missing auth error handled correctly (code: $CODE)"
    [[ "$VERBOSE" == "1" ]] && echo "$RESPONSE" | jq .
else
    log_error "Missing auth error handling failed"
    echo "$RESPONSE" | jq .
fi

# Test 5: Empty symbol (error 1001)
log_test "Empty symbol (expect error 1001)"
RESPONSE=$(curl -s -X POST "$GATEWAY_URL/api/v1/create_order" \
    -H "Content-Type: application/json" \
    -H "X-User-ID: 1001" \
    -d '{
        "symbol": "",
        "side": "BUY",
        "order_type": "LIMIT",
        "price": 85000,
        "qty": 0.001
    }')

# Empty symbol should be rejected by serde deserializer
if echo "$RESPONSE" | grep -q "cannot be empty"; then
    log_success "Empty symbol rejected correctly"
    [[ "$VERBOSE" == "1" ]] && echo "$RESPONSE"
else
    log_error "Empty symbol validation failed"
    echo "$RESPONSE"
fi

# Test 6: Invalid side (serde error)
log_test "Invalid side enum (expect serde error)"
RESPONSE=$(curl -s -X POST "$GATEWAY_URL/api/v1/create_order" \
    -H "Content-Type: application/json" \
    -H "X-User-ID: 1001" \
    -d '{
        "symbol": "BTC_USDT",
        "side": "INVALID",
        "order_type": "LIMIT",
        "price": 85000,
        "qty": 0.001
    }')

if echo "$RESPONSE" | grep -q -i "error\|invalid"; then
    log_success "Invalid side rejected correctly"
    [[ "$VERBOSE" == "1" ]] && echo "$RESPONSE"
else
    log_error "Invalid side validation failed"
    echo "$RESPONSE"
fi

# Test 7: LIMIT order without price (error 1001)
log_test "LIMIT order without price (expect error 1001)"
RESPONSE=$(curl -s -X POST "$GATEWAY_URL/api/v1/create_order" \
    -H "Content-Type: application/json" \
    -H "X-User-ID: 1001" \
    -d '{
        "symbol": "BTC_USDT",
        "side": "BUY",
        "order_type": "LIMIT",
        "qty": 0.001
    }')

CODE=$(echo "$RESPONSE" | jq -r '.code')
MSG=$(echo "$RESPONSE" | jq -r '.msg')

if [[ "$CODE" == "1001" && "$MSG" == *"required"* ]]; then
    log_success "Missing price error handled correctly (code: $CODE)"
    [[ "$VERBOSE" == "1" ]] && echo "$RESPONSE" | jq .
else
    log_error "Missing price validation failed"
    echo "$RESPONSE" | jq .
fi

# Test 8: Zero quantity (error 1001)
log_test "Zero quantity (expect error 1001)"
RESPONSE=$(curl -s -X POST "$GATEWAY_URL/api/v1/create_order" \
    -H "Content-Type: application/json" \
    -H "X-User-ID: 1001" \
    -d '{
        "symbol": "BTC_USDT",
        "side": "BUY",
        "order_type": "LIMIT",
        "price": 85000,
        "qty": 0
    }')

CODE=$(echo "$RESPONSE" | jq -r '.code')
MSG=$(echo "$RESPONSE" | jq -r '.msg')

if [[ "$CODE" == "1001" && "$MSG" == *"greater than zero"* ]]; then
    log_success "Zero quantity error handled correctly (code: $CODE)"
    [[ "$VERBOSE" == "1" ]] && echo "$RESPONSE" | jq .
else
    log_error "Zero quantity validation failed"
    echo "$RESPONSE" | jq .
fi

# Summary
echo ""
echo "=========================================="
echo "  Test Summary"
echo "=========================================="
echo -e "Total:  ${TESTS_RUN}"
echo -e "${GREEN}Passed: ${TESTS_PASSED}${NC}"
echo -e "${RED}Failed: ${TESTS_FAILED}${NC}"
echo ""

if [[ $TESTS_FAILED -eq 0 ]]; then
    echo -e "${GREEN}✅ All tests passed!${NC}"
    exit 0
else
    echo -e "${RED}❌ Some tests failed${NC}"
    exit 1
fi
