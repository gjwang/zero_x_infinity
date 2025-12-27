#!/bin/bash
# Admin Dashboard Verified Runner - Real QA One-Click Tool
# Verifies: Unit Tests + DB Schema + E2E Propagation

set -e

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
NC='\033[0m'

echo "===================================================="
echo "🚀 Admin Dashboard VERIFIED RUNNER"
echo "===================================================="

cd "$(dirname "$BASH_SOURCE[0]")"

# 1. Environment Check
source ../scripts/lib/db_env.sh
if [ -d ".venv" ]; then
    source .venv/bin/activate
elif [ -d "venv" ]; then
    source venv/bin/activate
else
    echo "Error: Virtual environment not found (.venv or venv)"
    exit 1
fi

# 2. Unit Tests
# 2. Unit Tests
echo -e "\n[1/3] Running Unit Tests (Schemas & Logic)..."
pytest tests/ -m "not e2e" --ignore=tests/e2e -q --tb=short

# 3. DB Integrity & E2E
echo -e "\n[2/3] Checking DB Schema & Propagation..."
# Start server in background
echo "Cleaning up any stale test data..."
python cleanup_test_data.py

uvicorn main:app --host 127.0.0.1 --port $ADMIN_PORT > /tmp/admin_verified.log 2>&1 &
ADMIN_PID=$!

# Ensure cleanup
trap "kill $ADMIN_PID" EXIT

echo "Waiting for Admin Dashboard to start..."
sleep 5

# Run Pytest E2E Suite (QA New Scripts)
echo -e "\n[3/3] Running QA E2E Suite..."
pytest tests/e2e/ -v
PYTEST_E2E=$?

# Run the real E2E verifies
python tests/integration/test_admin_gateway_e2e.py
E2E_RESULT=$?

if [ $PYTEST_E2E -ne 0 ] || [ $E2E_RESULT -ne 0 ]; then
    echo -e "\n${RED}❌ VERIFICATION FAILED${NC}"
    echo "Pytest E2E: $PYTEST_E2E"
    echo "Legacy E2E: $E2E_RESULT"
    echo "Check /tmp/admin_verified.log for details"
    exit 1
fi

echo "===================================================="
echo "🏁 All quality gates PASSED"
echo "===================================================="
