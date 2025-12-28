#!/bin/bash
set -e

# =============================================================================
# Custom QA Runner for Phase 0x11-a (Deposit Lifecycle)
# Covers: Gateway + Sentinel + Bitcoind + Postgres + QA Scripts
# =============================================================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export PROJECT_ROOT="$SCRIPT_DIR"

# Source DB Environment (Handles DATABASE_URL/Ports from dev.yaml)
source "$PROJECT_ROOT/scripts/lib/db_env.sh"

echo "=== 0x11-a QA Environment Setup ==="
echo "DATABASE_URL: $DATABASE_URL"

# 1. Start Docker Dependencies (Postgres + Bitcoind)
echo "--------------------------------------------------------"
echo "Starting Docker Containers..."

# Postgres
docker rm -f postgres_dev 2>/dev/null || true
docker run --name postgres_dev -e POSTGRES_USER=$PG_USER -e POSTGRES_PASSWORD=$PG_PASSWORD -e POSTGRES_DB=$PG_DB -p $PG_PORT:5432 -d postgres:latest

# Bitcoind (Regtest)
docker rm -f bitcoind 2>/dev/null || true
docker run -d --name bitcoind -p 18443:18443 -p 18332:18332 --rm ruimarinho/bitcoin-core:24 -regtest -rpcuser=admin -rpcpassword=admin -printtoconsole

# Wait for readiness
echo "Waiting for services..."
for i in {1..30}; do
    if curl -s --user admin:admin --data-binary '{"jsonrpc": "1.0", "id": "curltest", "method": "getblockchaininfo", "params": []}' -H 'content-type: text/plain;' http://127.0.0.1:18443/ > /dev/null; then
        echo "✅ Bitcoind is ready!"
        break
    fi
    echo "Waiting for Bitcoind... ($i/30)"
    sleep 2
done

# Create Wallet for QA script
echo "Creating Wallet: sentinel_test..."
curl -s --user admin:admin --data-binary '{"jsonrpc": "1.0", "id": "createwallet", "method": "createwallet", "params": ["sentinel_test"]}' -H 'content-type: text/plain;' http://127.0.0.1:18443/ > /dev/null || echo "Wallet creation failed (exists?)"

# 2. Init DB & Migrations
echo "--------------------------------------------------------"
echo "Applying Migrations..."
"$PROJECT_ROOT/scripts/db/init.sh" pg

# 3. Start Gateway (Background)
echo "--------------------------------------------------------"
echo "Starting Gateway..."
cargo run --bin zero_x_infinity -- -e dev > /tmp/gateway.log 2>&1 &
GATEWAY_PID=$!
echo "Gateway PID: $GATEWAY_PID"

# Wait for Gateway
sleep 5

# 4. Start Sentinel (Background)
echo "--------------------------------------------------------"
echo "Starting Sentinel..."
# Sentinel needs the same DATABASE_URL
export DATABASE_URL
cargo run --bin zero_x_infinity -- --sentinel -e dev > /tmp/sentinel.log 2>&1 &
SENTINEL_PID=$!
echo "Sentinel PID: $SENTINEL_PID"

# Wait for Sentinel startup
sleep 5

# 5. Run QA Script
echo "--------------------------------------------------------"
echo "Running QA Script: test_deposit_lifecycle.py"
echo "--------------------------------------------------------"

# Ensure common library is discoverable
export PYTHONPATH="$PROJECT_ROOT/scripts:$PYTHONPATH"
export BTC_RPC_USER="admin"
export BTC_RPC_PASS="admin"
export BTC_WALLET="sentinel_test"

# Run Script
# Note: we need to allow failures to perform cleanup, so we modify set -e temporarily or use || true
set +e
uv run python3 scripts/tests/0x11a_real_chain/agent_b_core/test_deposit_lifecycle.py
EXIT_CODE=$?
set -e

# 6. Cleanup
echo "--------------------------------------------------------"
echo "Cleaning up..."
kill $GATEWAY_PID 2>/dev/null || true
kill $SENTINEL_PID 2>/dev/null || true

if [ $EXIT_CODE -eq 0 ]; then
    echo "✅ QA Verification SUCCESS"
else
    echo "❌ QA Verification FAILED (Exit Code: $EXIT_CODE)"
    echo "Gateway Log Tail:"
    tail -n 10 /tmp/gateway.log
    echo "Sentinel Log Tail:"
    tail -n 10 /tmp/sentinel.log
fi

exit $EXIT_CODE
