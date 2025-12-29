#!/bin/bash
# Phase 0x11-b Sentinel Hardening Test Runner
# 
# Usage:
#   ./scripts/tests/0x11b_sentinel/run_tests.sh [--with-nodes]
#
# Options:
#   --with-nodes    Start BTC/ETH nodes if not running

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"

cd "$PROJECT_ROOT"

echo "========================================================================"
echo "🌟 Phase 0x11-b: Sentinel Hardening Test Suite"
echo "========================================================================"

# Parse arguments
WITH_NODES=false
while [[ $# -gt 0 ]]; do
    case $1 in
        --with-nodes)
            WITH_NODES=true
            shift
            ;;
        *)
            echo "Unknown option: $1"
            exit 1
            ;;
    esac
done

# Check for node availability
check_btc_node() {
    curl -s --user user:pass --data-binary '{"jsonrpc":"1.0","method":"getblockcount","params":[]}' \
        -H 'content-type: text/plain;' http://127.0.0.1:18443/ > /dev/null 2>&1
}

check_eth_node() {
    curl -s -X POST -H "Content-Type: application/json" \
        --data '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' \
        http://127.0.0.1:8545/ > /dev/null 2>&1
}

echo ""
echo "📡 Checking node availability..."

if check_btc_node; then
    echo "   ✅ BTC Node: Running"
    BTC_RUNNING=true
else
    echo "   ❌ BTC Node: Not running"
    BTC_RUNNING=false
fi

if check_eth_node; then
    echo "   ✅ ETH Node: Running"
    ETH_RUNNING=true
else
    echo "   ❌ ETH Node: Not running"
    ETH_RUNNING=false
fi

# Start nodes if requested
if [ "$WITH_NODES" = true ]; then
    echo ""
    echo "🐳 Starting blockchain nodes via docker-compose..."
    docker-compose up -d bitcoind anvil 2>/dev/null || true
    
    # Wait for nodes
    echo "   ⏳ Waiting for nodes to start..."
    for i in {1..30}; do
        if check_btc_node && check_eth_node; then
            echo "   ✅ Both nodes ready"
            break
        fi
        sleep 1
    done
fi

echo ""
echo "========================================================================"
echo "🧪 Running Tests"
echo "========================================================================"

# 1. Rust Unit Tests (always run)
echo ""
echo "🦀 [1/2] Rust Sentinel Unit Tests"
echo "------------------------------------------------------------------------"
cargo test --package zero_x_infinity --lib sentinel -- --nocapture 2>&1 | grep -E "(test |ok|FAILED|passed|failed)"

RUST_RESULT=$?

# 2. Python E2E Tests
echo ""
echo "🐍 [2/2] Python E2E Tests"
echo "------------------------------------------------------------------------"
uv run python3 "$SCRIPT_DIR/test_sentinel_0x11b.py"

PYTHON_RESULT=$?

# Summary
echo ""
echo "========================================================================"
echo "📊 Final Summary"
echo "========================================================================"

if [ $RUST_RESULT -eq 0 ] && [ $PYTHON_RESULT -eq 0 ]; then
    echo "   ✅ All tests passed!"
    exit 0
else
    echo "   ❌ Some tests failed"
    exit 1
fi
