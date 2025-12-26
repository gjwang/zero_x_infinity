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
source .venv/bin/activate || source venv/bin/activate

# 2. Unit Tests
echo -e "\n[1/3] Running Unit Tests (Schemas & Logic)..."
pytest tests/ -m "not e2e" -q --tb=short

# 3. DB Integrity & E2E
echo -e "\n[2/3] Checking DB Schema & Propagation..."
# Start server in background
uvicorn main:app --host 127.0.0.1 --port 8001 > /tmp/admin_verified.log 2>&1 &
ADMIN_PID=$!

# Ensure cleanup
trap "kill $ADMIN_PID" EXIT

echo "Waiting for Admin Dashboard to start..."
sleep 3

# Run the real E2E verifies
python test_admin_gateway_e2e.py
E2E_RESULT=$?

if [ $E2E_RESULT -eq 0 ]; then
    echo -e "\n${GREEN}✅ VERIFICATION SUCCESSFUL${NC}"
else
    echo -e "\n${RED}❌ VERIFICATION FAILED${NC}"
    echo "Check /tmp/admin_verified.log for details"
    exit 1
fi

echo "===================================================="
echo "🏁 All quality gates PASSED"
echo "===================================================="
