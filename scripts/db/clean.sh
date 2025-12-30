#!/bin/bash
# =============================================================================
# Database Cleanup Script
# =============================================================================
# Cleans data from both PostgreSQL and TDengine (preserves structure).
# Use this between test runs.
#
# Usage:
#   ./scripts/db/clean.sh          # Clean all
#   ./scripts/db/clean.sh pg       # PostgreSQL only
#   ./scripts/db/clean.sh td       # TDengine only
# =============================================================================

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

# Source unified configuration
source "$PROJECT_ROOT/scripts/lib/db_env.sh"

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

log_info() { echo -e "${GREEN}[INFO]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }

# =============================================================================
# PostgreSQL Cleanup
# =============================================================================
clean_postgres() {
    log_info "Cleaning PostgreSQL..."
    
    if ! pg_check; then
        log_warn "PostgreSQL not accessible, skipping"
        return
    fi
    
    # Clean transactional data, keep reference data
    # Adjust tables as needed for your schema
    psql -h "$PG_HOST" -p "$PG_PORT" -U "$PG_USER" -d "$PG_DB" -c "
        -- Clean transactional tables (if they exist)
        TRUNCATE TABLE transfers_tb CASCADE;
        TRUNCATE TABLE deposit_history CASCADE;
        TRUNCATE TABLE withdraw_history CASCADE;
        TRUNCATE TABLE fsm_transfers_tb CASCADE;
        TRUNCATE TABLE transfer_operations_tb CASCADE;
    " 2>/dev/null || true
    
    log_info "✅ PostgreSQL cleaned"
}

# =============================================================================
# TDengine Cleanup
# =============================================================================
clean_tdengine() {
    log_info "Cleaning TDengine..."
    
    if ! td_check; then
        log_warn "TDengine not accessible, skipping"
        return
    fi
    
    # Delete data from super tables (keeps table structure)
    for table in orders trades balances order_events klines; do
        td_exec "DELETE FROM ${TD_DB}.${table}" 2>/dev/null || true
    done
    
    log_info "✅ TDengine cleaned"
}

# =============================================================================
# Main
# =============================================================================
main() {
    log_info "=== Database Cleanup ==="
    
    case "${1:-all}" in
        pg|postgres)
            clean_postgres
            ;;
        td|tdengine)
            clean_tdengine
            ;;
        all|*)
            clean_postgres
            clean_tdengine
            ;;
    esac
    
    log_info "=== Cleanup complete ==="
}

main "$@"
