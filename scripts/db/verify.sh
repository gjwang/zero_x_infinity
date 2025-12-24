#!/bin/bash
# =============================================================================
# Pre-Flight Environment Check
# =============================================================================
# Validates that the environment meets all requirements before running tests
# or starting the Gateway.
#
# Usage:
#   ./scripts/pre-flight-check.sh
#
# Exit codes:
#   0 = All checks passed
#   1 = One or more checks failed
# =============================================================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

# Source unified configuration
source "$PROJECT_ROOT/scripts/lib/db_env.sh"

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m'

CHECKS_PASSED=0
CHECKS_FAILED=0

check_pass() {
    echo -e "${GREEN}✓${NC} $1"
    ((CHECKS_PASSED++))
}

check_fail() {
    echo -e "${RED}✗${NC} $1"
    ((CHECKS_FAILED++))
}

check_warn() {
    echo -e "${YELLOW}⚠${NC} $1"
}

echo "═══════════════════════════════════════════════════════════"
echo " Pre-Flight Environment Check"
echo "═══════════════════════════════════════════════════════════"
echo ""

# =============================================================================
# PostgreSQL Checks
# =============================================================================
echo "PostgreSQL Checks:"
echo "  Connection: ${PG_USER}@${PG_HOST}:${PG_PORT}/${PG_DB}"

if pg_check; then
    check_pass "PostgreSQL connection successful"
    
    # Check PostgreSQL version
    PG_VERSION=$(psql -h "$PG_HOST" -p "$PG_PORT" -U "$PG_USER" -d "$PG_DB" -t -c "SHOW server_version;" | tr -d ' \n' | cut -d'.' -f1)
    if [ "$PG_VERSION" -ge 16 ]; then
        check_pass "PostgreSQL version: $PG_VERSION (>= 16)"
    else
        check_warn "PostgreSQL version: $PG_VERSION (recommended: >= 16)"
    fi
    
    # Check required tables
    TABLES=$(psql -h "$PG_HOST" -p "$PG_PORT" -U "$PG_USER" -d "$PG_DB" -t -c "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = 'public'" | tr -d ' ')
    if [ "$TABLES" -ge 8 ]; then
        check_pass "PostgreSQL tables: $TABLES found"
    else
        check_fail "PostgreSQL tables: only $TABLES found (expected >= 8)"
        echo "     Run: ./scripts/db/init.sh pg"
    fi
else
    check_fail "PostgreSQL connection failed"
    echo "     Check if PostgreSQL is running: docker ps | grep postgres"
    echo "     Run: docker-compose up -d postgres"
fi

echo ""

# =============================================================================
# TDengine Checks
# =============================================================================
echo "TDengine Checks:"
echo "  Connection: ${TD_USER}@${TD_HOST}:${TD_PORT_REST}/${TD_DB}"

if td_check; then
    check_pass "TDengine connection successful"
    
    # Check database exists
    DB_EXISTS=$(curl -s -u "${TD_USER}:${TD_PASSWORD}" -d "SHOW DATABASES" "${TD_REST_URL}/rest/sql" | grep -c "\"${TD_DB}\"" || echo 0)
    if [ "$DB_EXISTS" -gt 0 ]; then
        check_pass "TDengine database '${TD_DB}' exists"
        
        # CRITICAL: Check precision
        PRECISION=$(curl -s -u "${TD_USER}:${TD_PASSWORD}" -d "SHOW CREATE DATABASE ${TD_DB}" "${TD_REST_URL}/rest/sql" | grep -o "PRECISION '[^']*'" | cut -d"'" -f2 || echo "unknown")
        
        if [ "$PRECISION" = "us" ]; then
            check_pass "TDengine precision: 'us' (correct)"
        elif [ "$PRECISION" = "ms" ]; then
            check_fail "TDengine precision: 'ms' (WRONG! Expected 'us')"
            echo "     This will cause 'Timestamp data out of range' errors!"
            echo "     Fix: DROP DATABASE ${TD_DB}; then run ./scripts/db/init.sh td"
        elif [ "$PRECISION" = "ns" ]; then
            check_fail "TDengine precision: 'ns' (WRONG! Expected 'us')"
            echo "     Fix: DROP DATABASE ${TD_DB}; then run ./scripts/db/init.sh td"
        else
            check_warn "TDengine precision: unknown"
        fi
        
        # Check super tables using REST API response
        # We look for the table names in the output
        STABLES_COUNT=$(curl -s -u "${TD_USER}:${TD_PASSWORD}" -d "SHOW ${TD_DB}.STABLES" "${TD_REST_URL}/rest/sql" | grep -oE 'orders|trades|balances|klines|order_events' | sort | uniq | wc -l | tr -d ' ')
        
        if [ "$STABLES_COUNT" -ge 4 ]; then
            check_pass "TDengine super tables: $STABLES_COUNT found"
        else
            check_fail "TDengine super tables: only $STABLES_COUNT found (expected >= 4)"
            echo "     Run: ./scripts/db/init.sh td"
        fi
    else
        check_fail "TDengine database '${TD_DB}' not found"
        echo "     Run: ./scripts/db/init.sh td"
    fi
else
    check_fail "TDengine connection failed"
    echo "     Check if TDengine is running: docker ps | grep tdengine"
    echo "     Run: docker-compose up -d tdengine"
fi

echo ""

# =============================================================================
# Port Checks
# =============================================================================
echo "Port Availability:"

# Check Gateway port (8080)
if lsof -i :8080 > /dev/null 2>&1; then
    check_warn "Port 8080 is in use (Gateway may already be running)"
else
    check_pass "Port 8080 is available"
fi

echo ""

# =============================================================================
# Summary
# =============================================================================
TOTAL=$((CHECKS_PASSED + CHECKS_FAILED))
echo "═══════════════════════════════════════════════════════════"
echo " Summary: $CHECKS_PASSED/$TOTAL checks passed"
echo "═══════════════════════════════════════════════════════════"

if [ $CHECKS_FAILED -eq 0 ]; then
    echo -e "${GREEN}✓ Environment is ready${NC}"
    exit 0
else
    echo -e "${RED}✗ Environment has issues - please fix before proceeding${NC}"
    exit 1
fi
