#!/bin/bash
set -e
PROJECT_ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
source "$PROJECT_ROOT/scripts/lib/db_env.sh"
export RUST_LOG=info,gateway=debug,zero_x_infinity=debug
# DATABASE_URL and TDENGINE_DSN are exported by db_env.sh
export JWT_SECRET="dev_secret_key_for_testing_only_do_not_use_in_production"

# Source unified test utilities
source "$PROJECT_ROOT/scripts/lib/test_utils.sh"
# db_env.sh is sourced above for DATABASE_URL

LOG_DIR="${PROJECT_ROOT}/logs"

# Setup
ensure_log_dir "$LOG_DIR"

# Wait for DB
wait_for_postgres

# Cleanup old processes
cleanup_gateway_process

# Start Gateway
echo "[POC] Starting Gateway..."
echo "[POC] Using DB: $DATABASE_URL"
echo "[POC] Args: $GATEWAY_ARGS"

cd "$PROJECT_ROOT"
# Use existing debug binary (proven by QA runs)
GATEWAY_BIN="${GATEWAY_BINARY:-./target/debug/zero_x_infinity}"
# Redirect both stdout and stderr
$GATEWAY_BIN $GATEWAY_ARGS > "${LOG_DIR}/gateway_poc_0x12.log" 2>&1 &
GATEWAY_PID=$!

echo "[POC] Waiting for Gateway (PID $GATEWAY_PID)..."
wait_for_gateway 8080

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
