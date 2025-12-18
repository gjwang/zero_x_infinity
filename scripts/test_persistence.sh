#!/bin/bash
#
# Settlement Persistence E2E Test Script
# Tests TDengine integration and query endpoints
#

set -e  # Exit on error

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Test counters
TESTS_PASSED=0
TESTS_FAILED=0

# Helper functions
print_header() {
    echo -e "\n${BLUE}========================================${NC}"
    echo -e "${BLUE}$1${NC}"
    echo -e "${BLUE}========================================${NC}\n"
}

print_test() {
    echo -e "${YELLOW}[TEST]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[✓]${NC} $1"
    ((TESTS_PASSED++))
}

print_error() {
    echo -e "${RED}[✗]${NC} $1"
    ((TESTS_FAILED++))
}

print_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

# Check if command exists
command_exists() {
    command -v "$1" >/dev/null 2>&1
}

# Wait for service to be ready
wait_for_service() {
    local url=$1
    local max_attempts=30
    local attempt=0
    
    print_info "Waiting for service at $url..."
    while [ $attempt -lt $max_attempts ]; do
        if curl -s "$url" >/dev/null 2>&1; then
            print_success "Service is ready"
            return 0
        fi
        sleep 1
        ((attempt++))
    done
    
    print_error "Service failed to start after $max_attempts seconds"
    return 1
}

# Cleanup function
cleanup() {
    print_info "Cleaning up..."
    pkill -f "zero_x_infinity --gateway" 2>/dev/null || true
    sleep 1
}

# Set trap for cleanup
trap cleanup EXIT

# ============================================
# Pre-flight Checks
# ============================================
print_header "Pre-flight Checks"

print_test "Checking required commands..."
for cmd in docker curl jq cargo; do
    if command_exists "$cmd"; then
        print_success "$cmd is installed"
    else
        print_error "$cmd is not installed"
        exit 1
    fi
done

# ============================================
# TDengine Setup
# ============================================
print_header "TDengine Setup"

print_test "Checking TDengine container..."
if docker ps | grep -q tdengine; then
    print_success "TDengine is running"
else
    print_info "Starting TDengine container..."
    docker run -d --name tdengine -p 6030:6030 -p 6041:6041 tdengine/tdengine:latest
    sleep 5
    print_success "TDengine started"
fi

print_test "Testing TDengine connection..."
if docker exec tdengine taos -s "SELECT SERVER_VERSION();" >/dev/null 2>&1; then
    print_success "TDengine connection successful"
else
    print_error "Failed to connect to TDengine"
    exit 1
fi

# ============================================
# Build Application
# ============================================
print_header "Building Application"

print_test "Building release binary..."
if cargo build --release --bin zero_x_infinity 2>&1 | tail -1 | grep -q "Finished"; then
    print_success "Build successful"
else
    print_error "Build failed"
    exit 1
fi

# ============================================
# Unit Tests
# ============================================
print_header "Unit Tests"

print_test "Running persistence unit tests..."
if cargo test --lib persistence -- --ignored --nocapture 2>&1 | grep -q "test result: ok"; then
    print_success "All unit tests passed"
else
    print_error "Unit tests failed"
fi

# ============================================
# Start Gateway
# ============================================
print_header "Starting Gateway"

print_test "Enabling persistence in config..."
sed -i.bak 's/enabled: false/enabled: true/' config/dev.yaml
print_success "Persistence enabled"

print_test "Starting Gateway server..."
./target/release/zero_x_infinity --gateway --env dev > /tmp/gateway.log 2>&1 &
GATEWAY_PID=$!
print_info "Gateway PID: $GATEWAY_PID"

if wait_for_service "http://localhost:8080/api/v1/orders?user_id=1&limit=1"; then
    print_success "Gateway started successfully"
else
    print_error "Gateway failed to start"
    cat /tmp/gateway.log
    exit 1
fi

# ============================================
# TDengine Schema Tests
# ============================================
print_header "TDengine Schema Tests"

print_test "Checking database creation..."
if docker exec tdengine taos -s "USE trading;" >/dev/null 2>&1; then
    print_success "Database 'trading' exists"
else
    print_error "Database 'trading' not found"
fi

print_test "Checking Super Tables..."
STABLES=$(docker exec tdengine taos -s "USE trading; SHOW STABLES;" 2>/dev/null | grep -c "orders\|trades\|balances\|order_events" || echo 0)
if [ "$STABLES" -ge 4 ]; then
    print_success "All 4 Super Tables exist"
else
    print_error "Expected 4 Super Tables, found $STABLES"
fi

# ============================================
# API Endpoint Tests
# ============================================
print_header "API Endpoint Tests"

# Test 1: Orders query
print_test "GET /api/v1/orders?user_id=1001&limit=5"
RESPONSE=$(curl -s "http://localhost:8080/api/v1/orders?user_id=1001&limit=5")
CODE=$(echo "$RESPONSE" | jq -r '.code')
if [ "$CODE" = "0" ]; then
    print_success "Orders query successful (code: $CODE)"
else
    print_error "Orders query failed (code: $CODE)"
    echo "$RESPONSE" | jq .
fi

# Test 2: Order by ID query
print_test "GET /api/v1/order/100"
RESPONSE=$(curl -s "http://localhost:8080/api/v1/order/100")
CODE=$(echo "$RESPONSE" | jq -r '.code')
if [ "$CODE" = "0" ] || [ "$CODE" = "4001" ]; then
    print_success "Order by ID query successful (code: $CODE)"
else
    print_error "Order by ID query failed (code: $CODE)"
    echo "$RESPONSE" | jq .
fi

# Test 3: Trades query
print_test "GET /api/v1/trades?limit=5"
RESPONSE=$(curl -s "http://localhost:8080/api/v1/trades?limit=5")
CODE=$(echo "$RESPONSE" | jq -r '.code')
if [ "$CODE" = "0" ]; then
    print_success "Trades query successful (code: $CODE)"
else
    print_error "Trades query failed (code: $CODE)"
    echo "$RESPONSE" | jq .
fi

# Test 4: Balance query
print_test "GET /api/v1/balances?user_id=1001&asset_id=1"
RESPONSE=$(curl -s "http://localhost:8080/api/v1/balances?user_id=1001&asset_id=1")
CODE=$(echo "$RESPONSE" | jq -r '.code')
if [ "$CODE" = "0" ]; then
    DATA=$(echo "$RESPONSE" | jq -r '.data')
    if [ "$DATA" != "null" ]; then
        print_success "Balance query successful with data"
        echo "$RESPONSE" | jq '.data'
    else
        print_success "Balance query successful (no data)"
    fi
elif [ "$CODE" = "4001" ]; then
    print_success "Balance query successful (not found)"
else
    print_error "Balance query failed (code: $CODE)"
    echo "$RESPONSE" | jq .
fi

# ============================================
# Connection Stability Test
# ============================================
print_header "Connection Stability Test"

print_test "Testing connection stability (5 consecutive queries)..."
STABLE=true
for i in {1..5}; do
    RESPONSE=$(curl -s "http://localhost:8080/api/v1/orders?user_id=1001&limit=1")
    CODE=$(echo "$RESPONSE" | jq -r '.code')
    if [ "$CODE" != "0" ]; then
        print_error "Query $i failed (code: $CODE)"
        STABLE=false
        break
    fi
    sleep 0.5
done

if [ "$STABLE" = true ]; then
    print_success "Connection is stable (5/5 queries successful)"
else
    print_error "Connection is unstable"
fi

# ============================================
# Create Order Test
# ============================================
print_header "Create Order Test"

print_test "Creating a test order..."
RESPONSE=$(curl -s -X POST http://localhost:8080/api/v1/create_order \
    -H "Content-Type: application/json" \
    -H "X-User-ID: 1001" \
    -d '{
        "symbol": "BTC_USDT",
        "side": "BUY",
        "order_type": "LIMIT",
        "price": "85000.00",
        "qty": "0.001"
    }')
CODE=$(echo "$RESPONSE" | jq -r '.code')
if [ "$CODE" = "0" ]; then
    ORDER_ID=$(echo "$RESPONSE" | jq -r '.data.order_id')
    print_success "Order created successfully (ID: $ORDER_ID)"
    
    # Wait a bit for persistence
    sleep 2
    
    # Verify in TDengine
    print_test "Verifying order in TDengine..."
    COUNT=$(docker exec tdengine taos -s "USE trading; SELECT COUNT(*) FROM orders WHERE order_id = $ORDER_ID;" 2>/dev/null | grep -oE '[0-9]+' | tail -1)
    if [ "$COUNT" -gt 0 ]; then
        print_success "Order found in TDengine"
    else
        print_error "Order not found in TDengine"
    fi
else
    print_error "Failed to create order (code: $CODE)"
    echo "$RESPONSE" | jq .
fi

# ============================================
# Data Verification
# ============================================
print_header "Data Verification"

print_test "Checking orders table..."
ORDER_COUNT=$(docker exec tdengine taos -s "USE trading; SELECT COUNT(*) FROM orders;" 2>/dev/null | grep -oE '[0-9]+' | tail -1 || echo 0)
print_info "Orders in database: $ORDER_COUNT"

print_test "Checking balances table..."
BALANCE_COUNT=$(docker exec tdengine taos -s "USE trading; SELECT COUNT(*) FROM balances;" 2>/dev/null | grep -oE '[0-9]+' | tail -1 || echo 0)
print_info "Balance snapshots in database: $BALANCE_COUNT"

# ============================================
# Test Summary
# ============================================
print_header "Test Summary"

TOTAL_TESTS=$((TESTS_PASSED + TESTS_FAILED))
echo -e "Total Tests: $TOTAL_TESTS"
echo -e "${GREEN}Passed: $TESTS_PASSED${NC}"
echo -e "${RED}Failed: $TESTS_FAILED${NC}"

if [ $TESTS_FAILED -eq 0 ]; then
    echo -e "\n${GREEN}✓ All tests passed!${NC}\n"
    exit 0
else
    echo -e "\n${RED}✗ Some tests failed${NC}\n"
    exit 1
fi
