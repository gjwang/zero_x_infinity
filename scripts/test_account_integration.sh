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

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Source unified database configuration
source "$PROJECT_ROOT/scripts/lib/db_env.sh"

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

# Helper to call API and verify response
call_api() {
    local method="$1"
    local url="$2"
    local name="$3"
    
    # Capture HTTP status code and response body
    local response_file=$(mktemp)
    local http_code=$(curl -s -X "$method" -w "%{http_code}" -o "$response_file" "$url")
    local response_body=$(cat "$response_file")
    rm -f "$response_file"
    
    if [ "$http_code" -ne 200 ]; then
        log_error "$name failed with HTTP $http_code"
        log_info "URL: $url"
        log_info "Response: $response_body"
        return 1
    fi
    
    echo "$response_body"
    return 0
}

# Cleanup on exit
cleanup() {
    local exit_code=$?
    if [ $exit_code -ne 0 ]; then
        echo ""
        log_warn "Test suite failed with exit code $exit_code"
        echo "=== Last 20 lines of Gateway Log (/tmp/gateway.log) ==="
        tail -n 20 /tmp/gateway.log 2>/dev/null || echo "Log not found"
        echo "========================================================"
    fi
    
    # Kill gateway if it was started in this script
    if [ -n "$GATEWAY_PID" ]; then
        kill "$GATEWAY_PID" 2>/dev/null || true
    fi
}
trap cleanup EXIT

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
elif ./scripts/db/init.sh pg --reset >/dev/null 2>&1; then
    log_success "Database initialized (reset + seed)"
else
    log_error "Failed to initialize database"
    log_info "Make sure PostgreSQL is running and init.sh exists"
    exit 1
fi

# Check PostgreSQL connection
test_start "Check PostgreSQL connection"

# In CI, PostgreSQL is a service container accessible via localhost
# Locally, it's accessed via docker exec
if [ "$CI" = "true" ]; then
    # CI environment - use psql directly or Python
    if uv run python3 -c "import psycopg2; conn = psycopg2.connect(host='localhost', dbname='exchange_info_db', user='trading', password='trading123'); conn.close()" 2>/dev/null; then
        log_success "PostgreSQL is accessible (via psycopg2)"
    elif command -v psql &>/dev/null && PGPASSWORD=trading123 psql -h localhost -U trading -d exchange_info_db -c "SELECT 1" >/dev/null 2>&1; then
        log_success "PostgreSQL is accessible (via psql)"
    else
        log_error "PostgreSQL is not accessible in CI"
        exit 1
    fi
else
    # Local environment - try docker exec with common container names or direct psql
    PG_CONTAINER=$(docker ps --format '{{.Names}}' | grep -iE '^postgres' | head -1)
    if [ -n "$PG_CONTAINER" ] && docker exec "$PG_CONTAINER" psql -U "$PG_USER" -d "$PG_DB" -c "SELECT 1" >/dev/null 2>&1; then
        log_success "PostgreSQL is accessible (via Docker: $PG_CONTAINER)"
    elif pg_check; then
        log_success "PostgreSQL is accessible (via pg_check)"
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

BINARY="./target/release/zero_x_infinity"

# Check binary existence
if [ ! -f "$BINARY" ]; then
    log_error "Gateway binary not found: $BINARY"
    log_info "Please run: cargo build --release"
    exit 1
fi

# Print version info
log_info "Binary Version: $($BINARY --version | tr '\n' ' ')"

# Check binary freshness (only locally)
if [ "$CI" != "true" ]; then
    STALE_FILES=$(find src -type f -newer "$BINARY" | head -n 5)
    
    if [ -n "$STALE_FILES" ]; then
        echo ""
        log_warn "⚠️  WARNING: Release binary is STALE!"
        log_info "The following files (and possibly others) were modified after the last release build:"
        echo "$STALE_FILES" | sed 's/^/  - /'
        echo ""
        log_info "To avoid misleading results, please run: cargo build --release"
        echo ""
    else
        log_success "Binary is up-to-date"
    fi
fi

# ============================================================================
# Prepare Test Data
# ============================================================================

test_start "Prepare test data"
TEST_DIR="test_account"
mkdir -p "$TEST_DIR"

# Copy essential config files and the FULL balances file
cp fixtures/assets_config.csv "$TEST_DIR/"
cp fixtures/symbols_config.csv "$TEST_DIR/"
cp fixtures/balances_init.csv "$TEST_DIR/"

log_success "Test data (Full Dataset) prepared in $TEST_DIR/"

test_start "Start Gateway in background"

# Port conflict detection
if curl -s http://localhost:8080/api/v1/health >/dev/null 2>&1; then
    log_error "Port 8080 is already in use by another process!"
    log_info "If a previous test run didn't clean up, try: pkill zero_x_infinity"
    exit 1
fi

log_info "Starting Gateway on port 8080..."

# Use CI config when running in CI environment
if [ "$CI" = "true" ]; then
    ENV_FLAG="--env ci"
else
    ENV_FLAG=""
fi

./target/release/zero_x_infinity --gateway $ENV_FLAG --port 8080 --input "$TEST_DIR" > /tmp/gateway.log 2>&1 &
GATEWAY_PID=$!

# Wait for Gateway to start - increased for CI stability and large dataset
log_info "Waiting for Gateway to start (max 120s)..."
READY=false
for i in {1..120}; do
    if curl -s http://localhost:8080/api/v1/health | grep -q "ok"; then
        log_success "Gateway started successfully"
        READY=true
        break
    fi
    
    # Check if process died
    if ! kill -0 $GATEWAY_PID 2>/dev/null; then
        log_error "Gateway process died"
        tail -n 20 /tmp/gateway.log 2>/dev/null || true
        exit 1
    fi
    
    [ $((i % 5)) -eq 0 ] && log_info "Waiting for Gateway... ($i/120)"
    sleep 1
done

if [ "$READY" = false ]; then
    log_error "Gateway failed to start within 120 seconds"
    exit 1
fi

# ============================================================================
# API Endpoint Tests
# ============================================================================

test_start "Test /api/v1/assets endpoint"
ASSETS_RESPONSE=$(call_api GET "http://localhost:8080/api/v1/public/assets" "Assets API")
if [ $? -eq 0 ] && echo "$ASSETS_RESPONSE" | jq -e '.code == 0' > /dev/null 2>&1; then
    ASSET_COUNT=$(echo "$ASSETS_RESPONSE" | jq '.data | length')
    log_success "Assets endpoint returned $ASSET_COUNT assets"
    
    # Verify asset structure
    if echo "$ASSETS_RESPONSE" | jq -e '.data[0] | has("asset_id", "asset", "name")' > /dev/null 2>&1; then
        log_success "Asset structure is correct"
    else
        log_error "Asset structure is incorrect"
    fi
else
    # Details already logged by call_api if it failed
    [ $? -ne 0 ] || log_error "Assets API returned business error: $ASSETS_RESPONSE"
fi

test_start "Test /api/v1/symbols endpoint"
SYMBOLS_RESPONSE=$(call_api GET "http://localhost:8080/api/v1/public/symbols" "Symbols API")
if [ $? -eq 0 ] && echo "$SYMBOLS_RESPONSE" | jq -e '.code == 0' > /dev/null 2>&1; then
    SYMBOL_COUNT=$(echo "$SYMBOLS_RESPONSE" | jq '.data | length')
    log_success "Symbols endpoint returned $SYMBOL_COUNT symbols"
    
    # Verify symbol structure
    if echo "$SYMBOLS_RESPONSE" | jq -e '.data[0] | has("symbol_id", "symbol", "base_asset")' > /dev/null 2>&1; then
        log_success "Symbol structure is correct"
    else
        log_error "Symbol structure is incorrect"
    fi
else
    [ $? -ne 0 ] || log_error "Symbols API returned business error: $SYMBOLS_RESPONSE"
fi

# Test /api/v1/exchange_info endpoint
test_start "Test /api/v1/exchange_info endpoint"
EXCHANGE_INFO_RESPONSE=$(call_api GET "http://localhost:8080/api/v1/public/exchange_info" "ExchangeInfo API")
if [ $? -eq 0 ] && echo "$EXCHANGE_INFO_RESPONSE" | jq -e '.code == 0' > /dev/null 2>&1; then
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
    [ $? -ne 0 ] || log_error "ExchangeInfo API returned business error: $EXCHANGE_INFO_RESPONSE"
fi

# ============================================================================
# Idempotency Test
# ============================================================================

test_start "Test endpoint idempotency (multiple requests)"
ASSETS_RESPONSE_2=$(curl -s http://localhost:8080/api/v1/public/assets)
if [ "$ASSETS_RESPONSE" = "$ASSETS_RESPONSE_2" ]; then
    log_success "Assets endpoint is idempotent"
else
    log_error "Assets endpoint returned different results"
fi

SYMBOLS_RESPONSE_2=$(curl -s http://localhost:8080/api/v1/public/symbols)
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
        "INSERT INTO assets_tb (asset, name, internal_scale, asset_precision, status, asset_flags) VALUES ('btc_test', 'Test', 8, 8, 0, 7)" 2>&1 || true)
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
        "INSERT INTO assets_tb (asset, name, internal_scale, asset_precision, status, asset_flags) VALUES ('TEST_ASSET', 'Test Asset', 8, 8, 1, 7)" >/dev/null 2>&1; then
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
# Cleanup (handled by trap)
# ============================================================================

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
