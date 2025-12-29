#!/bin/bash
#
# 🎯 Phase 0x11-b: Critical Path E2E Test
#
# 基于第一性原理设计的端到端测试
# 覆盖完整资金流转路径 (5 阶段)
#
# Critical Path (Longest):
#   [1] DEPOSIT      → Funding Account
#   [2] TRANSFER IN  → Spot Account  
#   [3] TRADING      → Place/Match Order
#   [4] TRANSFER OUT → Funding Account
#   [5] WITHDRAWAL   → External Address
#
# Usage:
#   ./scripts/run_0x11b_e2e.sh
#

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TEST_DIR="$SCRIPT_DIR/tests/0x11b_sentinel"

echo ""
echo "╔══════════════════════════════════════════════════════════════════════╗"
echo "║  🎯 Phase 0x11-b: Complete Money Flow E2E Test                       ║"
echo "║                                                                      ║"
echo "║  First Principles - TRUE Longest Path (5 Phases)                     ║"
echo "║                                                                      ║"
echo "║    [1] DEPOSIT      → Funding Account (DEF-002)                      ║"
echo "║        ↓                                                             ║"
echo "║    [2] TRANSFER IN  → Spot Account                                   ║"
echo "║        ↓                                                             ║"
echo "║    [3] TRADING      → Place/Match Order                              ║"
echo "║        ↓                                                             ║"
echo "║    [4] TRANSFER OUT → Funding Account                                ║"
echo "║        ↓                                                             ║"
echo "║    [5] WITHDRAWAL   → External Address                               ║"
echo "║                                                                      ║"
echo "╚══════════════════════════════════════════════════════════════════════╝"
echo ""

# Quick environment check
echo "📡 Environment Check..."

# BTC
if curl -s --user admin:admin --data-binary '{"jsonrpc":"1.0","id":"test","method":"getblockcount","params":[]}' -H 'content-type:text/plain;' http://127.0.0.1:18443/ > /dev/null 2>&1; then
    echo "   ✅ BTC Node: Connected"
else
    echo "   ❌ BTC Node: Not available"
    echo "   Run: bitcoind -regtest -daemon"
    exit 1
fi

# Gateway
if curl -s http://127.0.0.1:8080/health > /dev/null 2>&1 || curl -s http://127.0.0.1:8080/ > /dev/null 2>&1; then
    echo "   ✅ Gateway: Connected"
else
    echo "   ❌ Gateway: Not available"
    echo "   Run: cargo run -- --gateway"
    exit 1
fi

echo ""
echo "🚀 Running Critical Path E2E Test..."
echo ""

cd "$TEST_DIR"
uv run python3 e2e_critical_path.py

EXIT_CODE=$?

echo ""
if [ $EXIT_CODE -eq 0 ]; then
    echo "╔══════════════════════════════════════════════════════════════════════╗"
    echo "║  🎉 CRITICAL PATH E2E: PASSED                                        ║"
    echo "║  DEF-002 is FIXED - SegWit deposits work end-to-end                 ║"
    echo "╚══════════════════════════════════════════════════════════════════════╝"
else
    echo "╔══════════════════════════════════════════════════════════════════════╗"
    echo "║  ❌ CRITICAL PATH E2E: FAILED                                        ║"
    echo "║  DEF-002 needs investigation                                        ║"
    echo "╚══════════════════════════════════════════════════════════════════════╝"
fi

exit $EXIT_CODE
