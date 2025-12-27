#!/bin/bash
# test_settlement.sh - Settlement Persistence Verification
# =========================================================
#
# PURPOSE:
#   Verify that Settlement Persistence correctly writes data to TDengine.
#   Compares Pipeline CSV output with TDengine data.
#
# WORKFLOW:
#   1. Dump balances from TDengine to CSV
#   2. Compare Pipeline output CSV with DB dump CSV
#   3. Report results
#
# REUSABLE SCRIPTS:
#   - scripts/dump_balances.py    - Export TDengine -> CSV
#   - scripts/compare_settlement.py - Compare two CSVs
#
# USAGE:
#   ./scripts/test_settlement.sh                    # Default paths
#   ./scripts/test_settlement.sh --pipeline FILE    # Custom pipeline CSV
#
# PREREQUISITES:
#   - TDengine running (docker ps | grep tdengine)
#   - Gateway running (for balance API) or TDengine REST API accessible
#
# EXIT CODES:
#   0 = Verification passed (100% match)
#   1 = Verification failed (mismatches found)
#   2 = Setup error
# =========================================================

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

# Default paths
PIPELINE_CSV="${PROJECT_DIR}/output/t2_balances_final.csv"
DB_DUMP_CSV="/tmp/settlement_db_dump.csv"
USERS="0-100"
ASSETS="1,2"

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --pipeline) PIPELINE_CSV="$2"; shift 2 ;;
        --users) USERS="$2"; shift 2 ;;
        --assets) ASSETS="$2"; shift 2 ;;
        --help|-h)
            head -30 "$0" | tail -28
            exit 0
            ;;
        *) echo "Unknown option: $1"; exit 2 ;;
    esac
done

# Colors
if [ -t 1 ]; then
    RED='\033[0;31m'
    GREEN='\033[0;32m'
    YELLOW='\033[1;33m'
    NC='\033[0m'
else
    RED=''
    GREEN=''
    YELLOW=''
    NC=''
fi

echo "╔════════════════════════════════════════════════════════════╗"
echo "║    Settlement Persistence Verification                    ║"
echo "╚════════════════════════════════════════════════════════════╝"
echo ""

STEP=0
FAILED_STEP=""

fail_at_step() {
    local step=$1
    local msg=$2
    echo ""
    echo -e "${RED}════════════════════════════════════════════════════════════${NC}"
    echo -e "${RED}FAILED at Step ${step}: ${msg}${NC}"
    echo -e "${RED}════════════════════════════════════════════════════════════${NC}"
    exit 1
}

# ============================================================================
# Step 1: Check TDengine
# ============================================================================
STEP=1
echo "[Step $STEP] Checking TDengine..."

if ! docker ps 2>/dev/null | grep -q tdengine; then
    fail_at_step $STEP "TDengine not running. Start with: docker start tdengine"
fi
echo -e "    ${GREEN}✓${NC} TDengine running"

# ============================================================================
# Step 2: Check Pipeline output exists
# ============================================================================
STEP=2
echo ""
echo "[Step $STEP] Checking Pipeline output..."

if [ ! -f "$PIPELINE_CSV" ]; then
    fail_at_step $STEP "Pipeline CSV not found: $PIPELINE_CSV"
fi
PIPELINE_COUNT=$(wc -l < "$PIPELINE_CSV" | tr -d ' ')
echo -e "    ${GREEN}✓${NC} Found: $PIPELINE_CSV ($PIPELINE_COUNT lines)"

# ============================================================================
# Step 3: Dump balances from TDengine
# ============================================================================
STEP=3
echo ""
echo "[Step $STEP] Dumping balances from TDengine..."

if ! uv run "${SCRIPT_DIR}/dump_balances.py" \
    --output "$DB_DUMP_CSV" \
    --users "$USERS" \
    --assets "$ASSETS" \
    --method gateway; then
    fail_at_step $STEP "Failed to dump balances from TDengine"
fi

if [ ! -f "$DB_DUMP_CSV" ]; then
    fail_at_step $STEP "DB dump file not created: $DB_DUMP_CSV"
fi
DB_COUNT=$(wc -l < "$DB_DUMP_CSV" | tr -d ' ')
echo -e "    ${GREEN}✓${NC} Dumped: $DB_DUMP_CSV ($DB_COUNT lines)"

# ============================================================================
# Step 4: Compare Pipeline vs DB
# ============================================================================
STEP=4
echo ""
echo "[Step $STEP] Comparing Pipeline CSV vs TDengine CSV..."

if ! uv run "${SCRIPT_DIR}/compare_settlement.py" \
    --pipeline "$PIPELINE_CSV" \
    --db "$DB_DUMP_CSV"; then
    fail_at_step $STEP "Comparison failed - mismatches found"
fi

# ============================================================================
# Success
# ============================================================================
echo ""
echo -e "${GREEN}╔════════════════════════════════════════════════════════════╗${NC}"
echo -e "${GREEN}║  ✅ ALL 4 STEPS PASSED - SETTLEMENT VERIFICATION OK        ║${NC}"
echo -e "${GREEN}╚════════════════════════════════════════════════════════════╝${NC}"
echo ""
echo "Steps completed:"
echo "  [1] TDengine check        ✓"
echo "  [2] Pipeline CSV check    ✓"
echo "  [3] TDengine dump         ✓"
echo "  [4] Field-level compare   ✓"
exit 0
