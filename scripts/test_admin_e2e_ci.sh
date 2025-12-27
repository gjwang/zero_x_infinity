#!/bin/bash
set -e

# ==============================================================================
# Admin Dashboard <-> Gateway E2E CI Runner
# ==============================================================================
# Usage: ./scripts/test_admin_e2e_ci.sh
#
# Prereqs (CI Environment):
# - Postgres running on localhost:5432
# - TDengine running on localhost:6030
# - Gateway binary at ./target/release/zero_x_infinity
# - Admin deps installed
# ==============================================================================

# 0. Configuration
PROJECT_ROOT=$(pwd)
ADMIN_DIR="$PROJECT_ROOT/admin"
GATEWAY_BIN="$PROJECT_ROOT/target/release/zero_x_infinity"
LOG_DIR="$PROJECT_ROOT/logs"
mkdir -p "$LOG_DIR"

# Source DB environment variables (handles CI vs Local ports automatically)
source "$PROJECT_ROOT/scripts/lib/db_env.sh"
print_db_config

export GATEWAY_PORT=${GATEWAY_PORT:-8081}  # Dev port to avoid conflict with QA on 8080
export ADMIN_PORT=${ADMIN_PORT:-8002}     # Dev port to avoid conflict with QA on 8001

echo "🚀 Starting Admin <-> Gateway E2E Automation..."

# Standard venv path (must be 'venv', not '.venv')
VENV_PYTHON="$ADMIN_DIR/venv/bin/python"
if [ ! -f "$VENV_PYTHON" ]; then
    echo "❌ Python venv not found at $ADMIN_DIR/venv"
    echo "   Run: cd admin && python3 -m venv venv && pip install -r requirements.txt"
    exit 1
fi

# 1. Database Initialization
echo "📦 Initializing Databases..."
# Force fresh init using dynamic vars
PGPASSWORD=$PG_PASSWORD psql -h $PG_HOST -p $PG_PORT -U $PG_USER -d $PG_DB -c "DROP SCHEMA public CASCADE; CREATE SCHEMA public;"
$VENV_PYTHON $ADMIN_DIR/init_db.py
echo "✅ Database Initialized"

# 2. Start Admin Dashboard (Background)
echo "Starting Admin Dashboard..."
cd "$ADMIN_DIR"
# Assuming .venv is active or environment is prepared
$VENV_PYTHON -m uvicorn main:app --host 0.0.0.0 --port $ADMIN_PORT --workers 1 > "$LOG_DIR/admin_dashboard.log" 2>&1 &
ADMIN_PID=$!
cd "$PROJECT_ROOT"

# Wait for Admin Health
echo "⏳ Waiting for Admin Dashboard..."
for i in {1..30}; do
    if curl -s "http://localhost:$ADMIN_PORT/health" > /dev/null; then
        echo "✅ Admin Dashboard Ready"
        break
    fi
    sleep 1
done

# 3. Start Gateway (Background)
# Update CI config to match current environment (port/password)
if [ -f "config/ci.yaml" ]; then
    # Escape slashes in DATABASE_URL for sed
    SAFE_PG_URL=$(echo $DATABASE_URL | sed 's/\//\\\//g')
    sed -i '' "s/postgres_url: .*/postgres_url: \"$SAFE_PG_URL\"/g" config/ci.yaml
fi

echo "Starting Gateway..."
if [ ! -f "$GATEWAY_BIN" ]; then
    echo "❌ Gateway binary not found at $GATEWAY_BIN"
    kill $ADMIN_PID
    exit 1
fi

$GATEWAY_BIN --gateway --env ci --port $GATEWAY_PORT > "$LOG_DIR/gateway.log" 2>&1 &
GATEWAY_PID=$!

# Wait for Gateway Health
echo "⏳ Waiting for Gateway..."
for i in {1..30}; do
    if curl -s "http://localhost:$GATEWAY_PORT/api/v1/health" > /dev/null; then
        echo "✅ Gateway Ready"
        break
    fi
    sleep 1
done

# 4. Run E2E Test
echo "🧪 Running E2E Test Script..."
set +e # Allow test failure to handle cleanup
cd "$ADMIN_DIR"
$VENV_PYTHON test_admin_gateway_e2e.py
EXIT_CODE=$?
cd "$PROJECT_ROOT"

# 5. Cleanup
echo "🧹 Cleaning up..."
kill $ADMIN_PID || true
kill $GATEWAY_PID || true

if [ $EXIT_CODE -eq 0 ]; then
    echo "🎉 E2E Tests PASSED"
else
    echo "❌ E2E Tests FAILED"
    echo "=== Admin Log Tail ==="
    tail -n 20 "$LOG_DIR/admin_dashboard.log"
    echo "=== Gateway Log Tail ==="
    tail -n 20 "$LOG_DIR/gateway.log"
fi

exit $EXIT_CODE
