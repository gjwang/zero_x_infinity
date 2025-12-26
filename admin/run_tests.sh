#!/bin/bash
set -e

# Admin Tests Runner - One-click testing with auto environment setup
# Usage: ./run_tests.sh [pytest args]

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

echo "================================"
echo "🧪 Admin Tests Runner"
echo "================================"
echo ""

# Step 1: Check venv
if [ ! -d "venv" ]; then
    echo -e "${YELLOW}⚠️  venv not found, creating...${NC}"
    python3 -m venv venv
    echo -e "${GREEN}✓${NC} venv created"
fi

# Step 2: Activate venv
echo -e "${GREEN}✓${NC} Activating venv..."
source venv/bin/activate

# Step 3: Check dependencies
if [ ! -f "venv/.deps_installed" ]; then
    echo -e "${YELLOW}⚠️  Installing dependencies...${NC}"
    pip install -q -r requirements.txt
    touch venv/.deps_installed
    echo -e "${GREEN}✓${NC} Dependencies installed"
fi

# Step 4: Load environment variables (REQUIRED!)
echo -e "${GREEN}✓${NC} Loading environment variables..."
source ../scripts/lib/db_env.sh

# Step 5: Verify DATABASE_URL_ASYNC is set
if [ -z "$DATABASE_URL_ASYNC" ]; then
    echo -e "${RED}✗${NC} ERROR: DATABASE_URL_ASYNC not set"
    echo "Please ensure scripts/lib/db_env.sh is working"
    exit 1
fi
echo -e "${GREEN}✓${NC} DATABASE_URL_ASYNC = $DATABASE_URL_ASYNC"

# Step 6: Run tests
echo ""
echo "================================"
echo "🧪 Running pytest..."
echo "================================"
pytest tests/ "${@}"

# Capture exit code
EXIT_CODE=$?

echo ""
if [ $EXIT_CODE -eq 0 ]; then
    echo -e "${GREEN}✅ All tests passed!${NC}"
else
    echo -e "${RED}❌ Some tests failed${NC}"
fi

exit $EXIT_CODE
