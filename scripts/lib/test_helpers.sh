#!/bin/bash
# =============================================================================
# Test Helper Library
# =============================================================================
# Modular, reusable test functions
# Source this file: source scripts/lib/test_helpers.sh
# =============================================================================

# Colors
export RED='\033[0;31m'
export GREEN='\033[0;32m'
export YELLOW='\033[1;33m'
export BLUE='\033[0;34m'
export NC='\033[0m'

# Test counters
export TESTS_TOTAL=0
export TESTS_PASSED=0
export TESTS_FAILED=0
declare -a FAILED_TESTS

# =============================================================================
# Logging Functions
# =============================================================================

log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[PASS]${NC} $1"
    ((TESTS_PASSED++)) || true
}

log_error() {
    echo -e "${RED}[FAIL]${NC} $1"
    ((TESTS_FAILED++)) || true
    FAILED_TESTS+=("$1")
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

# =============================================================================
# Test Framework Functions
# =============================================================================

test_start() {
    ((TESTS_TOTAL++)) || true
    log_info "Test $TESTS_TOTAL: $1"
}

test_section() {
    echo ""
    echo -e "${BLUE}=============================================================================${NC}"
    echo -e "${BLUE} $1${NC}"
    echo -e "${BLUE}=============================================================================${NC}"
}

# Print test summary
test_summary() {
    echo ""
    test_section "Test Summary"
    echo -e "Total Tests:  ${BLUE}$TESTS_TOTAL${NC}"
    echo -e "Passed:       ${GREEN}$TESTS_PASSED${NC}"
    echo -e "Failed:       ${RED}$TESTS_FAILED${NC}"
    
    if [ $TESTS_FAILED -gt 0 ]; then
        echo ""
        log_error "Failed Tests:"
        for test in "${FAILED_TESTS[@]}"; do
            echo -e "  ${RED}✗${NC} $test"
        done
        echo ""
        return 1
    else
        echo ""
        log_success "All tests PASSED! ✓"
        return 0
    fi
}

# =============================================================================
# Assertion Functions
# =============================================================================

# Assert command succeeds
assert_success() {
    local cmd="$1"
    local msg="${2:-Command should succeed}"
    
    if eval "$cmd" >/dev/null 2>&1; then
        log_success "$msg"
        return 0
    else
        log_error "$msg"
        return 1
    fi
}

# Assert command fails
assert_failure() {
    local cmd="$1"
    local msg="${2:-Command should fail}"
    
    if eval "$cmd" >/dev/null 2>&1; then
        log_error "$msg"
        return 1
    else
        log_success "$msg"
        return 0
    fi
}

# Assert output contains pattern
assert_contains() {
    local output="$1"
    local pattern="$2"
    local msg="${3:-Output should contain pattern}"
    
    if echo "$output" | grep -qi "$pattern"; then
        log_success "$msg"
        return 0
    else
        log_error "$msg"
        return 1
    fi
}
