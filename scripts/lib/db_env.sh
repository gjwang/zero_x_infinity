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
# Default PG_PORT varies by environment
if [ "$CI" = "true" ]; then
    DEFAULT_PG_PORT="5432"
else
    DEFAULT_PG_PORT="5433"
fi

export PG_HOST="${PG_HOST:-localhost}"
export PG_PORT="${PG_PORT:-$DEFAULT_PG_PORT}"
export PG_USER="${PG_USER:-trading}"
export PG_PASSWORD="${PG_PASSWORD:-trading123}"
export PG_DB="${PG_DB:-exchange_info_db}"

# Connection URL (for sqlx and other tools)
export DATABASE_URL="postgresql://${PG_USER}:${PG_PASSWORD}@${PG_HOST}:${PG_PORT}/${PG_DB}"

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
