#!/bin/bash
# Integration test script for Phase 0x0A Account System
# Tests Gateway endpoints for assets and symbols

set -e  # Exit on error

echo "=== Phase 0x0A Integration Test ==="
echo ""

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Test counters
TESTS_TOTAL=0
TESTS_PASSED=0
TESTS_FAILED=0

# Test result tracking
declare -a FAILED_TESTS

# ============================================================================
# Helper Functions
# ============================================================================

log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[PASS]${NC} $1"
    ((TESTS_PASSED++))
}

log_error() {
    echo -e "${RED}[FAIL]${NC} $1"
    ((TESTS_FAILED++))
    FAILED_TESTS+=("$1")
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

test_start() {
    ((TESTS_TOTAL++))
    log_info "Test $TESTS_TOTAL: $1"
}

# ============================================================================
# Pre-flight Checks
# ============================================================================

log_info "========================================="
log_info "Phase 0x0A Integration Test"
log_info "========================================="
echo ""

# Check if Gateway is running
test_start "Check if Gateway is already running"
if lsof -Pi :8080 -sTCP:LISTEN -t >/dev/null 2>&1; then
    log_warn "Gateway already running on port 8080"
    log_info "Attempting to stop existing Gateway..."
    pkill -f "zero_x_infinity" || true
    sleep 2
    
    if lsof -Pi :8080 -sTCP:LISTEN -t >/dev/null 2>&1; then
        log_error "Failed to stop existing Gateway"
        exit 1
    fi
    log_success "Existing Gateway stopped"
else
    log_success "Port 8080 is available"
fi

# Check PostgreSQL connection
test_start "Check PostgreSQL connection"
if docker exec postgres psql -U trading -d trading -c "SELECT 1" >/dev/null 2>&1; then
    log_success "PostgreSQL is accessible"
else
    log_error "PostgreSQL is not accessible"
    log_info "Please ensure PostgreSQL Docker container is running:"
    log_info "  docker ps | grep postgres"
    exit 1
fi

# ============================================================================
# Build and Start Gateway
# ============================================================================

test_start "Build Gateway"
log_info "Running: cargo build --release"
if cargo build --release 2>&1 | tail -5; then
    log_success "Gateway built successfully"
else
    log_error "Gateway build failed"
    exit 1
fi

test_start "Start Gateway in background"
log_info "Starting Gateway on port 8080..."
./target/release/zero_x_infinity gateway > /tmp/gateway.log 2>&1 &
GATEWAY_PID=$!

# Wait for Gateway to start
log_info "Waiting for Gateway to start (PID: $GATEWAY_PID)..."
for i in {1..10}; do
    if curl -s http://localhost:8080/health > /dev/null 2>&1; then
        log_success "Gateway started successfully"
        break
    fi
    if [ $i -eq 10 ]; then
        log_error "Gateway failed to start within 10 seconds"
        log_info "Gateway log:"
        cat /tmp/gateway.log
else
    echo -e "${RED}❌ Symbols endpoint failed${NC}"
    kill $GATEWAY_PID 2>/dev/null || true
    exit 1
fi

# Verify BTC_USDT exists
if echo "$SYMBOLS_RESPONSE" | jq -e '.data | map(.symbol) | contains(["BTC_USDT"])' > /dev/null 2>&1; then
    echo -e "${GREEN}✅ Found expected symbol: BTC_USDT${NC}"
else
    echo -e "${RED}❌ Missing expected symbol${NC}"
    kill $GATEWAY_PID 2>/dev/null || true
    exit 1
fi

# Step 6: Cleanup
echo -e "${YELLOW}[6/6]${NC} Cleaning up..."
kill $GATEWAY_PID 2>/dev/null || true
wait $GATEWAY_PID 2>/dev/null || true
echo -e "${GREEN}✅ Gateway stopped${NC}"

# Summary
echo ""
echo "=== Integration Test Summary ==="
echo -e "${GREEN}✅ All tests passed!${NC}"
echo ""
echo "Tested endpoints:"
echo "  - GET /api/v1/assets"
echo "  - GET /api/v1/symbols"
echo ""
echo "To stop PostgreSQL:"
echo "  docker-compose down"
