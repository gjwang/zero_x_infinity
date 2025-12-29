#!/bin/bash
#
# 🚀 Phase 0x11-b: One-Click Test Runner
#
# 一键运行所有 39 个测试用例
#
# Prerequisites:
#   - bitcoind regtest: localhost:18443
#   - anvil (optional): localhost:8545
#   - Gateway: localhost:8080
#
# Usage:
#   ./scripts/run_0x11b_oneclick.sh
#

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TESTS_DIR="$SCRIPT_DIR/tests/0x11b_sentinel"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
MAGENTA='\033[0;35m'
CYAN='\033[0;36m'
NC='\033[0m'

echo ""
echo -e "${CYAN}╔══════════════════════════════════════════════════════════════════════╗${NC}"
echo -e "${CYAN}║                                                                      ║${NC}"
echo -e "${CYAN}║   🚀 Phase 0x11-b: Sentinel Hardening - One-Click Test Suite        ║${NC}"
echo -e "${CYAN}║                                                                      ║${NC}"
echo -e "${CYAN}║   Total: 39 Test Cases                                               ║${NC}"
echo -e "${CYAN}║   Agent A (激进派): 14 Edge Case Tests                               ║${NC}"
echo -e "${CYAN}║   Agent B (保守派): 12 Core Flow Tests                               ║${NC}"
echo -e "${CYAN}║   Agent C (安全专家): 13 Security Tests                              ║${NC}"
echo -e "${CYAN}║                                                                      ║${NC}"
echo -e "${CYAN}╚══════════════════════════════════════════════════════════════════════╝${NC}"
echo ""

# Check if test directory exists
if [ ! -d "$TESTS_DIR" ]; then
    echo -e "${RED}❌ Test directory not found: $TESTS_DIR${NC}"
    exit 1
fi

# Check for run_all script
if [ -f "$TESTS_DIR/run_all_0x11b.sh" ]; then
    echo -e "${GREEN}✅ Found test runner${NC}"
else
    echo -e "${RED}❌ run_all_0x11b.sh not found${NC}"
    exit 1
fi

# Quick environment check
echo ""
echo -e "${YELLOW}📡 Environment Check...${NC}"

# BTC Node
if curl -s --user admin:admin --data-binary '{"jsonrpc":"1.0","id":"test","method":"getblockcount","params":[]}' -H 'content-type:text/plain;' http://127.0.0.1:18443/ > /dev/null 2>&1; then
    echo -e "   ${GREEN}✅ BTC Node (bitcoind regtest): Connected${NC}"
    BTC_OK=true
else
    echo -e "   ${RED}❌ BTC Node: Not available${NC}"
    echo -e "   ${YELLOW}   Start with: bitcoind -regtest -daemon${NC}"
    BTC_OK=false
fi

# ETH Node (optional)
if curl -s -X POST -H "Content-Type: application/json" --data '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' http://127.0.0.1:8545/ > /dev/null 2>&1; then
    echo -e "   ${GREEN}✅ ETH Node (anvil): Connected${NC}"
    ETH_OK=true
else
    echo -e "   ${YELLOW}⚠️  ETH Node: Not available (ETH tests will be skipped)${NC}"
    ETH_OK=false
fi

# Gateway
if curl -s http://127.0.0.1:8080/health > /dev/null 2>&1 || curl -s http://127.0.0.1:8080/ > /dev/null 2>&1; then
    echo -e "   ${GREEN}✅ Gateway: Connected${NC}"
    GW_OK=true
else
    echo -e "   ${RED}❌ Gateway: Not available${NC}"
    echo -e "   ${YELLOW}   Start with: cargo run -- --gateway${NC}"
    GW_OK=false
fi

echo ""

# Check if we can proceed
if [ "$BTC_OK" = false ] || [ "$GW_OK" = false ]; then
    echo -e "${RED}╔══════════════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${RED}║  ❌ Cannot proceed: Required services not running                    ║${NC}"
    echo -e "${RED}╚══════════════════════════════════════════════════════════════════════╝${NC}"
    echo ""
    echo -e "${YELLOW}Please start the required services:${NC}"
    echo -e "  1. ${CYAN}bitcoind -regtest -daemon${NC}"
    echo -e "  2. ${CYAN}cargo run -- --gateway${NC}"
    echo -e "  3. ${CYAN}anvil${NC} (optional, for ETH tests)"
    echo ""
    exit 1
fi

# Run all tests
echo -e "${GREEN}╔══════════════════════════════════════════════════════════════════════╗${NC}"
echo -e "${GREEN}║  🧪 Running All 0x11-b Tests...                                      ║${NC}"
echo -e "${GREEN}╚══════════════════════════════════════════════════════════════════════╝${NC}"
echo ""

cd "$TESTS_DIR"
bash run_all_0x11b.sh

EXIT_CODE=$?

echo ""
if [ $EXIT_CODE -eq 0 ]; then
    echo -e "${GREEN}╔══════════════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${GREEN}║  🎉 ALL TESTS PASSED!                                                 ║${NC}"
    echo -e "${GREEN}║  Phase 0x11-b: Sentinel Hardening - VERIFIED                         ║${NC}"
    echo -e "${GREEN}╚══════════════════════════════════════════════════════════════════════╝${NC}"
else
    echo -e "${YELLOW}╔══════════════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${YELLOW}║  ⚠️  Some tests failed - check results above                         ║${NC}"
    echo -e "${YELLOW}╚══════════════════════════════════════════════════════════════════════╝${NC}"
fi

exit $EXIT_CODE
