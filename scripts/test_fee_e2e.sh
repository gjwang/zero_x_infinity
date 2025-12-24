#!/bin/bash
# test_fee_e2e.sh - Trade Fee E2E Verification Test
# ===================================================================
#
# PURPOSE:
#   Verify trade fee system end-to-end by checking TDengine directly:
#   1. Check TDengine is running
#   2. Verify trades table has data
#   3. Verify trades have fee/role columns
#   4. Verify balance_events has fee_amount > 0
#   5. Verify asset conservation
#
# PREREQUISITE: Run test_gateway_e2e_full.sh first to inject data
#
# USAGE:
#   ./scripts/test_gateway_e2e_full.sh quick  # Inject data first
#   ./scripts/test_fee_e2e.sh                 # Then run this
#
# ===================================================================

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

cd "$PROJECT_DIR"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

STEP=0
PASSED=0
FAILED=0
SKIPPED=0

fail_at_step() {
    echo ""
    echo -e "${RED}════════════════════════════════════════════════════════════${NC}"
    echo -e "${RED}FAILED at Step ${STEP}: $1${NC}"
    echo -e "${RED}════════════════════════════════════════════════════════════${NC}"
    exit 1
}

echo "╔════════════════════════════════════════════════════════════╗"
echo "║    Trade Fee E2E Verification Test                        ║"
echo "╚════════════════════════════════════════════════════════════╝"
echo ""

# ============================================================================
# Step 1: Check TDengine is running (REQUIRED)
# ============================================================================
STEP=1
echo "[Step $STEP] Checking TDengine..."

if ! docker ps | grep -q tdengine; then
    fail_at_step "TDengine not running. Start with: docker start tdengine"
fi
echo -e "    ${GREEN}✓${NC} TDengine running"
((PASSED++))

# ============================================================================
# Step 2: Check trades exist in TDengine (REQUIRED)
# ============================================================================
STEP=2
echo ""
echo "[Step $STEP] Checking trades in TDengine..."

TRADES_COUNT_RAW=$(docker exec tdengine taos -s "SELECT COUNT(*) FROM trading.trades" 2>&1)
if echo "$TRADES_COUNT_RAW" | grep -q "not exist\|error"; then
    fail_at_step "trading.trades table not found. Run test_gateway_e2e_full.sh first."
fi

TRADES_COUNT=$(echo "$TRADES_COUNT_RAW" | grep -E "^\s+[0-9]+" | awk '{print $1}' | head -1)
if [ -z "$TRADES_COUNT" ] || [ "$TRADES_COUNT" -eq 0 ]; then
    fail_at_step "No trades found. Run test_gateway_e2e_full.sh first."
fi
echo -e "    ${GREEN}✓${NC} Found $TRADES_COUNT trades"
((PASSED++))

# ============================================================================
# Step 3: Verify trades table has fee/role columns (REQUIRED)
# ============================================================================
STEP=3
echo ""
echo "[Step $STEP] Checking trades table schema (fee, role columns)..."

SAMPLE_TRADE=$(docker exec tdengine taos -s "SELECT fee, role FROM trading.trades LIMIT 1" 2>&1)
if echo "$SAMPLE_TRADE" | grep -q "Invalid column\|not exist"; then
    fail_at_step "trades table missing fee or role column"
fi
echo -e "    ${GREEN}✓${NC} trades table has fee and role columns"
((PASSED++))

# ============================================================================
# Step 4: Check balance_events for fee_amount > 0 (REQUIRED when table exists)
# ============================================================================
STEP=4
echo ""
echo "[Step $STEP] Checking balance_events for fee_amount..."

FEE_EVENTS_RAW=$(docker exec tdengine taos -s "SELECT COUNT(*) FROM trading.balance_events WHERE fee_amount > 0" 2>&1)
if echo "$FEE_EVENTS_RAW" | grep -q "not exist"; then
    echo -e "    ${YELLOW}○${NC} skipped: balance_events table not found"
    ((SKIPPED++))
else
    FEE_COUNT=$(echo "$FEE_EVENTS_RAW" | grep -E "^\s+[0-9]+" | awk '{print $1}' | head -1)
    if [ -n "$FEE_COUNT" ] && [ "$FEE_COUNT" -gt 0 ]; then
        echo -e "    ${GREEN}✓${NC} Found $FEE_COUNT balance_events with fee_amount > 0"
        ((PASSED++))
    else
        echo -e "    ${RED}✗${NC} No balance_events with fee_amount > 0"
        ((FAILED++))
    fi
fi

# ============================================================================
# Step 5: Verify asset conservation (Σ delta = 0 for each asset)
# ============================================================================
STEP=5
echo ""
echo "[Step $STEP] Checking asset conservation..."

CONSERVATION_RAW=$(docker exec tdengine taos -s "SELECT asset_id, SUM(delta) as total FROM trading.balance_events GROUP BY asset_id" 2>&1)
if echo "$CONSERVATION_RAW" | grep -q "not exist"; then
    echo -e "    ${YELLOW}○${NC} skipped: balance_events table not found"
    ((SKIPPED++))
else
    # Check each asset
    HAS_DATA=false
    while read line; do
        if [ -n "$line" ]; then
            HAS_DATA=true
            asset_id=$(echo "$line" | awk '{print $1}')
            total=$(echo "$line" | awk '{print $2}')
            if [ "$total" == "0" ]; then
                echo -e "    ${GREEN}✓${NC} Asset $asset_id: Σ delta = 0 (conserved)"
            else
                echo -e "    ${RED}✗${NC} Asset $asset_id: Σ delta = $total (NOT conserved!)"
                ((FAILED++))
            fi
        fi
    done <<< "$(echo "$CONSERVATION_RAW" | grep -E "^\s+[0-9]")"
    
    if [ "$HAS_DATA" = true ] && [ "$FAILED" -eq 0 ]; then
        ((PASSED++))
    fi
fi

# ============================================================================
# Summary (like cargo test output)
# ============================================================================
echo ""
echo "════════════════════════════════════════════════════════════"
echo -e "test result: ${PASSED} passed; ${FAILED} failed; ${SKIPPED} skipped"
echo "════════════════════════════════════════════════════════════"

if [ "$FAILED" -gt 0 ]; then
    echo ""
    echo -e "${RED}╔════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${RED}║  ❌ FEE E2E TEST FAILED                                    ║${NC}"
    echo -e "${RED}╚════════════════════════════════════════════════════════════╝${NC}"
    exit 1
elif [ "$SKIPPED" -gt 0 ]; then
    echo ""
    echo -e "${YELLOW}╔════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${YELLOW}║  ⚠️  FEE E2E TEST INCOMPLETE (some tests skipped)          ║${NC}"
    echo -e "${YELLOW}╚════════════════════════════════════════════════════════════╝${NC}"
    exit 0
else
    echo ""
    echo -e "${GREEN}╔════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${GREEN}║  ✅ FEE E2E TEST PASSED                                    ║${NC}"
    echo -e "${GREEN}╚════════════════════════════════════════════════════════════╝${NC}"
    exit 0
fi
