#!/bin/bash
# =============================================================================
# Database Configuration - Single Source of Truth
# =============================================================================
# All scripts should source this file instead of hardcoding credentials.
# 
# Usage:
#   source scripts/lib/db_env.sh
#   # Now use $PG_HOST, $DATABASE_URL, $TDENGINE_DSN, etc.
# =============================================================================

# -----------------------------------------------------------------------------
# PostgreSQL Configuration
# -----------------------------------------------------------------------------
# Read port from config files to avoid hardcoding
# CI uses config/ci.yaml (port 5432), Local uses config/dev.yaml (port 5433)

_DB_ENV_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$_DB_ENV_DIR/../.." && pwd)"

if [ "$CI" = "true" ]; then
    CONFIG_FILE="$PROJECT_ROOT/config/ci.yaml"
else
    CONFIG_FILE="$PROJECT_ROOT/config/dev.yaml"
fi

# Extract port from postgres_url in config file (e.g., localhost:5432)
if [ -f "$CONFIG_FILE" ]; then
    PG_URL_FROM_CONFIG=$(grep "postgres_url:" "$CONFIG_FILE" | head -1 | sed 's/.*localhost:\([0-9]*\).*/\1/')
    DEFAULT_PG_PORT="${PG_URL_FROM_CONFIG:-5432}"
else
    # Fallback if config file is missing
    DEFAULT_PG_PORT="5432"
fi

export PG_HOST="${PG_HOST:-localhost}"
export PG_PORT="${PG_PORT:-$DEFAULT_PG_PORT}"
export PG_USER="${PG_USER:-trading}"
export PG_PASSWORD="${PG_PASSWORD:-trading123}"
export PG_DB="${PG_DB:-exchange_info_db}"

# PostgreSQL Connection String (sync driver - for Rust/psql)
export DATABASE_URL="postgresql://${PG_USER}:${PG_PASSWORD}@${PG_HOST}:${PG_PORT}/${PG_DB}"

# PostgreSQL Async Connection String (async driver - for Python Admin Dashboard)
export DATABASE_URL_ASYNC="postgresql+asyncpg://${PG_USER}:${PG_PASSWORD}@${PG_HOST}:${PG_PORT}/${PG_DB}"

# For psql commands
export PGPASSWORD="${PG_PASSWORD}"

# -----------------------------------------------------------------------------
# TDengine Configuration
# -----------------------------------------------------------------------------
export TD_HOST="${TD_HOST:-localhost}"
export TD_PORT_NATIVE="${TD_PORT_NATIVE:-6030}"
export TD_PORT_REST="${TD_PORT_REST:-6041}"
export TD_USER="${TD_USER:-root}"
export TD_PASSWORD="${TD_PASSWORD:-taosdata}"
export TD_DB="${TD_DB:-trading}"

# Connection DSN (for taos client)
export TDENGINE_DSN="taos+ws://${TD_USER}:${TD_PASSWORD}@${TD_HOST}:${TD_PORT_REST}"

# REST API base URL
export TD_REST_URL="http://${TD_HOST}:${TD_PORT_REST}"

# -----------------------------------------------------------------------------
# Derived URLs (convenience)
# -----------------------------------------------------------------------------
export PG_CONN_STRING="host=${PG_HOST} port=${PG_PORT} user=${PG_USER} password=${PG_PASSWORD} dbname=${PG_DB}"

# -----------------------------------------------------------------------------
# Helper Functions
# -----------------------------------------------------------------------------

# Check if PostgreSQL is accessible
pg_check() {
    psql -h "$PG_HOST" -p "$PG_PORT" -U "$PG_USER" -d "$PG_DB" -c "SELECT 1" > /dev/null 2>&1
}

# Check if TDengine is accessible
td_check() {
    curl -s -u "${TD_USER}:${TD_PASSWORD}" "${TD_REST_URL}/rest/login/${TD_USER}/${TD_PASSWORD}" > /dev/null 2>&1
}

# Run psql command
pg_exec() {
    psql -h "$PG_HOST" -p "$PG_PORT" -U "$PG_USER" -d "$PG_DB" -c "$1"
}

# Run TDengine SQL via REST
td_exec() {
    curl -s -u "${TD_USER}:${TD_PASSWORD}" -d "$1" "${TD_REST_URL}/rest/sql"
}

# Mask sensitive values (show first 3 chars + ***)
mask_secret() {
    local val="$1"
    if [ ${#val} -le 3 ]; then
        echo "***"
    else
        echo "${val:0:3}***"
    fi
}

# Print current database configuration (call at script startup for transparency)
print_db_config() {
    # Extract environment name from config file (e.g., dev.yaml -> DEV)
    local env_name="unknown"
    if [ -n "$CONFIG_FILE" ]; then
        env_name=$(basename "$CONFIG_FILE" .yaml | tr '[:lower:]' '[:upper:]')
    fi
    
    echo "========================================"
    echo " Database Configuration"
    echo "========================================"
    echo "  Runtime Environment: $env_name"
    echo "  Config File: ${CONFIG_FILE:-<not set>}"
    echo ""
    echo "  PostgreSQL:"
    echo "    Host:     $PG_HOST"
    echo "    Port:     $PG_PORT"
    echo "    User:     $PG_USER"
    echo "    Password: $(mask_secret "$PG_PASSWORD")"
    echo "    Database: $PG_DB"
    echo ""
    echo "  TDengine:"
    echo "    Host:     $TD_HOST"
    echo "    REST Port: $TD_PORT_REST"
    echo "    User:     $TD_USER"
    echo "    Password: $(mask_secret "$TD_PASSWORD")"
    echo "    Database: $TD_DB"
    echo "========================================"
}
