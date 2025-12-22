#!/bin/bash
# test_ci.sh - CI/CD Optimized Integration Test Suite
# Designed for automated CI pipelines (GitHub Actions, GitLab CI, Jenkins, etc.)
#
# ============================================================================
# USAGE:
#   ./scripts/test_ci.sh              # Run all tests (default)
#   ./scripts/test_ci.sh --quick      # Skip 1.3M dataset (faster CI)
#   ./scripts/test_ci.sh --full       # Include all tests including 1.3M
#
# EXIT CODES:
#   0 = All tests passed
#   1 = Some tests failed
#   2 = Setup/dependency error
#
# CI INTEGRATION:
#   - Outputs test results in TAP format for easy parsing
#   - Generates JUnit-compatible XML report (for CI dashboards)
#   - Non-interactive, no color output when not TTY
#
# ENVIRONMENT VARIABLES:
#   CI=true                  - Detect CI environment (auto-set by most CI systems)
#   SKIP_LARGE_DATASET=1     - Skip 1.3M dataset test
#   TDENGINE_HOST=localhost  - TDengine host (default: localhost)
#   TEST_TIMEOUT=600         - Default timeout for each test (seconds)
# ============================================================================

set -o pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
LOG_DIR="${CI_LOG_DIR:-/tmp/integration_tests}"
REPORT_DIR="${PROJECT_DIR}/test-reports"

# Counters
TOTAL=0
PASSED=0
FAILED=0
SKIPPED=0
START_TIME=$(date +%s)

# CI Detection
if [ -t 1 ] && [ -z "$CI" ]; then
    # Interactive terminal with colors
    RED='\033[0;31m'
    GREEN='\033[0;32m'
    YELLOW='\033[1;33m'
    BLUE='\033[0;34m'
    NC='\033[0m'
else
    # CI or non-interactive: no colors
    RED=''
    GREEN=''
    YELLOW=''
    BLUE=''
    NC=''
fi

# Parse arguments
QUICK_MODE=false
for arg in "$@"; do
    case $arg in
        --quick) QUICK_MODE=true ;;
        --full) QUICK_MODE=false ;;
        --help|-h)
            head -30 "$0" | tail -28
            exit 0
            ;;
    esac
done

# Also respect environment variable
if [ "$SKIP_LARGE_DATASET" = "1" ]; then
    QUICK_MODE=true
fi

mkdir -p "$LOG_DIR" "$REPORT_DIR"

# ============================================================================
# Helper Functions
# ============================================================================

log_test_start() {
    local name="$1"
    echo -n "[TEST] $name... "
    ((TOTAL++))
}

log_test_pass() {
    echo -e "${GREEN}PASS${NC}"
    ((PASSED++))
}

log_test_fail() {
    local msg="${1:-}"
    echo -e "${RED}FAIL${NC} $msg"
    ((FAILED++))
}

log_test_skip() {
    local msg="${1:-}"
    echo -e "${YELLOW}SKIP${NC} $msg"
    ((SKIPPED++))
}

# Run a test with timeout and logging
run_test() {
    local name="$1"
    local script="$2"
    local timeout_sec="${3:-${TEST_TIMEOUT:-600}}"
    local log_file="$LOG_DIR/${name}.log"
    
    log_test_start "$name"
    
    if [ ! -f "$PROJECT_DIR/$script" ]; then
        log_test_skip "(script not found: $script)"
        return 0
    fi
    
    if timeout "$timeout_sec" "$PROJECT_DIR/$script" > "$log_file" 2>&1; then
        log_test_pass
        return 0
    else
        local exit_code=$?
        if [ $exit_code -eq 124 ]; then
            log_test_fail "(timeout after ${timeout_sec}s)"
        else
            log_test_fail "(exit code: $exit_code, see $log_file)"
        fi
        return 1
    fi
}

# Run a test and check for specific success pattern in output
run_test_with_pattern() {
    local name="$1"
    local script="$2"
    local pattern="$3"
    local timeout_sec="${4:-${TEST_TIMEOUT:-600}}"
    local log_file="$LOG_DIR/${name}.log"
    
    log_test_start "$name"
    
    if [ ! -f "$PROJECT_DIR/$script" ]; then
        log_test_skip "(script not found)"
        return 0
    fi
    
    if timeout "$timeout_sec" "$PROJECT_DIR/$script" > "$log_file" 2>&1; then
        if grep -q "$pattern" "$log_file"; then
            log_test_pass
            return 0
        else
            log_test_fail "(pattern not found: $pattern)"
            return 1
        fi
    else
        local exit_code=$?
        log_test_fail "(exit code: $exit_code)"
        return 1
    fi
}

check_dependencies() {
    local missing=0
    
    echo "═══════════════════════════════════════════════════════════════"
    echo "Dependency Check"
    echo "═══════════════════════════════════════════════════════════════"
    
    # Rust/Cargo
    echo -n "[DEP] Rust toolchain... "
    if command -v cargo &> /dev/null; then
        echo -e "${GREEN}OK${NC} ($(rustc --version 2>/dev/null | cut -d' ' -f2))"
    else
        echo -e "${RED}MISSING${NC}"
        ((missing++))
    fi
    
    # Python3
    echo -n "[DEP] Python3... "
    if command -v python3 &> /dev/null; then
        echo -e "${GREEN}OK${NC} ($(python3 --version 2>/dev/null | cut -d' ' -f2))"
        # In CI, ensure required packages are present
        if [ "$CI" = "true" ]; then
            echo "    Installing Python dependencies..."
            python3 -m pip install --upgrade pip &>/dev/null
            python3 -m pip install pandas taos-ws-py &>/dev/null || echo "    Warning: Python dependency install failed"
        fi
    else
        echo -e "${RED}MISSING${NC}"
        ((missing++))
    fi
    
    # Docker (for TDengine)
    echo -n "[DEP] Docker... "
    if command -v docker &> /dev/null && docker info &> /dev/null; then
        echo -e "${GREEN}OK${NC}"
    else
        echo -e "${YELLOW}NOT RUNNING${NC} (TDengine tests will be skipped)"
    fi
    
    # TDengine container
    echo -n "[DEP] TDengine... "
    if docker ps 2>/dev/null | grep -q tdengine; then
        echo -e "${GREEN}RUNNING${NC}"
    else
        echo -e "${YELLOW}NOT RUNNING${NC} (attempting to start...)"
        if docker start tdengine 2>/dev/null || \
           docker run -d --name tdengine -p 6030:6030 -p 6041:6041 tdengine/tdengine:latest 2>/dev/null; then
            echo -n "    Waiting for TDengine to be ready..."
            for i in {1..30}; do
                if docker exec tdengine taos -s "SELECT SERVER_VERSION();" &>/dev/null; then
                    echo -e " ${GREEN}READY${NC}"
                    break
                fi
                echo -n "."
                sleep 2
                if [ $i -eq 30 ]; then
                    echo -e " ${RED}TIMEOUT${NC}"
                fi
            done
        else
            echo "    Could not start TDengine"
        fi
    fi
    
    echo ""
    
    if [ $missing -gt 0 ]; then
        echo -e "${RED}ERROR: Missing $missing required dependencies${NC}"
        return 2
    fi
    return 0
}

generate_junit_report() {
    local xml_file="$REPORT_DIR/junit-report.xml"
    local end_time=$(date +%s)
    local duration=$((end_time - START_TIME))
    
    cat > "$xml_file" << EOF
<?xml version="1.0" encoding="UTF-8"?>
<testsuites name="Integration Tests" tests="$TOTAL" failures="$FAILED" skipped="$SKIPPED" time="$duration">
  <testsuite name="Phase0x09" tests="$TOTAL" failures="$FAILED" skipped="$SKIPPED" time="$duration">
EOF
    
    # Add test cases from logs
    for log in "$LOG_DIR"/*.log; do
        local test_name=$(basename "$log" .log)
        if [ -f "$log" ]; then
            if grep -q "PASS\|passed\|success" "$log" 2>/dev/null; then
                echo "    <testcase name=\"$test_name\" classname=\"integration\" />" >> "$xml_file"
            elif grep -q "SKIP" "$log" 2>/dev/null; then
                echo "    <testcase name=\"$test_name\" classname=\"integration\"><skipped/></testcase>" >> "$xml_file"
            else
                local error_msg=$(tail -5 "$log" 2>/dev/null | tr '\n' ' ' | head -c 200)
                echo "    <testcase name=\"$test_name\" classname=\"integration\"><failure message=\"Test failed\">$error_msg</failure></testcase>" >> "$xml_file"
            fi
        fi
    done
    
    cat >> "$xml_file" << EOF
  </testsuite>
</testsuites>
EOF
    
    echo "JUnit report: $xml_file"
}

# ============================================================================
# Main Test Execution
# ============================================================================

main() {
    echo "╔════════════════════════════════════════════════════════════════╗"
    echo "║     Integration Test Suite for CI/CD                          ║"
    echo "╚════════════════════════════════════════════════════════════════╝"
    echo ""
    echo "Mode: $([ "$QUICK_MODE" = true ] && echo 'QUICK (skip 1.3M)' || echo 'FULL')"
    echo "Log directory: $LOG_DIR"
    echo ""
    
    # Check dependencies
    check_dependencies || exit 2
    
    # ========== Phase 1: Unit Tests ==========
    echo "═══════════════════════════════════════════════════════════════"
    echo "Phase 1: Unit Tests"
    echo "═══════════════════════════════════════════════════════════════"
    
    log_test_start "Cargo Build"
    if cargo build --release > "$LOG_DIR/cargo_build.log" 2>&1; then
        log_test_pass
    else
        log_test_fail "(see $LOG_DIR/cargo_build.log)"
    fi
    
    log_test_start "Cargo Test"
    if cargo test > "$LOG_DIR/cargo_test.log" 2>&1; then
        log_test_pass
    else
        log_test_fail "(see $LOG_DIR/cargo_test.log)"
    fi
    
    # ========== Phase 2: Pipeline Correctness ==========
    echo ""
    echo "═══════════════════════════════════════════════════════════════"
    echo "Phase 2: Pipeline Correctness"
    echo "═══════════════════════════════════════════════════════════════"
    
    run_test_with_pattern "Pipeline_100K" "scripts/test_pipeline_compare.sh 100k" "ALL TESTS PASSED" 600
    
    if [ "$QUICK_MODE" = true ]; then
        log_test_start "Pipeline_1.3M"
        log_test_skip "(quick mode)"
    else
        run_test_with_pattern "Pipeline_1.3M" "scripts/test_pipeline_compare.sh highbal" "ALL TESTS PASSED" 3600
    fi
    
    # ========== Phase 3: Settlement Persistence ==========
    echo ""
    echo "═══════════════════════════════════════════════════════════════"
    echo "Phase 3: Settlement Persistence"
    echo "═══════════════════════════════════════════════════════════════"
    
    # Note: test_persistence.sh uses 'docker exec tdengine' which doesn't work
    # with GitHub Actions service containers. Skip in CI and rely on API tests.
    if [ "$CI" = "true" ]; then
        log_test_start "Persistence"
        log_test_skip "(skipped in CI - service container incompatible with docker exec)"
    elif docker ps 2>/dev/null | grep -q tdengine; then
        run_test "Persistence" "scripts/test_persistence.sh" 300
    else
        log_test_start "Persistence"
        log_test_skip "(TDengine not running)"
    fi
    
    # ========== Phase 4: HTTP API ==========
    echo ""
    echo "═══════════════════════════════════════════════════════════════"
    echo "Phase 4: HTTP API Endpoints"
    echo "═══════════════════════════════════════════════════════════════"
    
    run_test "Gateway_E2E" "scripts/test_order_api.sh" 180
    run_test "KLine_E2E" "scripts/test_kline_e2e.sh" 180
    run_test "Depth_API" "scripts/test_depth.sh" 120
    
    # ========== Phase 5: Account Integration ==========
    echo ""
    echo "═══════════════════════════════════════════════════════════════"
    echo "Phase 5: Account Integration (PostgreSQL)"
    echo "═══════════════════════════════════════════════════════════════"
    
    if docker ps 2>/dev/null | grep -q postgres; then
        run_test "Account_Integration" "scripts/test_account_integration.sh" 120
    else
        log_test_start "Account_Integration"
        log_test_skip "(PostgreSQL not running)"
    fi
    
    # ========== Summary ==========
    echo ""
    echo "═══════════════════════════════════════════════════════════════"
    echo "Test Summary"
    echo "═══════════════════════════════════════════════════════════════"
    
    local end_time=$(date +%s)
    local duration=$((end_time - START_TIME))
    
    echo ""
    echo "Total:   $TOTAL"
    echo "Passed:  $PASSED"
    echo "Failed:  $FAILED"
    echo "Skipped: $SKIPPED"
    echo "Time:    ${duration}s"
    echo ""
    
    # Generate JUnit report for CI
    generate_junit_report
    
    if [ $FAILED -eq 0 ]; then
        echo -e "${GREEN}╔════════════════════════════════════════════════════════════════╗${NC}"
        echo -e "${GREEN}║            ✅ ALL TESTS PASSED                                 ║${NC}"
        echo -e "${GREEN}╚════════════════════════════════════════════════════════════════╝${NC}"
        exit 0
    else
        echo -e "${RED}╔════════════════════════════════════════════════════════════════╗${NC}"
        echo -e "${RED}║            ❌ $FAILED TEST(S) FAILED                              ║${NC}"
        echo -e "${RED}╚════════════════════════════════════════════════════════════════╝${NC}"
        exit 1
    fi
}

cd "$PROJECT_DIR"
main "$@"
