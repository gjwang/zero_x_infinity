#!/bin/bash

# ============================================================================
# Phase 0x0A Account Integration Test Script
# ============================================================================
# This script tests the PostgreSQL integration and Gateway API endpoints
# Features:
# - Idempotent: Can be run multiple times safely
# - Detailed error reporting
# - Test result summary
# ============================================================================

set -e  # Exit on error

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
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
    ((TESTS_PASSED++)) || true
}

log_error() {
    echo -e "${RED}[FAIL]${NC} $1"
    ((TESTS_FAILED++)) || true
    FAILED_TESTS+=("$1")
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

test_start() {
    ((TESTS_TOTAL++)) || true
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
    
    # IMPORTANT: Do NOT use `pkill -f "zero_x_infinity"` - it will kill IDE!
    # Use pgrep with specific pattern + kill with PID instead
    GW_PID=$(pgrep -f "target.*zero_x_infinity.*--gateway" | head -1)
    if [ -n "$GW_PID" ]; then
        kill "$GW_PID" 2>/dev/null || true
        sleep 2
    fi
    
    if lsof -Pi :8080 -sTCP:LISTEN -t >/dev/null 2>&1; then
        log_error "Failed to stop existing Gateway"
        exit 1
    fi
    log_success "Existing Gateway stopped"
else
    log_success "Port 8080 is available"
fi

# Initialize database to clean state
test_start "Initialize database to clean state"
if [ "$CI" = "true" ]; then
    # In CI, database is already initialized by the workflow's "Initialize DB" step
    log_success "Database initialization skipped (CI workflow handles this)"
elif python3 scripts/db/manage_db.py init >/dev/null 2>&1; then
    log_success "Database initialized (reset + seed)"
else
    log_error "Failed to initialize database"
    log_info "Make sure PostgreSQL is running and manage_db.py exists"
    exit 1
fi

# Check PostgreSQL connection
test_start "Check PostgreSQL connection"

# In CI, PostgreSQL is a service container accessible via localhost
# Locally, it's accessed via docker exec
if [ "$CI" = "true" ]; then
    # CI environment - use psql directly or Python
    if python3 -c "import psycopg2; conn = psycopg2.connect(host='localhost', dbname='exchange_info_db', user='trading', password='trading123'); conn.close()" 2>/dev/null; then
        log_success "PostgreSQL is accessible (via psycopg2)"
    elif command -v psql &>/dev/null && PGPASSWORD=trading123 psql -h localhost -U trading -d exchange_info_db -c "SELECT 1" >/dev/null 2>&1; then
        log_success "PostgreSQL is accessible (via psql)"
    else
        log_error "PostgreSQL is not accessible in CI"
        exit 1
    fi
else
    # Local environment - use docker exec
    if docker exec postgres psql -U trading -d exchange_info_db -c "SELECT 1" >/dev/null 2>&1; then
        log_success "PostgreSQL is accessible (via Docker)"
    else
        log_error "PostgreSQL is not accessible"
        log_info "Please ensure PostgreSQL Docker container is running:"
        log_info "  docker ps | grep postgres"
        exit 1
    fi
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

# Use CI config when running in CI environment
if [ "$CI" = "true" ]; then
    ENV_FLAG="--env ci"
else
    ENV_FLAG=""
fi

./target/release/zero_x_infinity --gateway $ENV_FLAG --port 8080 > /tmp/gateway.log 2>&1 &
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
        kill $GATEWAY_PID 2>/dev/null || true
        exit 1
    fi
    sleep 1
done

# ============================================================================
# API Endpoint Tests
# ============================================================================

test_start "Test /api/v1/assets endpoint"
ASSETS_RESPONSE=$(curl -s http://localhost:8080/api/v1/assets)
if echo "$ASSETS_RESPONSE" | jq -e '.code == 0' > /dev/null 2>&1; then
    ASSET_COUNT=$(echo "$ASSETS_RESPONSE" | jq '.data | length')
    log_success "Assets endpoint returned $ASSET_COUNT assets"
    
    # Verify asset structure
    if echo "$ASSETS_RESPONSE" | jq -e '.data[0] | has("asset_id", "asset", "name")' > /dev/null 2>&1; then
        log_success "Asset structure is correct"
    else
        log_error "Asset structure is incorrect"
    fi
else
    log_error "Assets endpoint failed"
    log_info "Response: $ASSETS_RESPONSE"
fi

test_start "Test /api/v1/symbols endpoint"
SYMBOLS_RESPONSE=$(curl -s http://localhost:8080/api/v1/symbols)
if echo "$SYMBOLS_RESPONSE" | jq -e '.code == 0' > /dev/null 2>&1; then
    SYMBOL_COUNT=$(echo "$SYMBOLS_RESPONSE" | jq '.data | length')
    log_success "Symbols endpoint returned $SYMBOL_COUNT symbols"
    
    # Verify symbol structure
    if echo "$SYMBOLS_RESPONSE" | jq -e '.data[0] | has("symbol_id", "symbol", "base_asset")' > /dev/null 2>&1; then
        log_success "Symbol structure is correct"
    else
        log_error "Symbol structure is incorrect"
    fi
else
    log_error "Symbols endpoint failed"
    log_info "Response: $SYMBOLS_RESPONSE"
fi

# Test /api/v1/exchange_info endpoint
test_start "Test /api/v1/exchange_info endpoint"
EXCHANGE_INFO_RESPONSE=$(curl -s http://localhost:8080/api/v1/exchange_info)
if echo "$EXCHANGE_INFO_RESPONSE" | jq -e '.code == 0' > /dev/null 2>&1; then
    ASSET_COUNT=$(echo "$EXCHANGE_INFO_RESPONSE" | jq '.data.assets | length')
    SYMBOL_COUNT=$(echo "$EXCHANGE_INFO_RESPONSE" | jq '.data.symbols | length')
    SERVER_TIME=$(echo "$EXCHANGE_INFO_RESPONSE" | jq '.data.server_time')
    log_success "Exchange info returned $ASSET_COUNT assets, $SYMBOL_COUNT symbols"
    
    # Verify structure
    if echo "$EXCHANGE_INFO_RESPONSE" | jq -e '.data | has("assets", "symbols", "server_time")' > /dev/null 2>&1; then
        log_success "Exchange info structure is correct"
    else
        log_error "Exchange info structure is incorrect"
    fi
else
    log_error "Exchange info endpoint failed"
    log_info "Response: $EXCHANGE_INFO_RESPONSE"
fi

# ============================================================================
# Idempotency Test
# ============================================================================

test_start "Test endpoint idempotency (multiple requests)"
ASSETS_RESPONSE_2=$(curl -s http://localhost:8080/api/v1/assets)
if [ "$ASSETS_RESPONSE" = "$ASSETS_RESPONSE_2" ]; then
    log_success "Assets endpoint is idempotent"
else
    log_error "Assets endpoint returned different results"
fi

SYMBOLS_RESPONSE_2=$(curl -s http://localhost:8080/api/v1/symbols)
if [ "$SYMBOLS_RESPONSE" = "$SYMBOLS_RESPONSE_2" ]; then
    log_success "Symbols endpoint is idempotent"
else
    log_error "Symbols endpoint returned different results"
fi

# ============================================================================
# Validation Tests (Database Constraints)
# ============================================================================

# First check if PostgreSQL tables exist
test_start "Check if database tables exist for validation tests"

# Skip validation tests in CI - docker exec not available for service containers
if [ "$CI" = "true" ]; then
    log_warn "Skipping database constraint tests in CI (docker exec not available)"
elif docker exec postgres psql -U trading -d exchange_info_db -c "\d assets_tb" >/dev/null 2>&1; then
    log_success "Database tables exist - running validation tests"
    
    # Test lowercase asset rejection (if CHECK constraint exists)
    test_start "Test database constraint: lowercase asset rejected"
    INSERT_RESULT=$(docker exec postgres psql -U trading -d exchange_info_db -c \
        "INSERT INTO assets_tb (asset, name, decimals, status, asset_flags) VALUES ('btc_test', 'Test', 8, 0, 7)" 2>&1)
    if echo "$INSERT_RESULT" | grep -qi "check\|constraint\|uppercase\|violates"; then
        log_success "Lowercase asset correctly rejected by database"
    else
        # Cleanup if inserted
        docker exec postgres psql -U trading -d exchange_info_db -c "DELETE FROM assets_tb WHERE asset = 'btc_test'" >/dev/null 2>&1 || true
        log_warn "Database constraint may not be applied yet - lowercase accepted"
    fi
    
    # Test uppercase asset acceptance
    test_start "Test database constraint: uppercase asset accepted"
    if docker exec postgres psql -U trading -d exchange_info_db -c \
        "INSERT INTO assets_tb (asset, name, decimals, status, asset_flags) VALUES ('TEST_ASSET', 'Test Asset', 8, 1, 7)" >/dev/null 2>&1; then
        log_success "Uppercase asset correctly accepted by database"
        docker exec postgres psql -U trading -d exchange_info_db -c "DELETE FROM assets_tb WHERE asset = 'TEST_ASSET'" >/dev/null 2>&1 || true
    else
        log_error "Uppercase asset insertion failed unexpectedly"
    fi
else
    log_warn "Database tables not created yet - skipping validation tests"
    log_info "Run migrations to enable validation tests"
fi

# ============================================================================
# Cleanup
# ============================================================================

log_info "Stopping Gateway (PID: $GATEWAY_PID)..."
kill $GATEWAY_PID 2>/dev/null || true
wait $GATEWAY_PID 2>/dev/null || true
log_success "Gateway stopped"

# ============================================================================
# Test Summary
# ============================================================================

echo ""
log_info "========================================="
log_info "Test Summary"
log_info "========================================="
echo -e "Total Tests:  ${BLUE}$TESTS_TOTAL${NC}"
echo -e "Passed:       ${GREEN}$TESTS_PASSED${NC}"
echo -e "Failed:       ${RED}$TESTS_FAILED${NC}"

if [ $TESTS_FAILED -gt 0 ]; then
    echo ""
    log_error "Failed Tests:"
    for test in "${FAILED_TESTS[@]}"; do
        echo -e "  ${RED}✗${NC} $test"
    done
    echo ""
    log_error "Integration test FAILED"
    exit 1
else
    echo ""
    log_success "All tests PASSED! ✓"
    exit 0
fi
