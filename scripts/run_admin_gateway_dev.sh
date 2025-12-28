#!/bin/bash
# =============================================================================
# Dev Environment Runner: Admin + Gateway
# Usage: ./scripts/run_admin_gateway_dev.sh
# =============================================================================

PROJECT_ROOT=$(pwd)
ADMIN_DIR="$PROJECT_ROOT/admin"
GATEWAY_BIN="$PROJECT_ROOT/target/release/zero_x_infinity"
LOG_DIR="$PROJECT_ROOT/logs"
mkdir -p "$LOG_DIR"

# Source Dynamic DB Config
source "$PROJECT_ROOT/scripts/lib/db_env.sh"
print_db_config

export GATEWAY_PORT=8080
export ADMIN_PORT=8001

# Cleanup function
cleanup() {
    echo "🧹 Stopping services..."
    kill $(jobs -p) 2>/dev/null
    exit 0
}
trap cleanup SIGINT SIGTERM

echo "🚀 Starting Dev Environment..."

# 1. DB Init
echo "📦 Reseting Database..."
# Use the robust init script which runs all migrations (including Core & Admin)
# and ensures balances_tb, etc. exist for the Gateway.
"$PROJECT_ROOT/scripts/db/init.sh" --reset pg

# 2. Patch Gateway Config (if needed)
if [ -f "config/ci.yaml" ]; then
    SAFE_PG_URL=$(echo $DATABASE_URL | sed 's/\//\\\//g')
    sed -i '' "s/postgres_url: .*/postgres_url: \"$SAFE_PG_URL\"/g" config/ci.yaml
fi

# 3. Start Services
echo "Starting Admin..."
cd "$ADMIN_DIR" && $ADMIN_DIR/.venv/bin/uvicorn main:app --host 0.0.0.0 --port $ADMIN_PORT > "$LOG_DIR/admin.log" 2>&1 &
cd "$PROJECT_ROOT"

echo "Starting Gateway..."
if [ ! -f "$GATEWAY_BIN" ]; then
    echo "⚠️  Gateway binary not found. Build it with: cargo build --release"
else
    # Check if port 8080 is in use and kill it
    lsof -ti :8080 | xargs kill -9 2>/dev/null || true
    $GATEWAY_BIN --gateway --env ci --port $GATEWAY_PORT > "$LOG_DIR/gateway.log" 2>&1 &
fi

echo "✅ Services Started!"
echo "   - Admin:   http://localhost:8001/admin"
echo "   - Gateway: http://localhost:8080/api/v1/health"
echo ""
echo "📝 To run UI PoC:"
echo "   cd admin"
echo "   pytest test_ui_poc.py"
echo ""
echo "Press Ctrl+C to stop..."
wait
