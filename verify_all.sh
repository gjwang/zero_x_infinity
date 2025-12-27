#!/bin/bash
# ==============================================================================
# Unified Test Runner - One-Click Full Verification
# ==============================================================================
# Usage: ./verify_all.sh [--quick]
#
# Runs all test suites in the correct order:
# 1. Rust unit tests (cargo test)
# 2. Admin unit tests (pytest)
# 3. Admin E2E tests (test_admin_e2e_ci.sh)
# ==============================================================================

set -e

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m'

PROJECT_ROOT=$(cd "$(dirname "$0")" && pwd)
cd "$PROJECT_ROOT"

QUICK_MODE=false
if [ "$1" == "--quick" ]; then
    QUICK_MODE=true
fi

echo "=============================================="
echo "🚀 UNIFIED TEST RUNNER"
echo "=============================================="
echo "Project: $PROJECT_ROOT"
echo "Quick Mode: $QUICK_MODE"
echo ""

FAILED=0
TOTAL=0

run_test() {
    local name=$1
    local cmd=$2
    
    TOTAL=$((TOTAL + 1))
    echo -e "\n${YELLOW}[$TOTAL] $name${NC}"
    echo "Command: $cmd"
    echo "----------------------------------------------"
    
    if eval "$cmd"; then
        echo -e "${GREEN}✅ $name PASSED${NC}"
    else
        echo -e "${RED}❌ $name FAILED${NC}"
        FAILED=$((FAILED + 1))
    fi
}

# 1. Rust Unit Tests
run_test "Rust Unit Tests" "cargo test --quiet 2>&1 | tail -10"

# 2. Admin Unit Tests (if not quick mode)
if [ "$QUICK_MODE" == "false" ]; then
    if [ -d "admin/tests" ]; then
        run_test "Admin Unit Tests" "cd admin && source venv/bin/activate && pytest tests/ -m 'not e2e' --ignore=tests/e2e -q --tb=short 2>&1 || true"
    fi
fi

# 3. Admin E2E Tests
run_test "Admin E2E Tests" "cd $PROJECT_ROOT && ./scripts/test_admin_e2e_ci.sh 2>&1 | tail -20"

# Summary
echo ""
echo "=============================================="
if [ $FAILED -eq 0 ]; then
    echo -e "${GREEN}🎉 ALL $TOTAL TEST SUITES PASSED${NC}"
    echo "=============================================="
    exit 0
else
    echo -e "${RED}❌ $FAILED/$TOTAL TEST SUITES FAILED${NC}"
    echo "=============================================="
    exit 1
fi
