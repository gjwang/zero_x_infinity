#!/bin/bash
# =============================================================================
# 0x14-b Order Commands QA Test Suite - CI Entry Point
# =============================================================================
#
# 用途: 一键运行所有 Phase 0x14-b QA 测试
# 适用: CI 集成、本地验证
#
# 前置条件:
#   1. Gateway 已启动: cargo run --release -- --gateway --env dev
#   2. 或者使用 --with-gateway 参数自动启动
#
# 用法:
#   ./run_qa_ci.sh              # 假设 Gateway 已运行
#   ./run_qa_ci.sh --with-gateway  # 自动启动 Gateway
#
# =============================================================================

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"
GATEWAY_URL="${GATEWAY_URL:-http://localhost:8080}"
GATEWAY_PID=""

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo "================================================================================"
echo "🧪 QA 0x14-b: Order Commands CI Test Suite"
echo "================================================================================"
echo ""
echo "Gateway URL: $GATEWAY_URL"
echo "Project Root: $PROJECT_ROOT"
echo ""

# Parse arguments
WITH_GATEWAY=false
for arg in "$@"; do
    case $arg in
        --with-gateway)
            WITH_GATEWAY=true
            shift
            ;;
    esac
done

# Function to check if Gateway is running
check_gateway() {
    # Check if port 8080 is listening
    lsof -i :8080 > /dev/null 2>&1
}

# Function to wait for Gateway
wait_for_gateway() {
    echo -n "⏳ Waiting for Gateway..."
    for i in {1..30}; do
        if check_gateway; then
            echo -e " ${GREEN}Ready!${NC}"
            return 0
        fi
        echo -n "."
        sleep 1
    done
    echo -e " ${RED}Timeout!${NC}"
    return 1
}

# Function to start Gateway
start_gateway() {
    echo "🚀 Starting Gateway..."
    
    # Kill any existing process on port 8080
    if check_gateway; then
        echo "   Stopping existing Gateway on port 8080..."
        lsof -ti :8080 | xargs -r kill 2>/dev/null || true
        sleep 2
    fi
    
    cd "$PROJECT_ROOT"
    cargo run --release -- --gateway --env dev > /tmp/gateway_0x14b.log 2>&1 &
    GATEWAY_PID=$!
    echo "   Gateway PID: $GATEWAY_PID"
    wait_for_gateway
}

# Function to cleanup
cleanup() {
    if [ -n "$GATEWAY_PID" ]; then
        echo "🛑 Stopping Gateway (PID: $GATEWAY_PID)..."
        kill $GATEWAY_PID 2>/dev/null || true
    fi
}

# Set trap for cleanup
trap cleanup EXIT

# Start Gateway if requested
if [ "$WITH_GATEWAY" = true ]; then
    start_gateway
else
    echo "🔍 Checking Gateway..."
    if ! check_gateway; then
        echo -e "${YELLOW}⚠️  Gateway not responding at $GATEWAY_URL${NC}"
        echo ""
        echo "Options:"
        echo "  1. Start Gateway manually: cargo run --release -- --gateway --env dev"
        echo "  2. Use --with-gateway flag: $0 --with-gateway"
        echo ""
        exit 1
    fi
    echo -e "   ${GREEN}Gateway is running${NC}"
fi

echo ""

# Run the tests
echo "================================================================================
📦 Running QA Tests
================================================================================"

cd "$PROJECT_ROOT"
python3 scripts/tests/0x14b_matching/run_all_qa_tests.py
TEST_EXIT_CODE=$?

echo ""
echo "================================================================================"
if [ $TEST_EXIT_CODE -eq 0 ]; then
    echo -e "${GREEN}✅ QA 0x14-b: ALL TESTS PASSED${NC}"
else
    echo -e "${RED}❌ QA 0x14-b: TESTS FAILED (exit code: $TEST_EXIT_CODE)${NC}"
fi
echo "================================================================================"

exit $TEST_EXIT_CODE
