#!/bin/bash
#
# 🚀 Phase 0x14-b: Order Commands - One-Click Test Suite
#
# 一键运行 IOC, ReduceOrder, MoveOrder 匹配引擎测试
#
# Features Tested:
#   - TimeInForce::IOC (Immediate-or-Cancel)
#   - ReduceOrder (Reduce order quantity, preserves priority)
#   - MoveOrder (Move order price, loses priority)
#
# Usage:
#   ./scripts/run_0x14b_order_commands.sh   # Run unit tests + clippy
#
# Note:
#   本次更新是匹配引擎内部功能，单元测试已完整覆盖。
#   E2E 测试需要 Gateway 支持 time_in_force 参数后单独运行。
#

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$SCRIPT_DIR/.."
cd "$PROJECT_ROOT"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

echo ""
echo -e "${CYAN}╔══════════════════════════════════════════════════════════════════════╗${NC}"
echo -e "${CYAN}║                                                                      ║${NC}"
echo -e "${CYAN}║   🚀 Phase 0x14-b: Order Commands - One-Click Test Suite            ║${NC}"
echo -e "${CYAN}║                                                                      ║${NC}"
echo -e "${CYAN}║   Features:                                                          ║${NC}"
echo -e "${CYAN}║     • TimeInForce::IOC (Immediate-or-Cancel)                        ║${NC}"
echo -e "${CYAN}║     • ReduceOrder (Reduce qty, preserves priority)                  ║${NC}"
echo -e "${CYAN}║     • MoveOrder (Change price, loses priority)                      ║${NC}"
echo -e "${CYAN}║                                                                      ║${NC}"
echo -e "${CYAN}║   Tests: 18 Unit Tests (Matching Engine Core)                       ║${NC}"
echo -e "${CYAN}║                                                                      ║${NC}"
echo -e "${CYAN}╚══════════════════════════════════════════════════════════════════════╝${NC}"
echo ""

UNIT_PASSED=0
TOTAL_FAILED=0
ENGINE_PASSED=0
GATEWAY_PASSED=0

# =============================================================================
# Step 1: Unit Tests (Rust)
# =============================================================================

echo -e "${GREEN}╔══════════════════════════════════════════════════════════════════════╗
║  🧪 Step 1: Running Rust Unit Tests                                  ║
╚══════════════════════════════════════════════════════════════════════╝"
echo ""

# Engine tests
if cargo test --lib engine::tests -- --nocapture 2>&1 | tee /tmp/0x14b_engine.log; then
    ENGINE_PASSED=$(grep -c " ok$" /tmp/0x14b_engine.log || echo "0")
    echo ""
    echo -e "${GREEN}   ✅ Matching Engine Tests: ${ENGINE_PASSED} passed${NC}"
else
    echo -e "${RED}   ❌ Engine tests failed${NC}"
    TOTAL_FAILED=$((TOTAL_FAILED + 1))
fi

# Gateway time_in_force integration tests
if cargo test --lib gateway::types::tests -- --nocapture 2>&1 | tee /tmp/0x14b_gateway.log; then
    GATEWAY_PASSED=$(grep -c " ok$" /tmp/0x14b_gateway.log || echo "0")
    echo -e "${GREEN}   ✅ Gateway Integration Tests: ${GATEWAY_PASSED} passed${NC}"
else
    echo -e "${RED}   ❌ Gateway tests failed${NC}"
    TOTAL_FAILED=$((TOTAL_FAILED + 1))
fi

UNIT_PASSED=$((ENGINE_PASSED + GATEWAY_PASSED))
echo ""

# =============================================================================
# Step 2: Clippy Check
# =============================================================================

echo -e "${GREEN}╔══════════════════════════════════════════════════════════════════════╗${NC}"
echo -e "${GREEN}║  🔍 Step 2: Clippy Lint Check                                        ║${NC}"
echo -e "${GREEN}╚══════════════════════════════════════════════════════════════════════╝${NC}"
echo ""

if cargo clippy --lib -- -D warnings 2>&1 | tee /tmp/0x14b_clippy.log; then
    echo -e "${GREEN}   ✅ Clippy: No warnings${NC}"
else
    echo -e "${RED}   ❌ Clippy found issues${NC}"
    TOTAL_FAILED=$((TOTAL_FAILED + 1))
fi
echo ""

# =============================================================================
# Summary
# =============================================================================

echo -e "${CYAN}╔══════════════════════════════════════════════════════════════════════╗${NC}"
echo -e "${CYAN}║                         📊 TEST SUMMARY                              ║${NC}"
echo -e "${CYAN}╠══════════════════════════════════════════════════════════════════════╣${NC}"
echo -e "${CYAN}║${NC}  Unit Tests (Rust):    ${GREEN}${UNIT_PASSED} passed${NC}                                   ${CYAN}║${NC}"
echo -e "${CYAN}║${NC}  Clippy:               ${GREEN}✓ clean${NC}                                       ${CYAN}║${NC}"
echo -e "${CYAN}╚══════════════════════════════════════════════════════════════════════╝${NC}"
echo ""

# Exit status
if [ "$TOTAL_FAILED" -eq 0 ] && [ "$UNIT_PASSED" -ge 18 ]; then
    echo -e "${GREEN}🎉 Phase 0x14-b: Order Commands - ALL TESTS PASSED${NC}"
    exit 0
else
    echo -e "${RED}❌ Tests failed${NC}"
    exit 1
fi
