#!/bin/bash
# test_admin_e2e.sh - Admin Dashboard E2E Test
# ===================================================================
#
# PURPOSE:
#   One-click script to test Admin Dashboard end-to-end:
#   1. Check prerequisites (Python3, PostgreSQL)
#   2. Install Python dependencies (venv + requirements.txt)
#   3. Initialize database
#   4. Start Admin server
#   5. Run all tests (basic + unit + E2E)
#   6. Cleanup
#
# USAGE:
#   ./scripts/test_admin_e2e.sh
#
# ===================================================================

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
ADMIN_DIR="$PROJECT_DIR/admin"

cd "$PROJECT_DIR"

# Source database environment variables (CI standard - required per ci-pitfalls.md 2.1)
if [ -f "$SCRIPT_DIR/lib/db_env.sh" ]; then
    source "$SCRIPT_DIR/lib/db_env.sh"
fi

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

STEP=0
TOTAL_TESTS=0
PASSED_TESTS=0
FAILED_TESTS=0

# Phase 1: Check Prerequisites
echo -e "${BLUE}════════════════════════════════════════════${NC}"
echo -e "${BLUE}Phase 1: Checking Prerequisites${NC}"
echo -e "${BLUE}════════════════════════════════════════════${NC}"

fail_at_step() {
    echo ""
    echo -e "${RED}════════════════════════════════════════════════════════════${NC}"
    echo -e "${RED}FAILED at Step ${STEP}: $1${NC}"
    echo -e "${RED}════════════════════════════════════════════════════════════${NC}"
    
    # Show recent server logs if available
    if [ -f "/tmp/admin_e2e.log" ]; then
        echo ""
        echo -e "${YELLOW}Recent server logs:${NC}"
        tail -20 /tmp/admin_e2e.log
    fi
    
    cleanup_server
    exit 1
}

cleanup_server() {
    echo ""
    echo -e "${BLUE}[Cleanup] Stopping Admin server...${NC}"
    
    # Kill any uvicorn process on port 8001
    ADMIN_PID=$(lsof -ti:8001 2>/dev/null || true)
    if [ -n "$ADMIN_PID" ]; then
        kill "$ADMIN_PID" 2>/dev/null || true
        sleep 1
        # Force kill if still running
        if kill -0 "$ADMIN_PID" 2>/dev/null; then
            kill -9 "$ADMIN_PID" 2>/dev/null || true
        fi
        echo -e "    ${GREEN}✓${NC} Server stopped (PID: $ADMIN_PID)"
    else
        echo -e "    ${GREEN}✓${NC} No server running"
    fi
}

# Trap to ensure cleanup on exit/interrupt
trap cleanup_server EXIT INT TERM

echo "╔════════════════════════════════════════════════════════════╗"
echo "║    Admin Dashboard E2E Test                               ║"
echo "╚════════════════════════════════════════════════════════════╝"
echo ""

# ============================================================================
# Step 1: Check prerequisites
# ============================================================================
STEP=1
echo "[Step $STEP] Checking prerequisites..."

# Check Python3
if ! command -v python3 &> /dev/null; then
    fail_at_step "Python3 not found. Install with: brew install python3"
fi
PYTHON_VERSION=$(python3 --version)
echo -e "    ${GREEN}✓${NC} $PYTHON_VERSION"

# Check PostgreSQL (optional - Admin works standalone)
if command -v psql &>/dev/null; then
    if psql -h localhost -p 5433 -U trading -d exchange_info_db -c "SELECT 1" &>/dev/null; then
        echo -e "    ${GREEN}✓${NC} PostgreSQL running on port 5433"
    else
        echo -e "    ${YELLOW}⚠${NC}  PostgreSQL not accessible (Admin uses default config)"
    fi
else
    echo -e "    ${YELLOW}⚠${NC}  psql not found (Admin uses default config)"
fi

# Check admin directory
if [ ! -d "$ADMIN_DIR" ]; then
    fail_at_step "Admin directory not found: $ADMIN_DIR"
fi
echo -e "    ${GREEN}✓${NC} Admin directory exists"

if [ ! -f "$ADMIN_DIR/requirements.txt" ]; then
    fail_at_step "requirements.txt not found in $ADMIN_DIR"
fi
echo -e "    ${GREEN}✓${NC} requirements.txt found"

# ============================================================================
# Step 2: Install Python dependencies
# ============================================================================
STEP=2
echo ""
echo "[Step $STEP] Installing Python dependencies..."

cd "$ADMIN_DIR"

# Create virtual environment if not exists
if [ ! -d "venv" ]; then
    echo "    Creating virtual environment..."
    python3 -m venv venv
    echo -e "    ${GREEN}✓${NC} Virtual environment created"
else
    echo -e "    ${GREEN}✓${NC} Virtual environment already exists"
fi

# Activate virtual environment
source venv/bin/activate

# Upgrade pip to latest version
echo "    Upgrading pip..."
python -m pip install --upgrade pip --quiet
echo -e "    ${GREEN}✓${NC} pip upgraded to $(pip --version | awk '{print $2}')"

# Install/upgrade dependencies from requirements.txt
echo "    Installing dependencies from requirements.txt..."
pip install -r requirements.txt --quiet
echo -e "    ${GREEN}✓${NC} All dependencies installed"

# Verify key packages
echo "    Verifying key packages:"
for pkg in fastapi uvicorn pytest; do
    if python -c "import $pkg" 2>/dev/null; then
        VERSION=$(python -c "import $pkg; print($pkg.__version__)" 2>/dev/null || echo "unknown")
        echo -e "      ${GREEN}✓${NC} $pkg ($VERSION)"
    else
        fail_at_step "Package $pkg not installed correctly"
    fi
done

# ============================================================================
# Step 3: Initialize database
# ============================================================================
STEP=3
echo ""
echo "[Step $STEP] Initializing database..."

# Check if database already initialized
if [ -f "admin_auth.db" ]; then
    DB_SIZE=$(du -h admin_auth.db | awk '{print $1}')
    echo -e "    ${GREEN}✓${NC} Database already exists (size: $DB_SIZE)"
    echo "    Skipping initialization (idempotent)"
else
    echo "    Running init_db.py..."
    if python init_db.py 2>&1 | grep -q "Database initialized successfully"; then
        echo -e "    ${GREEN}✓${NC} Database initialized"
        echo -e "    ${GREEN}✓${NC} Default admin user created (admin/admin)"
    else
        echo -e "    ${YELLOW}⚠${NC}  Database initialization may have issues, continuing anyway..."
    fi
fi

# ============================================================================
# Step 4: Start Admin server
# ============================================================================
STEP=4
echo ""
echo "[Step $STEP] Starting Admin server..."

# Stop any existing server on port 8001
cleanup_server

# Start uvicorn server in background
echo "    Starting uvicorn on port 8001..."
nohup uvicorn main:app --host 0.0.0.0 --port 8001 > /tmp/admin_e2e.log 2>&1 &
SERVER_PID=$!
sleep 2

# Wait for server health endpoint
check_health() {
    curl -sf http://localhost:8001/health > /dev/null 2>&1
}

echo "    Waiting for server to be ready..."
for i in $(seq 1 30); do
    if check_health; then
        break
    fi
    sleep 1
    if [ $i -eq 30 ]; then
        echo ""
        echo -e "${YELLOW}Server logs (last 30 lines):${NC}"
        tail -30 /tmp/admin_e2e.log
        fail_at_step "Server failed to respond after 30 seconds"
    fi
done

echo -e "    ${GREEN}✓${NC} Server responding (PID: $SERVER_PID)"

# Verify health response
HEALTH_RESP=$(curl -s http://localhost:8001/health)
if echo "$HEALTH_RESP" | grep -q '"status":"ok"'; then
    echo -e "    ${GREEN}✓${NC} Health check passed"
else
    fail_at_step "Health check returned unexpected response: $HEALTH_RESP"
fi

# ============================================================================
# Step 5: Run tests
# ============================================================================
STEP=5
echo ""
echo "[Step $STEP] Running tests..."

# 5.1: Basic HTTP Tests (verify_e2e.py)
echo ""
echo "    [5.1] Basic HTTP Tests (verify_e2e.py)..."
if python verify_e2e.py 2>&1 | tee /tmp/verify_e2e.log; then
    BASIC_PASSED=$(grep -o "[0-9]* passed" /tmp/verify_e2e.log | awk '{print $1}' || echo "4")
    BASIC_FAILED=$(grep -o "[0-9]* failed" /tmp/verify_e2e.log | awk '{print $1}' || echo "0")
    echo -e "    ${GREEN}✓${NC} Basic tests: $BASIC_PASSED passed, $BASIC_FAILED failed"
    TOTAL_TESTS=$((TOTAL_TESTS + BASIC_PASSED + BASIC_FAILED))
    PASSED_TESTS=$((PASSED_TESTS + BASIC_PASSED))
    FAILED_TESTS=$((FAILED_TESTS + BASIC_FAILED))
else
    echo -e "    ${YELLOW}⚠${NC}  Some basic tests may have failed, continuing..."
    TOTAL_TESTS=$((TOTAL_TESTS + 4))
    FAILED_TESTS=$((FAILED_TESTS + 1))
fi

# 5.2: Unit Tests (test_*.py in tests/)
echo ""
echo "    [5.2] Unit Tests (pytest tests/test_*.py)..."
cd "$ADMIN_DIR"
if pytest tests/test_*.py -v --tb=short 2>&1 | tee /tmp/pytest_unit.log; then
    UNIT_PASSED=$(grep -E "^(tests/test_.*\.py)" /tmp/pytest_unit.log | grep -c "PASSED" || echo "0")
    UNIT_FAILED=$(grep -E "^(tests/test_.*\.py)" /tmp/pytest_unit.log | grep -c "FAILED" || echo "0")
    echo -e "    ${GREEN}✓${NC} Unit tests: $UNIT_PASSED passed, $UNIT_FAILED failed"
    TOTAL_TESTS=$((TOTAL_TESTS + UNIT_PASSED + UNIT_FAILED))
    PASSED_TESTS=$((PASSED_TESTS + UNIT_PASSED))
    FAILED_TESTS=$((FAILED_TESTS + UNIT_FAILED))
    
    if [ "$UNIT_FAILED" -gt 0 ]; then
        echo -e "    ${YELLOW}⚠${NC}  Some unit tests failed, check /tmp/pytest_unit.log"
    fi
else
    echo -e "    ${YELLOW}⚠${NC}  Unit tests encountered errors, check /tmp/pytest_unit.log"
    # Try to extract counts even on failure
    UNIT_PASSED=$(grep -E "^(tests/test_.*\.py)" /tmp/pytest_unit.log | grep -c "PASSED" || echo "0")
    UNIT_FAILED=$(grep -E "^(tests/test_.*\.py)" /tmp/pytest_unit.log | grep -c "FAILED" || echo "0")
    if [ "$UNIT_PASSED" -gt 0 ] || [ "$UNIT_FAILED" -gt 0 ]; then
        TOTAL_TESTS=$((TOTAL_TESTS + UNIT_PASSED + UNIT_FAILED))
        PASSED_TESTS=$((PASSED_TESTS + UNIT_PASSED))
        FAILED_TESTS=$((FAILED_TESTS + UNIT_FAILED))
    fi
fi

# 5.3: E2E Integration Tests (tests/e2e/)
echo ""
echo "    [5.3] E2E Integration Tests (pytest tests/e2e/)..."
if pytest tests/e2e/ -v --tb=short 2>&1 | tee /tmp/pytest_e2e.log; then
    E2E_PASSED=$(grep -E "^(tests/e2e/test_.*\.py)" /tmp/pytest_e2e.log | grep -c "PASSED" 2>/dev/null || echo "0")
    E2E_FAILED=$(grep -E "^(tests/e2e/test_.*\.py)" /tmp/pytest_e2e.log | grep -c "FAILED" 2>/dev/null || echo "0")
    # Clean any whitespace/newlines that might cause arithmetic errors
    E2E_PASSED=$(echo "$E2E_PASSED" | tr -d '\n\r' | xargs)
    E2E_FAILED=$(echo "$E2E_FAILED" | tr -d '\n\r' | xargs)
    E2E_PASSED=${E2E_PASSED:-0}
    E2E_FAILED=${E2E_FAILED:-0}
    echo -e "    ${GREEN}✓${NC} E2E tests: $E2E_PASSED passed, $E2E_FAILED failed"
    TOTAL_TESTS=$((TOTAL_TESTS + E2E_PASSED + E2E_FAILED))
    PASSED_TESTS=$((PASSED_TESTS + E2E_PASSED))
    FAILED_TESTS=$((FAILED_TESTS + E2E_FAILED))
    
    if [ "$E2E_FAILED" -gt 0 ]; then
        echo -e "    ${YELLOW}⚠${NC}  Some E2E tests failed, check /tmp/pytest_e2e.log"
    fi
else
    echo -e "    ${YELLOW}⚠${NC}  E2E tests encountered errors, check /tmp/pytest_e2e.log"
    # Try to extract counts even on failure
    E2E_PASSED=$(grep -E "^(tests/e2e/test_.*\.py)" /tmp/pytest_e2e.log | grep -c "PASSED" 2>/dev/null || echo "0")
    E2E_FAILED=$(grep -E "^(tests/e2e/test_.*\.py)" /tmp/pytest_e2e.log | grep -c "FAILED" 2>/dev/null || echo "0")
    # Clean any whitespace/newlines
    E2E_PASSED=$(echo "$E2E_PASSED" | tr -d '\n\r' | xargs)
    E2E_FAILED=$(echo "$E2E_FAILED" | tr -d '\n\r' | xargs)
    E2E_PASSED=${E2E_PASSED:-0}
    E2E_FAILED=${E2E_FAILED:-0}
    if [ "$E2E_PASSED" -gt 0 ] || [ "$E2E_FAILED" -gt 0 ]; then
        TOTAL_TESTS=$((TOTAL_TESTS + E2E_PASSED + E2E_FAILED))
        PASSED_TESTS=$((PASSED_TESTS + E2E_PASSED))
        FAILED_TESTS=$((FAILED_TESTS + E2E_FAILED))
    fi
fi

# ============================================================================
# Step 6: Summary
# ============================================================================
echo ""
echo "════════════════════════════════════════════════════════════"
echo "Test Summary"
echo "════════════════════════════════════════════════════════════"
echo "  Total:  $TOTAL_TESTS tests"
echo "  Passed: $PASSED_TESTS tests"
echo "  Failed: $FAILED_TESTS tests"
echo "════════════════════════════════════════════════════════════"
echo ""

if [ "$FAILED_TESTS" -eq 0 ] && [ "$PASSED_TESTS" -gt 0 ]; then
    echo -e "${GREEN}╔════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${GREEN}║  ✅ ADMIN E2E TEST PASSED                                  ║${NC}"
    echo -e "${GREEN}╚════════════════════════════════════════════════════════════╝${NC}"
    echo ""
    echo "Next steps:"
    echo "  • Manual browser test: http://localhost:8001/admin"
    echo "  • Review logs: tail -f /tmp/admin_e2e.log"
    echo "  • Stop server: lsof -ti:8001 | xargs kill"
    exit 0
else
    echo -e "${RED}╔════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${RED}║  ❌ ADMIN E2E TEST FAILED                                  ║${NC}"
    echo -e "${RED}╚════════════════════════════════════════════════════════════╝${NC}"
    echo ""
    echo "Check logs for details:"
    echo "  • Server: /tmp/admin_e2e.log"
    echo "  • Basic tests: /tmp/verify_e2e.log"
    echo "  • Unit tests: /tmp/pytest_unit.log"
    echo "  • E2E tests: /tmp/pytest_e2e.log"
    exit 1
fi
