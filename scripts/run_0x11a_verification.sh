#!/bin/bash
# Phase 0x11-a Re-verification Runner
set -e

LOG_FILE="/Users/gjwang/eclipse-workspace/rust_source/zero_x_infinity_test/scripts/qa_verification_0x11a.log"
exec > >(tee -a "$LOG_FILE") 2>&1

echo "=== [$(date)] Starting Phase 0x11-a Verification ==="

# 1. Precise Cleanup
pkill "^zero_x_infinity$" || true
lsof -i :8080 -t | xargs kill -9 2>/dev/null || true

# 2. Environment Setup
export DATABASE_URL="postgresql://trading:trading123@127.0.0.1:5433/exchange_info_db"
export PG_USER="trading"
export PG_PASSWORD="trading123"
export PG_DB="exchange_info_db"
export PG_PORT=5433

echo "Restarting Docker (Postgres/Bitcoind)..."
docker rm -f postgres_dev bitcoind 2>/dev/null || true
docker run --name postgres_dev -e POSTGRES_USER=$PG_USER -e POSTGRES_PASSWORD=$PG_PASSWORD -e POSTGRES_DB=$PG_DB -p $PG_PORT:5432 -d postgres:latest
docker run -d --name bitcoind -p 18443:18443 -p 18332:18332 --rm ruimarinho/bitcoin-core:24 -regtest -rpcuser=admin -rpcpassword=admin -rpcallowip=0.0.0.0/0 -rpcbind=0.0.0.0 -fallbackfee=0.00001

echo "Waiting for Bitcoind..."
for i in {1..30}; do
    if curl -s --user admin:admin --data-binary '{"jsonrpc": "1.0", "id": "curltest", "method": "getblockchaininfo", "params": []}' -H 'content-type: text/plain;' http://127.0.0.1:18443/ > /dev/null; then
        echo "✅ Bitcoind ready"
        break
    fi
    sleep 2
done

echo "Initializing DB..."
./scripts/db/init.sh pg

# 3. Start Services
echo "Starting Gateway..."
./target/debug/zero_x_infinity --gateway -e dev > /tmp/gateway.log 2>&1 &
GW_PID=$!
echo "Gateway PID: $GW_PID"

echo "Starting Sentinel..."
./target/debug/zero_x_infinity --sentinel -e dev > /tmp/sentinel.log 2>&1 &
SENT_PID=$!
echo "Sentinel PID: $SENT_PID"

sleep 5

# 4. Run Test Suite
echo "Running Test Suite (scripts/tests/0x11_funding/run_tests.sh)..."
cd scripts/tests/0x11_funding/
bash run_tests.sh
EXIT_CODE=$?

# 5. Cleanup
echo "Cleaning up..."
kill "$GW_PID" 2>/dev/null || true
kill "$SENT_PID" 2>/dev/null || true

if [ $EXIT_CODE -eq 0 ]; then
    echo "✅ ALL TESTS PASSED"
else
    echo "❌ TEST SUITE FAILED (Code $EXIT_CODE)"
fi

exit $EXIT_CODE
