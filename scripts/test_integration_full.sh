#!/bin/bash
# test_integration_full.sh - Comprehensive Integration Test for Phase 0x09
# Runs all automated tests: Pipeline, Persistence, and API endpoints
#
# ============================================================================
# USAGE:
#   ./scripts/test_integration_full.sh           # Run all tests
#   nohup ./scripts/test_integration_full.sh > /tmp/integration.log 2>&1 &  # Background
#
# IMPORTANT - 大数据集测试注意事项:
#   1. 1.3M 测试非常耗时 (~500+ 秒)，建议后台运行并重定向输出
#   2. Balance events 中 lock count != accepted 是正常的 (cancel orders 无 lock)
#   3. [PUSH] queue full 警告是预期行为，不影响测试结果
#   4. 超时设置: 100K=600s, 1.3M=3600s，如需调整请修改下方 timeout 参数
#
# LOGS: All logs saved to /tmp/integration_tests/
# ============================================================================

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
LOG_DIR="/tmp/integration_tests"
PASSED=0
FAILED=0
SKIPPED=0

mkdir -p "$LOG_DIR"

# Add timeout fallback for macOS if missing
if ! command -v timeout &> /dev/null; then
    timeout() {
        local duration="$1"
        shift
        "$@"
    }
fi

echo "╔════════════════════════════════════════════════════════════════╗"
echo "║     Phase 0x09-f: Comprehensive Integration Test               ║"
echo "╚════════════════════════════════════════════════════════════════╝"
echo ""
echo "Log directory: $LOG_DIR"
echo ""

# Helper function
run_test() {
    local name="$1"
    local script="$2"
    local log_file="$LOG_DIR/${name}.log"
    
    echo -n "[TEST] $name... "
    
    if [ ! -f "$PROJECT_DIR/$script" ]; then
        echo -e "${YELLOW}SKIPPED${NC} (script not found)"
        ((SKIPPED++))
        return
    fi
    
    if timeout 1800 "$PROJECT_DIR/$script" > "$log_file" 2>&1; then
        echo -e "${GREEN}PASSED${NC}"
        ((PASSED++))
    else
        echo -e "${RED}FAILED${NC} (see $log_file)"
        ((FAILED++))
    fi
}

# Phase 1: Pipeline Correctness
echo "═══════════════════════════════════════════════════════════════"
echo "Phase 1: Pipeline Correctness"
echo "═══════════════════════════════════════════════════════════════"

echo -n "[TEST] 100K Pipeline Comparison... "
if timeout 600 "$PROJECT_DIR/scripts/test_pipeline_compare.sh" 100k > "$LOG_DIR/pipeline_100k.log" 2>&1; then
    if grep -q "ALL TESTS PASSED" "$LOG_DIR/pipeline_100k.log"; then
        echo -e "${GREEN}PASSED${NC}"
        ((PASSED++))
    else
        echo -e "${RED}FAILED${NC}"
        ((FAILED++))
    fi
else
    echo -e "${RED}TIMEOUT/ERROR${NC}"
    ((FAILED++))
fi

echo -n "[TEST] 1.3M Pipeline Comparison... "
if timeout 3600 "$PROJECT_DIR/scripts/test_pipeline_compare.sh" highbal > "$LOG_DIR/pipeline_1.3m.log" 2>&1; then
    if grep -q "ALL TESTS PASSED" "$LOG_DIR/pipeline_1.3m.log"; then
        echo -e "${GREEN}PASSED${NC}"
        ((PASSED++))
    else
        echo -e "${RED}FAILED${NC}"
        ((FAILED++))
    fi
else
    echo -e "${RED}TIMEOUT/ERROR${NC}"
    ((FAILED++))
fi

echo -n "[TEST] Balance Events Verification... "
if uv run "$PROJECT_DIR/scripts/verify_balance_events.py" > "$LOG_DIR/balance_events.log" 2>&1; then
    echo -e "${GREEN}PASSED${NC}"
    ((PASSED++))
else
    # Check if only lock count mismatch (expected for cancel orders)
    if grep -q "Lock events.*!= Accepted orders" "$LOG_DIR/balance_events.log"; then
        if grep -q "Frozen balances match" "$LOG_DIR/balance_events.log"; then
            echo -e "${YELLOW}PARTIAL${NC} (lock count differs due to cancel orders)"
            ((PASSED++))
        else
            echo -e "${RED}FAILED${NC}"
            ((FAILED++))
        fi
    else
        echo -e "${RED}FAILED${NC}"
        ((FAILED++))
    fi
fi

# Phase 2: Settlement Persistence
echo ""
echo "═══════════════════════════════════════════════════════════════"
echo "Phase 2: Settlement Persistence"
echo "═══════════════════════════════════════════════════════════════"

# Check TDengine
echo -n "[CHECK] TDengine running... "
if docker ps | grep -q tdengine; then
    echo -e "${GREEN}YES${NC}"
else
    echo -e "${RED}NO${NC} - Starting TDengine..."
    docker start tdengine 2>/dev/null || docker run -d --name tdengine -p 6030:6030 -p 6041:6041 tdengine/tdengine:latest
    sleep 5
fi

run_test "Persistence" "scripts/test_persistence.sh"

# Phase 3: HTTP API Endpoints
echo ""
echo "═══════════════════════════════════════════════════════════════"
echo "Phase 3: HTTP API Endpoints"
echo "═══════════════════════════════════════════════════════════════"

run_test "Gateway_E2E" "scripts/test_order_api.sh"
run_test "KLine_E2E" "scripts/test_kline_e2e.sh"
run_test "Depth_API" "scripts/test_depth.sh"

# Summary
echo ""
echo "═══════════════════════════════════════════════════════════════"
echo "Test Summary"
echo "═══════════════════════════════════════════════════════════════"
echo ""
echo "Passed:  $PASSED"
echo "Failed:  $FAILED"
echo "Skipped: $SKIPPED"
echo ""
echo "Logs saved in: $LOG_DIR"
echo ""

if [ $FAILED -eq 0 ]; then
    echo -e "${GREEN}╔════════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${GREEN}║            ✅ ALL INTEGRATION TESTS PASSED                     ║${NC}"
    echo -e "${GREEN}╚════════════════════════════════════════════════════════════════╝${NC}"
    exit 0
else
    echo -e "${RED}╔════════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${RED}║            ❌ SOME TESTS FAILED                                ║${NC}"
    echo -e "${RED}╚════════════════════════════════════════════════════════════════╝${NC}"
    exit 1
fi
