#!/bin/bash
#
# Phase 0x11-a: Real Chain Integration - Multi-Persona QA Test Suite
#
# This script runs all test cases from the three QA agents:
#   - Agent A (激进派): Edge cases, chaos testing
#   - Agent B (保守派): Core flows, stability
#   - Agent C (安全专家): Security testing
#
# Prerequisites:
#   - bitcoind regtest running on localhost:18443
#   - anvil (optional) running on localhost:8545
#   - Gateway running on localhost:8080
#   - Sentinel service running (for real chain tests)
#
# Usage:
#   ./run_all_0x11a.sh [agent_a|agent_b|agent_c|all]
#

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../../.." && pwd)"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
export GATEWAY_URL="${GATEWAY_URL:-http://127.0.0.1:8080}"
export BTC_RPC_URL="${BTC_RPC_URL:-http://127.0.0.1:18443}"
export BTC_RPC_USER="${BTC_RPC_USER:-admin}"
export BTC_RPC_PASS="${BTC_RPC_PASS:-admin}"
export ETH_RPC_URL="${ETH_RPC_URL:-http://127.0.0.1:8545}"
export BTC_REQUIRED_CONFIRMATIONS="${BTC_REQUIRED_CONFIRMATIONS:-6}"
export ETH_REQUIRED_CONFIRMATIONS="${ETH_REQUIRED_CONFIRMATIONS:-12}"

echo "========================================================================"
echo "🧪 Phase 0x11-a: Real Chain Integration - QA Test Suite"
echo "========================================================================"
echo ""
echo "Configuration:"
echo "  GATEWAY_URL: $GATEWAY_URL"
echo "  BTC_RPC_URL: $BTC_RPC_URL"
echo "  ETH_RPC_URL: $ETH_RPC_URL"
echo ""

# Track results
declare -a RESULTS
TOTAL_PASSED=0
TOTAL_FAILED=0

run_test() {
    local agent=$1
    local test_file=$2
    local test_name=$3
    
    echo ""
    echo "------------------------------------------------------------------------"
    echo "Running: $test_name"
    echo "------------------------------------------------------------------------"
    
    if uv run python3 "$test_file"; then
        RESULTS+=("✅ $test_name")
        ((TOTAL_PASSED++))
        return 0
    else
        RESULTS+=("❌ $test_name")
        ((TOTAL_FAILED++))
        return 1
    fi
}

run_agent_a() {
    echo ""
    echo -e "${RED}========================================================================"
    echo "🔴 Agent A (激进派): Edge Case Testing"
    echo "========================================================================${NC}"
    
    run_test "A" "$SCRIPT_DIR/agent_a_edge/test_reorg_shallow.py" "TC-A01/A03: Re-org Tests" || true
    run_test "A" "$SCRIPT_DIR/agent_a_edge/test_reorg_deep.py" "TC-A02: Deep Re-org Circuit Breaker" || true
    run_test "A" "$SCRIPT_DIR/agent_a_edge/test_precision_overflow.py" "TC-A04/A06: Precision Tests" || true
}

run_agent_b() {
    echo ""
    echo -e "${GREEN}========================================================================"
    echo "🟢 Agent B (保守派): Core Flow Testing"
    echo "========================================================================${NC}"
    
    run_test "B" "$SCRIPT_DIR/agent_b_core/test_deposit_lifecycle.py" "TC-B01/B02: Deposit Lifecycle" || true
    run_test "B" "$SCRIPT_DIR/agent_b_core/test_sentinel_stability.py" "TC-B04-B07: Sentinel Stability" || true
}

run_agent_c() {
    echo ""
    echo -e "${BLUE}========================================================================"
    echo "🔒 Agent C (安全专家): Security Testing"
    echo "========================================================================${NC}"
    
    run_test "C" "$SCRIPT_DIR/agent_c_security/test_address_poisoning.py" "TC-C01/C02/C05/C06: Address Security" || true
    run_test "C" "$SCRIPT_DIR/agent_c_security/test_rpc_injection.py" "TC-C03/C04: Injection Tests" || true
}

# Parse arguments
case "${1:-all}" in
    agent_a|a)
        run_agent_a
        ;;
    agent_b|b)
        run_agent_b
        ;;
    agent_c|c)
        run_agent_c
        ;;
    all)
        run_agent_b  # Core first
        run_agent_c  # Security second
        run_agent_a  # Edge cases last (may be destructive)
        ;;
    *)
        echo "Usage: $0 [agent_a|agent_b|agent_c|all]"
        exit 1
        ;;
esac

# Final Summary
echo ""
echo "========================================================================"
echo -e "${YELLOW}📊 FINAL SUMMARY - Agent Leader (主编)${NC}"
echo "========================================================================"
echo ""

for result in "${RESULTS[@]}"; do
    echo "  $result"
done

echo ""
echo "------------------------------------------------------------------------"
echo "  Total Passed: $TOTAL_PASSED"
echo "  Total Failed: $TOTAL_FAILED"
echo "------------------------------------------------------------------------"

if [ $TOTAL_FAILED -eq 0 ]; then
    echo ""
    echo -e "${GREEN}🎉 ALL TESTS PASSED${NC}"
    echo ""
    exit 0
else
    echo ""
    echo -e "${RED}❌ SOME TESTS FAILED${NC}"
    echo ""
    exit 1
fi
