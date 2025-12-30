#!/bin/bash
#
# L4 Decomposed Test Runner
# ==========================
# Runs the 5 decomposed L4 tests in sequence for bug isolation.
#
# Usage:
#   ./run_L4_decomposed.sh
#   ./run_L4_decomposed.sh --quick   # Stop at first failure
#
# Expected Results:
#   L4a: PASS (user isolation)
#   L4b: PASS (order placement)
#   L4c: PASS (taker verification)
#   L4d: FAIL (maker bug - SEC-001/002/003)
#   L4e: FAIL (data leak - SEC-004)
#

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
QUICK_MODE=false

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

# Parse args
for arg in "$@"; do
    if [ "$arg" = "--quick" ]; then
        QUICK_MODE=true
    fi
done

echo ""
echo -e "${CYAN}╔═══════════════════════════════════════════════════════════╗${NC}"
echo -e "${CYAN}║  🧪 L4 Decomposed Test Suite - Bug Isolation              ║${NC}"
echo -e "${CYAN}╠═══════════════════════════════════════════════════════════╣${NC}"
echo -e "${CYAN}║  L4a: User Isolation        (Expected: PASS)              ║${NC}"
echo -e "${CYAN}║  L4b: Order Placement       (Expected: PASS)              ║${NC}"
echo -e "${CYAN}║  L4c: Taker Verification    (Expected: PASS)              ║${NC}"
echo -e "${CYAN}║  L4d: Maker Verification    (Expected: FAIL - SEC-001/2/3)║${NC}"
echo -e "${CYAN}║  L4e: Data Isolation        (Expected: FAIL - SEC-004)    ║${NC}"
echo -e "${CYAN}╚═══════════════════════════════════════════════════════════╝${NC}"
echo ""

cd "$SCRIPT_DIR"

declare -a RESULTS
L4a_PASS=false
L4b_PASS=false
L4c_PASS=false
L4d_PASS=false
L4e_PASS=false

# L4a: User Isolation
echo -e "${YELLOW}━━━ L4a: User Isolation ━━━${NC}"
if uv run python3 L4a_user_isolation.py; then
    L4a_PASS=true
    RESULTS+=("✅ L4a User Isolation")
else
    RESULTS+=("❌ L4a User Isolation")
    if [ "$QUICK_MODE" = true ]; then
        echo -e "${RED}Quick mode: stopping at first failure${NC}"
        exit 1
    fi
fi

# L4b: Order Placement
echo ""
echo -e "${YELLOW}━━━ L4b: Order Placement ━━━${NC}"
if uv run python3 L4b_order_placement.py; then
    L4b_PASS=true
    RESULTS+=("✅ L4b Order Placement")
else
    RESULTS+=("❌ L4b Order Placement")
    if [ "$QUICK_MODE" = true ]; then exit 1; fi
fi

# L4c: Taker Verification
echo ""
echo -e "${YELLOW}━━━ L4c: Taker Verification ━━━${NC}"
if uv run python3 L4c_taker_verification.py; then
    L4c_PASS=true
    RESULTS+=("✅ L4c Taker Verification")
else
    RESULTS+=("❌ L4c Taker Verification")
    if [ "$QUICK_MODE" = true ]; then exit 1; fi
fi

# L4d: Maker Verification (EXPECTED FAIL)
echo ""
echo -e "${YELLOW}━━━ L4d: Maker Verification (BUG ISOLATION) ━━━${NC}"
if uv run python3 L4d_maker_verification.py; then
    L4d_PASS=true
    RESULTS+=("✅ L4d Maker Verification (BUG FIXED!)")
else
    RESULTS+=("⚠️  L4d Maker Verification (SEC-001/002/003 confirmed)")
fi

# L4e: Data Isolation (EXPECTED FAIL)
echo ""
echo -e "${YELLOW}━━━ L4e: Data Isolation (BUG ISOLATION) ━━━${NC}"
if uv run python3 L4e_data_isolation.py; then
    L4e_PASS=true
    RESULTS+=("✅ L4e Data Isolation (BUG FIXED!)")
else
    RESULTS+=("⚠️  L4e Data Isolation (SEC-004 confirmed)")
fi

# Summary
echo ""
echo -e "${CYAN}╔═══════════════════════════════════════════════════════════╗${NC}"
echo -e "${CYAN}║  📊 L4 DECOMPOSED TEST RESULTS                            ║${NC}"
echo -e "${CYAN}╚═══════════════════════════════════════════════════════════╝${NC}"
echo ""
for result in "${RESULTS[@]}"; do
    echo "   $result"
done
echo ""

# Diagnosis
echo -e "${YELLOW}━━━ Bug Diagnosis ━━━${NC}"

if [ "$L4a_PASS" = true ] && [ "$L4b_PASS" = true ] && [ "$L4c_PASS" = true ]; then
    echo "   ✅ Foundation: User isolation, orders, taker all work"
else
    echo "   ❌ Foundation issues detected - fix before addressing Maker bug"
fi

if [ "$L4d_PASS" = false ]; then
    echo "   ❌ SEC-001/002/003: Maker order events not propagating"
    echo "      → Fix: MatchingEngine → Sentinel event chain for Maker side"
fi

if [ "$L4e_PASS" = false ]; then
    echo "   ❌ SEC-004: /trades API returns global data"
    echo "      → Fix: Add user_id filter in trades query"
fi

if [ "$L4d_PASS" = true ] && [ "$L4e_PASS" = true ]; then
    echo ""
    echo -e "${GREEN}   🎉 ALL BUGS FIXED! L4 should now pass.${NC}"
fi

echo ""
