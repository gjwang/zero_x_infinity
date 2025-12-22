#!/bin/bash
# =============================================================================
# Validation Tests - AssetName/SymbolName Constraints
# =============================================================================
# Tests database CHECK constraints for uppercase enforcement
# Usage: ./scripts/tests/test_validation.sh
# =============================================================================

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LIB_DIR="$(dirname "$SCRIPT_DIR")/lib"
PROJECT_DIR="$(dirname "$(dirname "$SCRIPT_DIR")")"

# Source helper libraries
source "$LIB_DIR/test_helpers.sh"
source "$LIB_DIR/db_helpers.sh"

cd "$PROJECT_DIR"

# =============================================================================
# Main Test Suite
# =============================================================================

test_section "Validation Tests - Database Constraints"

# -----------------------------------------------------------------------------
# Step 1: Check Prerequisites
# -----------------------------------------------------------------------------

test_start "Check PostgreSQL connection"
if db_exec_quiet "SELECT 1"; then
    log_success "PostgreSQL is accessible"
else
    log_error "PostgreSQL is not accessible"
    exit 1
fi

test_start "Check assets table exists"
if db_table_exists "assets"; then
    log_success "Assets table exists"
else
    log_error "Assets table does not exist - run migrations first"
    exit 1
fi

test_start "Check symbols table exists"
if db_table_exists "symbols"; then
    log_success "Symbols table exists"
else
    log_error "Symbols table does not exist - run migrations first"
    exit 1
fi

# -----------------------------------------------------------------------------
# Step 2: Check Constraints Exist
# -----------------------------------------------------------------------------

test_start "Check uppercase constraint on assets"
if db_constraint_exists "chk_asset_uppercase"; then
    log_success "Asset uppercase constraint exists"
else
    log_warn "Asset uppercase constraint not found - need to apply migration 002"
fi

test_start "Check uppercase constraint on symbols"
if db_constraint_exists "chk_symbol_uppercase"; then
    log_success "Symbol uppercase constraint exists"
else
    log_warn "Symbol uppercase constraint not found - need to apply migration 002"
fi

# -----------------------------------------------------------------------------
# Step 3: Asset Validation Tests
# -----------------------------------------------------------------------------

test_section "Asset Validation Tests"

test_lowercase_rejected "assets" "asset" "btc_test" "Lowercase asset should be rejected"
test_lowercase_rejected "assets" "asset" "Btc_Test" "Mixed case asset should be rejected"
test_uppercase_accepted "assets" "asset" "BTC_TEST" "Uppercase asset should be accepted"
test_uppercase_accepted "assets" "asset" "ETH_2" "Uppercase with numbers should be accepted"

# -----------------------------------------------------------------------------
# Step 4: Symbol Validation Tests
# -----------------------------------------------------------------------------

test_section "Symbol Validation Tests"

test_lowercase_rejected "symbols" "symbol" "btc_usdt" "Lowercase symbol should be rejected"
test_lowercase_rejected "symbols" "symbol" "Btc_Usdt" "Mixed case symbol should be rejected"
test_uppercase_accepted "symbols" "symbol" "BTC_TEST_SYMBOL" "Uppercase symbol should be accepted"

# -----------------------------------------------------------------------------
# Summary
# -----------------------------------------------------------------------------

test_summary
exit $?
