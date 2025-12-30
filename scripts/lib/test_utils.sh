#!/bin/bash

# ==============================================================================
# Unified Test Utilities
# Common functions for CI/Local testing, cleanup, and readiness checks.
# Usage: source scripts/lib/test_utils.sh
# ==============================================================================

# ANSI Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

log_info() { echo -e "${GREEN}[INFO]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_err() { echo -e "${RED}[ERROR]${NC} $1"; }

# ------------------------------------------------------------------------------
# 1. Global Settings & Environment Detection
# ------------------------------------------------------------------------------

# Standardized Paths & Ports
export LOG_DIR="${LOG_DIR:-logs}"
export GATEWAY_PORT="${GATEWAY_PORT:-8080}"
export GATEWAY_HOST="${GATEWAY_HOST:-localhost}"
export BASE_URL="http://${GATEWAY_HOST}:${GATEWAY_PORT}"

detect_ci_env() {
    if [ -n "$CI" ] || [ -f "/.dockerenv" ]; then
        log_info "CI/Docker Environment Detected"
        export IS_CI=true
        export GATEWAY_ARGS="--gateway --env ci"
        export DB_HOST="localhost"
        # In CI, we might want absolute paths if CWD varies?
        # But usually scripts run from project root or consistent relative content.
    else
        log_info "Local Development Environment Detected"
        export IS_CI=false
        export GATEWAY_ARGS="--gateway --env dev"
    fi
}

# ------------------------------------------------------------------------------
# 2. Process Management & Cleanup
# ------------------------------------------------------------------------------
cleanup_gateway_process() {
    local bin_name="zero_x_infinity"
    log_info "Cleaning up existing '$bin_name' processes..."
    
    # Use -x for exact binary name match to avoid killing scripts in directories
    # named 'zero_x_infinity'. The -f flag matches the full command line, which
    # incorrectly matches shell scripts running from /path/to/zero_x_infinity/.
    if pgrep -x "$bin_name" > /dev/null; then
        if [ "$IS_CI" = "true" ]; then
            # Aggressive cleanup in CI - use -x for exact match
            pkill -9 -x "$bin_name" || true
            sleep 2
            log_info "Force killed lingering processes (CI mode)"
        else
            # Gentle cleanup locally to avoid killing IDE/Language Server
            log_warn "Local cleanup: Please ensure no other Gateway instances are running."
            log_warn "Skipping 'pkill' locally to prevent IDE crashes."
            # Optional: Check port 8080
            if lsof -i :8080 -sTCP:LISTEN -t >/dev/null; then
                 log_warn "Port 8080 is in use! You might need to stop the server manually."
            fi
        fi
    else
        log_info "No existing processes found."
    fi
}

# ------------------------------------------------------------------------------
# 3. Directory Setup
# ------------------------------------------------------------------------------
ensure_log_dir() {
    local log_dir="${1:-logs}"
    if [ ! -d "$log_dir" ]; then
        mkdir -p "$log_dir"
        log_info "Created log directory: $log_dir"
    fi
}

# ------------------------------------------------------------------------------
# 4. Readiness Checks
# ------------------------------------------------------------------------------
wait_for_postgres() {
    local max_retries=30
    local count=0
    
    # Ensure PG_USER is set (fallback to 'trading' which is the project default)
    local user="${PG_USER:-trading}"
    
    log_info "Waiting for PostgreSQL (user: $user)..."
    until pg_isready -h localhost -U "$user" >/dev/null 2>&1 || [ $count -eq $max_retries ]; do
        echo -n "."
        sleep 1
        count=$((count+1))
    done
    echo ""
    
    if [ $count -eq $max_retries ]; then
        log_err "PostgreSQL timed out!"
        return 1
    fi
    log_info "PostgreSQL is ready."
}

wait_for_gateway() {
    local port="${1:-8080}"
    local max_retries=60
    local count=0
    
    log_info "Waiting for Gateway on port $port..."
    while true; do
        if curl -s "http://localhost:$port/api/v1/health" >/dev/null; then
            log_info "Gateway is healthy!"
            return 0
        fi
        
        if [ $count -eq $max_retries ]; then
            log_err "Gateway failed to start after ${max_retries}s"
            return 1
        fi
        
        sleep 1
        count=$((count+1))
    done
}

# ------------------------------------------------------------------------------
# 5. Dependency Checks
# ------------------------------------------------------------------------------
check_command() {
    if ! command -v "$1" &> /dev/null; then
        log_err "Command '$1' not found. Please install it."
        exit 1
    fi
}

check_dependencies() {
    check_command curl
    check_command pgrep
    # check_command pkill (usually standard)
}

# Auto-run basic setup when sourced
check_dependencies
detect_ci_env
