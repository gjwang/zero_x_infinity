#!/bin/bash
#
# Phase 0x11-b: Sentinel Hardening - Multi-Persona QA Test Suite
#
# This script runs all 39 test cases from the three QA agents:
#   - Agent A (激进派): Edge cases, chaos testing (14 tests)
#   - Agent B (保守派): Core flows, stability (12 tests)
#   - Agent C (安全专家): Security testing (13 tests)
#
# Prerequisites:
#   - bitcoind regtest running on localhost:18443
#   - anvil (optional) running on localhost:8545
#   - Gateway running on localhost:8080
#   - Sentinel service running
#
# Usage:
#   ./run_all_0x11b.sh [agent_a|agent_b|agent_c|all] [--p0-only]
#

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../../.." && pwd)"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
MAGENTA='\033[0;35m'
NC='\033[0m' # No Color

# Configuration
export GATEWAY_URL="${GATEWAY_URL:-http://127.0.0.1:8080}"
export BTC_RPC_URL="${BTC_RPC_URL:-http://127.0.0.1:18443}"
export BTC_RPC_USER="${BTC_RPC_USER:-admin}"
export BTC_RPC_PASS="${BTC_RPC_PASS:-admin}"
export BTC_WALLET="${BTC_WALLET:-sentinel_test}"
export ETH_RPC_URL="${ETH_RPC_URL:-http://127.0.0.1:8545}"
export BTC_REQUIRED_CONFIRMATIONS="${BTC_REQUIRED_CONFIRMATIONS:-6}"
export ETH_REQUIRED_CONFIRMATIONS="${ETH_REQUIRED_CONFIRMATIONS:-12}"
export INTERNAL_SECRET="${INTERNAL_SECRET:-dev-secret}"

echo "========================================================================"
echo "🧪 Phase 0x11-b: Sentinel Hardening - QA Test Suite"
echo "========================================================================"
echo ""
echo "Configuration:"
echo "  GATEWAY_URL: $GATEWAY_URL"
echo "  BTC_RPC_URL: $BTC_RPC_URL"
echo "  ETH_RPC_URL: $ETH_RPC_URL"
echo "  BTC_REQUIRED_CONFIRMATIONS: $BTC_REQUIRED_CONFIRMATIONS"
echo ""

# Track results
declare -a RESULTS
declare -a P0_RESULTS
TOTAL_PASSED=0
TOTAL_FAILED=0
P0_PASSED=0
P0_FAILED=0

run_test() {
    local agent=$1
    local test_file=$2
    local test_name=$3
    local is_p0=$4
    
    echo ""
    echo "------------------------------------------------------------------------"
    echo "Running: $test_name"
    echo "------------------------------------------------------------------------"
    
    if uv run python3 "$test_file"; then
        RESULTS+=("✅ $test_name")
        ((TOTAL_PASSED++))
        if [ "$is_p0" = "true" ]; then
            P0_RESULTS+=("✅ $test_name")
            ((P0_PASSED++))
        fi
        return 0
    else
        RESULTS+=("❌ $test_name")
        ((TOTAL_FAILED++))
        if [ "$is_p0" = "true" ]; then
            P0_RESULTS+=("❌ $test_name")
            ((P0_FAILED++))
        fi
        return 1
    fi
}

run_agent_a() {
    echo ""
    echo -e "${RED}========================================================================"
    echo "🔴 Agent A (激进派): Edge Case & Chaos Testing"
    echo "========================================================================${NC}"
    
    run_test "A" "$SCRIPT_DIR/agent_a_edge/test_segwit_edge_cases.py" "Agent A: SegWit Edge Cases (TC-A01,A03,A09-A13)" "true" || true
    run_test "A" "$SCRIPT_DIR/agent_a_edge/test_eth_edge_cases.py" "Agent A: ETH Edge Cases (TC-A04-A08,A14-A15)" "true" || true
}

run_agent_b() {
    echo ""
    echo -e "${GREEN}========================================================================"
    echo "🟢 Agent B (保守派): Core Flow & Stability Testing"
    echo "========================================================================${NC}"
    
    run_test "B" "$SCRIPT_DIR/agent_b_core/test_segwit_core.py" "Agent B: SegWit Core (TC-B01★,B02,B03,B11,B14)" "true" || true
    run_test "B" "$SCRIPT_DIR/agent_b_core/test_eth_core.py" "Agent B: ETH Core (TC-B04-B08,B12)" "true" || true
}

run_agent_c() {
    echo ""
    echo -e "${BLUE}========================================================================"
    echo "🔒 Agent C (安全专家): Security Testing"
    echo "========================================================================${NC}"
    
    run_test "C" "$SCRIPT_DIR/agent_c_security/test_btc_security.py" "Agent C: BTC Security (TC-C01-C03,C11,C13,C15)" "true" || true
    run_test "C" "$SCRIPT_DIR/agent_c_security/test_eth_security.py" "Agent C: ETH Security (TC-C04-C10)" "true" || true
}

# Parse arguments
P0_ONLY=false
TARGET="all"

for arg in "$@"; do
    case "$arg" in
        --p0-only)
            P0_ONLY=true
            ;;
        agent_a|a)
            TARGET="agent_a"
            ;;
        agent_b|b)
            TARGET="agent_b"
            ;;
        agent_c|c)
            TARGET="agent_c"
            ;;
        all)
            TARGET="all"
            ;;
    esac
done

# Run tests based on target
case "$TARGET" in
    agent_a)
        run_agent_a
        ;;
    agent_b)
        run_agent_b
        ;;
    agent_c)
        run_agent_c
        ;;
    all)
        run_agent_b  # Core first (includes DEF-002 verification)
        run_agent_c  # Security second
        run_agent_a  # Edge cases last (may be destructive)
        ;;
esac

# Final Summary
echo ""
echo "========================================================================"
echo -e "${MAGENTA}📊 FINAL SUMMARY - Agent Leader (主编/D节点)${NC}"
echo "========================================================================"
echo ""

echo "All Test Results:"
for result in "${RESULTS[@]}"; do
    echo "  $result"
done

echo ""
echo "------------------------------------------------------------------------"
echo "  Total Passed: $TOTAL_PASSED"
echo "  Total Failed: $TOTAL_FAILED"
echo "------------------------------------------------------------------------"

# P0 Summary
if [ ${#P0_RESULTS[@]} -gt 0 ]; then
    echo ""
    echo -e "${YELLOW}P0 Critical Tests:${NC}"
    for result in "${P0_RESULTS[@]}"; do
        echo "  $result"
    done
    echo ""
    echo "  P0 Passed: $P0_PASSED"
    echo "  P0 Failed: $P0_FAILED"
fi

# DEF-002 Status
echo ""
echo "========================================================================"
echo "DEF-002 (SegWit P2WPKH Detection) Status:"
echo "========================================================================"

# Check if TC-B01 passed (contains "SegWit Core" which includes DEF-002 test)
if [[ "${RESULTS[*]}" == *"✅ Agent B: SegWit Core"* ]]; then
    echo -e "  ${GREEN}✅ DEF-002 IS FIXED - SegWit deposits are correctly detected${NC}"
else
    echo -e "  ${RED}❌ DEF-002 MAY NOT BE FIXED - Check TC-B01 results${NC}"
fi

echo ""

# Exit code
if [ $TOTAL_FAILED -eq 0 ]; then
    echo -e "${GREEN}🎉 ALL TESTS PASSED - Phase 0x11-b Verification Complete${NC}"
    echo ""
    exit 0
elif [ $P0_FAILED -eq 0 ]; then
    echo -e "${YELLOW}⚠️  PARTIAL PASS - P0 tests passed, some P1/P2 failures${NC}"
    echo ""
    exit 0
else
    echo -e "${RED}❌ P0 CRITICAL TESTS FAILED - Do NOT proceed to release${NC}"
    echo ""
    exit 1
fi
