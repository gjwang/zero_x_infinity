#!/bin/bash
# =============================================================================
# Database Helper Library
# =============================================================================
# Modular, reusable database functions for PostgreSQL
# Source this file: source scripts/lib/db_helpers.sh
# =============================================================================

# Database configuration
export PG_CONTAINER="${PG_CONTAINER:-postgres}"
export PG_USER="${PG_USER:-trading}"
export PG_DB="${PG_DB:-trading}"

# =============================================================================
# Core Database Functions
# =============================================================================

# Execute SQL command
db_exec() {
    local sql="$1"
    docker exec "$PG_CONTAINER" psql -U "$PG_USER" -d "$PG_DB" -c "$sql" 2>&1
}

# Execute SQL command quietly (no output on success)
db_exec_quiet() {
    local sql="$1"
    docker exec "$PG_CONTAINER" psql -U "$PG_USER" -d "$PG_DB" -c "$sql" >/dev/null 2>&1
}

# Execute SQL file
db_exec_file() {
    local file="$1"
    docker exec -i "$PG_CONTAINER" psql -U "$PG_USER" -d "$PG_DB" < "$file" 2>&1
}

# Check if table exists
db_table_exists() {
    local table="$1"
    docker exec "$PG_CONTAINER" psql -U "$PG_USER" -d "$PG_DB" -c "\d $table" >/dev/null 2>&1
}

# Check if constraint exists
db_constraint_exists() {
    local constraint="$1"
    local result
    result=$(docker exec "$PG_CONTAINER" psql -U "$PG_USER" -d "$PG_DB" -t -c \
        "SELECT conname FROM pg_constraint WHERE conname = '$constraint'" 2>/dev/null)
    [ -n "$(echo "$result" | tr -d ' \n')" ]
}

# =============================================================================
# Schema Management
# =============================================================================

# Drop all tables and recreate from fresh
db_reset_schema() {
    log_info "Resetting database schema..."
    
    # Drop existing tables
    db_exec_quiet "DROP TABLE IF EXISTS symbols CASCADE"
    db_exec_quiet "DROP TABLE IF EXISTS assets CASCADE"
    db_exec_quiet "DROP TABLE IF EXISTS users CASCADE"
    
    log_success "Old tables dropped"
}

# Apply migration file
db_apply_migration() {
    local migration_file="$1"
    local migration_name
    migration_name=$(basename "$migration_file")
    
    log_info "Applying migration: $migration_name"
    
    if [ ! -f "$migration_file" ]; then
        log_error "Migration file not found: $migration_file"
        return 1
    fi
    
    local result
    result=$(db_exec_file "$migration_file")
    
    if echo "$result" | grep -qi "error"; then
        log_error "Migration failed: $migration_name"
        log_info "Error: $result"
        return 1
    fi
    
    log_success "Migration applied: $migration_name"
    return 0
}

# =============================================================================
# Test Data Functions
# =============================================================================

# Insert test asset
db_insert_asset() {
    local asset="$1"
    local name="${2:-Test Asset}"
    local decimals="${3:-8}"
    
    db_exec "INSERT INTO assets (asset, name, decimals, status, asset_flags) VALUES ('$asset', '$name', $decimals, 1, 7)"
}

# Delete test asset
db_delete_asset() {
    local asset="$1"
    db_exec_quiet "DELETE FROM assets WHERE asset = '$asset'"
}

# Insert test symbol
db_insert_symbol() {
    local symbol="$1"
    local base_id="${2:-1}"
    local quote_id="${3:-2}"
    
    db_exec "INSERT INTO symbols (symbol, base_asset_id, quote_asset_id, price_decimals, qty_decimals, min_qty, status, symbol_flags) VALUES ('$symbol', $base_id, $quote_id, 2, 8, 1000, 1, 3)"
}

# Delete test symbol
db_delete_symbol() {
    local symbol="$1"
    db_exec_quiet "DELETE FROM symbols WHERE symbol = '$symbol'"
}

# =============================================================================
# Validation Test Helpers
# =============================================================================

# Test that lowercase value is rejected by CHECK constraint
test_lowercase_rejected() {
    local table="$1"
    local column="$2"
    local value="$3"
    local msg="$4"
    
    test_start "$msg"
    
    local result
    if [ "$table" = "assets" ]; then
        result=$(db_insert_asset "$value" 2>&1)
    elif [ "$table" = "symbols" ]; then
        result=$(db_insert_symbol "$value" 2>&1)
    fi
    
    if echo "$result" | grep -qi "violates check constraint\|check.*uppercase"; then
        log_success "Lowercase correctly rejected: $value"
        return 0
    else
        # Cleanup if inserted
        if [ "$table" = "assets" ]; then
            db_delete_asset "$value"
        elif [ "$table" = "symbols" ]; then
            db_delete_symbol "$value"
        fi
        log_error "Lowercase was accepted (should be rejected): $value"
        return 1
    fi
}

# Test that uppercase value is accepted
test_uppercase_accepted() {
    local table="$1"
    local column="$2"
    local value="$3"
    local msg="$4"
    
    test_start "$msg"
    
    local result
    if [ "$table" = "assets" ]; then
        result=$(db_insert_asset "$value" 2>&1)
    elif [ "$table" = "symbols" ]; then
        result=$(db_insert_symbol "$value" 2>&1)
    fi
    
    if echo "$result" | grep -qi "error\|violates"; then
        log_error "Uppercase was rejected (should be accepted): $value"
        return 1
    else
        log_success "Uppercase correctly accepted: $value"
        # Cleanup
        if [ "$table" = "assets" ]; then
            db_delete_asset "$value"
        elif [ "$table" = "symbols" ]; then
            db_delete_symbol "$value"
        fi
        return 0
    fi
}
