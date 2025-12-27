#!/bin/bash
# ==============================================================================
# Unified Test Runner - One-Click Full Verification
# ==============================================================================
# Usage: ./scripts/run_admin_full_suite.sh [--quick]
#
# Runs all test suites in the correct order:
# 1. Rust unit tests (cargo test)
# 2. Admin unit tests (pytest)
# 3. Admin E2E tests (run_admin_gateway_e2e.sh)
#
# IMPORTANT: This script MUST fail if any test fails!
# ==============================================================================

set -e  # Exit on first error

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m'

PROJECT_ROOT=$(cd "$(dirname "$0")/.." && pwd)
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
    
    # Run command and capture exit code
    set +e  # Temporarily allow errors
    eval "$cmd"
    local exit_code=$?
    set -e
    
    if [ $exit_code -eq 0 ]; then
        echo -e "${GREEN}✅ $name PASSED${NC}"
    else
        echo -e "${RED}❌ $name FAILED (exit code: $exit_code)${NC}"
        FAILED=$((FAILED + 1))
    fi
}

# Helper: Activate Python venv (standardized: admin/venv/)
activate_admin_venv() {
    cd "$PROJECT_ROOT/admin"
    
    # Standard: admin/venv/
    if [ ! -d "venv" ]; then
        echo -e "${RED}ERROR: Python venv not found at admin/venv/${NC}"
        echo "Setup: cd admin && python3 -m venv venv && source venv/bin/activate && pip install -r requirements.txt"
        return 1
    fi
    
    source venv/bin/activate
    
    # Verify Python version
    PYTHON_VERSION=$(python --version 2>&1)
    echo "Using: $PYTHON_VERSION"
    
    # Verify key packages installed
    if ! python -c "import pydantic" 2>/dev/null; then
        echo -e "${RED}ERROR: pydantic not installed in venv!${NC}"
        return 1
    fi
    
    return 0
}

# 1. Rust Unit Tests
run_test "Rust Unit Tests" "cargo test --quiet 2>&1 | tail -10"

# 2. Admin Unit Tests (if not quick mode)
if [ "$QUICK_MODE" == "false" ]; then
    if [ -d "admin/tests" ]; then
        echo -e "\n${YELLOW}[2] Admin Unit Tests${NC}"
        echo "----------------------------------------------"
        
        TOTAL=$((TOTAL + 1))
        
        # Activate venv properly
        if activate_admin_venv; then
            # Run pytest WITHOUT || true - must fail on error!
            pytest tests/ -m 'not e2e' --ignore=tests/e2e --ignore=tests/integration -q --tb=short 2>&1
            PYTEST_EXIT=$?
            
            if [ $PYTEST_EXIT -eq 0 ]; then
                echo -e "${GREEN}✅ Admin Unit Tests PASSED${NC}"
            else
                echo -e "${RED}❌ Admin Unit Tests FAILED (exit code: $PYTEST_EXIT)${NC}"
                FAILED=$((FAILED + 1))
            fi
            
            deactivate 2>/dev/null || true
        else
            echo -e "${RED}❌ Admin Unit Tests FAILED (venv setup error)${NC}"
            FAILED=$((FAILED + 1))
        fi
        
        cd "$PROJECT_ROOT"
    fi
fi

# 3. Admin E2E Tests
run_test "Admin E2E Tests" "cd $PROJECT_ROOT && ./scripts/run_admin_gateway_e2e.sh 2>&1 | tail -20"

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
