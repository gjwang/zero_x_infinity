#!/bin/bash
# test_settlement_e2e.sh - Complete End-to-End Settlement Test
# ==============================================================
#
# This script runs a COMPLETE settlement test from scratch:
# 1. Clear TDengine database
# 2. Run Pipeline MT (multi-thread) to generate and persist data
# 3. Run settlement comparison (Pipeline CSV vs TDengine)
#
# USAGE:
#   ./scripts/test_settlement_e2e.sh
#
# ==============================================================

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

cd "$PROJECT_DIR"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo "╔════════════════════════════════════════════════════════════╗"
echo "║    Settlement E2E Test - From Scratch                     ║"
echo "╚════════════════════════════════════════════════════════════╝"
echo ""

# ============================================================================
# Step 1: Clear TDengine
# ============================================================================
echo "[Step 1] Clearing TDengine database..."

if ! docker ps | grep -q tdengine; then
    echo -e "${RED}ERROR: TDengine not running. Start with: docker start tdengine${NC}"
    exit 2
fi

docker exec tdengine taos -s "DROP DATABASE IF EXISTS exchange" 2>&1 | grep -v "^taos>" || true
sleep 2
echo -e "    ${GREEN}✓${NC} TDengine exchange database cleared"

# ============================================================================
# Step 2: Run Pipeline MT (will persist to TDengine)
# ============================================================================
echo ""
echo "[Step 2] Running Pipeline MT with persistence..."
echo "    This will generate trades and persist to TDengine"

# Build if needed
if [ ! -f target/release/zero_x_infinity ]; then
    echo "    Building release..."
    cargo build --release --quiet
fi

# Run pipeline MT mode (default uses fixtures/orders.csv)
# Note: Pipeline MT will persist to TDengine if TDengine is running
./target/release/zero_x_infinity --pipeline-mt 2>&1 | tail -20

echo -e "    ${GREEN}✓${NC} Pipeline MT completed"

# Check output files
if [ ! -f output/t2_balances_final.csv ]; then
    echo -e "${RED}ERROR: Pipeline did not generate output/t2_balances_final.csv${NC}"
    exit 1
fi
BALANCE_COUNT=$(wc -l < output/t2_balances_final.csv | tr -d ' ')
echo -e "    ${GREEN}✓${NC} Generated $BALANCE_COUNT balance records"

# ============================================================================
# Step 3: Run Settlement Comparison
# ============================================================================
echo ""
echo "[Step 3] Running settlement comparison..."

./scripts/test_settlement.sh

echo ""
echo -e "${GREEN}╔════════════════════════════════════════════════════════════╗${NC}"
echo -e "${GREEN}║    ✅ E2E SETTLEMENT TEST COMPLETE                         ║${NC}"
echo -e "${GREEN}╚════════════════════════════════════════════════════════════╝${NC}"
