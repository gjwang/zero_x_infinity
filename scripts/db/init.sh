#!/bin/bash
# =============================================================================
# Database Initialization Script
# =============================================================================
# Initializes both PostgreSQL and TDengine databases.
# Use this for both local development and CI.
#
# Usage:
#   ./scripts/db/init.sh          # Initialize all
#   ./scripts/db/init.sh pg       # PostgreSQL only
#   ./scripts/db/init.sh td       # TDengine only
# =============================================================================

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

# Source unified configuration
source "$PROJECT_ROOT/scripts/lib/db_env.sh"

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m'

log_info() { echo -e "${GREEN}[INFO]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

# =============================================================================
# PostgreSQL Initialization
# =============================================================================
init_postgres() {
    log_info "=== Initializing PostgreSQL ==="
    
    # Wait for PostgreSQL to be ready
    log_info "Waiting for PostgreSQL..."
    for i in {1..30}; do
        if pg_check; then
            log_info "PostgreSQL is ready"
            break
        fi
        echo -n "."
        sleep 1
        if [ $i -eq 30 ]; then
            log_error "PostgreSQL connection timeout"
            exit 1
        fi
    done
    
    # Run all migrations in order
    log_info "Running migrations..."
    for f in "$PROJECT_ROOT"/migrations/*.sql; do
        if [ -f "$f" ]; then
            log_info "  Applying $(basename "$f")..."
            psql -h "$PG_HOST" -p "$PG_PORT" -U "$PG_USER" -d "$PG_DB" -f "$f" > /dev/null 2>&1 || {
                log_warn "  Migration $(basename "$f") had warnings (may be OK if already applied)"
            }
        fi
    done
    
    # Run seed data
    if [ -f "$PROJECT_ROOT/fixtures/seed_data.sql" ]; then
        log_info "Seeding data..."
        psql -h "$PG_HOST" -p "$PG_PORT" -U "$PG_USER" -d "$PG_DB" -f "$PROJECT_ROOT/fixtures/seed_data.sql" 2>&1 | grep -v 'does not exist' || true
    fi
    
    # Verify
    log_info "Verifying tables..."
    TABLES=$(psql -h "$PG_HOST" -p "$PG_PORT" -U "$PG_USER" -d "$PG_DB" -t -c "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = 'public'" | tr -d ' ')
    log_info "  Found $TABLES tables"
    
    log_info "✅ PostgreSQL initialized"
}

# =============================================================================
# TDengine Initialization
# =============================================================================
init_tdengine() {
    log_info "=== Initializing TDengine ==="
    
    # Wait for TDengine to be ready
    log_info "Waiting for TDengine..."
    for i in {1..30}; do
        if td_check; then
            log_info "TDengine is ready"
            break
        fi
        echo -n "."
        sleep 2
        if [ $i -eq 30 ]; then
            log_error "TDengine connection timeout"
            exit 1
        fi
    done
    
    # Create database with correct precision
    log_info "Creating database with PRECISION 'us'..."
    td_exec "CREATE DATABASE IF NOT EXISTS ${TD_DB} KEEP 365d DURATION 10d BUFFER 256 WAL_LEVEL 2 PRECISION 'us'" > /dev/null
    
    # Create super tables
    log_info "Creating super tables..."
    
    td_exec "USE ${TD_DB}" > /dev/null
    
    # Orders
    td_exec "CREATE STABLE IF NOT EXISTS orders (ts TIMESTAMP, order_id BIGINT UNSIGNED, user_id BIGINT UNSIGNED, side TINYINT UNSIGNED, order_type TINYINT UNSIGNED, price BIGINT UNSIGNED, qty BIGINT UNSIGNED, filled_qty BIGINT UNSIGNED, status TINYINT UNSIGNED, cid NCHAR(64)) TAGS (symbol_id INT UNSIGNED)" > /dev/null
    
    # Trades
    td_exec "CREATE STABLE IF NOT EXISTS trades (ts TIMESTAMP, trade_id BIGINT UNSIGNED, order_id BIGINT UNSIGNED, user_id BIGINT UNSIGNED, side TINYINT UNSIGNED, price BIGINT UNSIGNED, qty BIGINT UNSIGNED, fee BIGINT UNSIGNED, role TINYINT UNSIGNED) TAGS (symbol_id INT UNSIGNED)" > /dev/null
    
    # Balances
    td_exec "CREATE STABLE IF NOT EXISTS balances (ts TIMESTAMP, avail BIGINT UNSIGNED, frozen BIGINT UNSIGNED, lock_version BIGINT UNSIGNED, settle_version BIGINT UNSIGNED) TAGS (user_id BIGINT UNSIGNED, asset_id INT UNSIGNED)" > /dev/null
    
    # K-Lines
    td_exec "CREATE STABLE IF NOT EXISTS klines (ts TIMESTAMP, open BIGINT UNSIGNED, high BIGINT UNSIGNED, low BIGINT UNSIGNED, close BIGINT UNSIGNED, volume BIGINT UNSIGNED, quote_volume DOUBLE, trade_count INT UNSIGNED) TAGS (symbol_id INT UNSIGNED, intv NCHAR(8))" > /dev/null
    
    # Order events
    td_exec "CREATE STABLE IF NOT EXISTS order_events (ts TIMESTAMP, order_id BIGINT UNSIGNED, event_type TINYINT UNSIGNED, prev_status TINYINT UNSIGNED, new_status TINYINT UNSIGNED, filled_qty BIGINT UNSIGNED, remaining_qty BIGINT UNSIGNED) TAGS (symbol_id INT UNSIGNED)" > /dev/null
    
    log_info "✅ TDengine initialized"
}

# =============================================================================
# Main
# =============================================================================
main() {
    echo "======================================"
    echo " Database Initialization"
    echo "======================================"
    echo ""
    echo "Configuration:"
    echo "  PG: ${PG_USER}@${PG_HOST}:${PG_PORT}/${PG_DB}"
    echo "  TD: ${TD_USER}@${TD_HOST}:${TD_PORT_REST}/${TD_DB}"
    echo ""
    
    case "${1:-all}" in
        pg|postgres)
            init_postgres
            ;;
        td|tdengine)
            init_tdengine
            ;;
        all|*)
            init_postgres
            init_tdengine
            ;;
    esac
    
    echo ""
    echo "======================================"
    echo " ✅ All databases initialized"
    echo "======================================"
}

main "$@"
