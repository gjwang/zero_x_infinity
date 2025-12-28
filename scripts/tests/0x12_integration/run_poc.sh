#!/bin/bash
set -e
PROJECT_ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
export RUST_LOG=info,gateway=debug,zero_x_infinity=debug
export DATABASE_URL="postgres://trading:trading@localhost:5433/exchange_info_db"
export TDENGINE_DSN="taos://root:taosdata@localhost:6041/trading"
export JWT_SECRET="dev_secret_key_for_testing_only_do_not_use_in_production"

# Start Gateway
echo "[POC] Starting Gateway (Production Mode)..."
cd "$PROJECT_ROOT"
# Use existing debug binary (proven by QA runs)
./target/debug/zero_x_infinity --gateway > logs/gateway_poc_0x12.log 2>&1 &
GATEWAY_PID=$!

echo "[POC] Waiting for Gateway (PID $GATEWAY_PID)..."
# Simple wait loop
for i in {1..30}; do
    if grep -q "Gateway listening" logs/gateway_poc_0x12.log; then
        echo "[POC] Gateway UP!"
        break
    fi
    sleep 1
done

# Run Test
echo "[POC] Executing verify_full_lifecycle.py..."
uv run python3 scripts/tests/0x12_integration/verify_full_lifecycle.py
TEST_EXIT_CODE=$?

# Cleanup
echo "[POC] Stopping Gateway..."
kill $GATEWAY_PID || true
wait $GATEWAY_PID || true

exit $TEST_EXIT_CODE
