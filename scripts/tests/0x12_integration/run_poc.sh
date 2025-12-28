#!/bin/bash
set -e
PROJECT_ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
source "$PROJECT_ROOT/scripts/lib/db_env.sh"
export RUST_LOG=info,gateway=debug,zero_x_infinity=debug
# DATABASE_URL and TDENGINE_DSN are exported by db_env.sh
export JWT_SECRET="dev_secret_key_for_testing_only_do_not_use_in_production"

# Wait for DB
echo "[POC] Waiting for PostgreSQL..."
for i in {1..30}; do
    if pg_check; then
        echo "[POC] PostgreSQL is ready"
        break
    fi
    sleep 2
done

# Start Gateway
echo "[POC] Starting Gateway (Production Mode)..."
echo "[POC] Using DB: $DATABASE_URL"

GATEWAY_ARGS="--gateway"
if [ "$CI" = "true" ]; then
    GATEWAY_ARGS="$GATEWAY_ARGS --env ci"
fi

cd "$PROJECT_ROOT"
# Use existing debug binary (proven by QA runs)
GATEWAY_BIN="${GATEWAY_BINARY:-./target/debug/zero_x_infinity}"
$GATEWAY_BIN $GATEWAY_ARGS > logs/gateway_poc_0x12.log 2>&1 &
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
# Run Test 1: Address Validation (Real Chain Formats) - P0 Fix Verification
echo "[POC] Executing test_address_validation.py..."
uv run python3 scripts/tests/0x12_integration/test_address_validation.py
EXIT_CODE_1=$?

# Run Test 2: Full Lifecycle (Deposit -> Transfer -> Trade)
echo "[POC] Executing verify_full_lifecycle.py..."
uv run python3 scripts/tests/0x12_integration/verify_full_lifecycle.py
EXIT_CODE_2=$?

# Combine Exit Codes
if [ $EXIT_CODE_1 -ne 0 ] || [ $EXIT_CODE_2 -ne 0 ]; then
    TEST_EXIT_CODE=1
else
    TEST_EXIT_CODE=0
fi

# Cleanup
echo "[POC] Stopping Gateway..."
kill $GATEWAY_PID || true
wait $GATEWAY_PID || true

exit $TEST_EXIT_CODE
