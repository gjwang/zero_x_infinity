#!/bin/bash
set -e

# 1. Reset DB (to ensure clean slate)
echo "🔄 Resetting Database..."
python3 scripts/db/manage_db.py init

# 2. Start Gateway in background
echo "🚀 Starting Gateway..."
mkdir -p logs
# Using release build if available is faster, but cargo run ensures latest code
cargo run --bin zero_x_infinity -- --gateway > logs/gateway_test.log 2>&1 &
GATEWAY_PID=$!
echo "   Gateway PID: $GATEWAY_PID"

# Wait for Gateway to be ready
echo "⏳ Waiting for Gateway..."
sleep 10 # Give it time to compile if needed and start

# 3. Run Python Test
echo "🧪 Running Integration Test..."
# Ensure PYTHONPATH allows finding lib
export PYTHONPATH=$PYTHONPATH:$(pwd)/scripts
python3 scripts/test_transfer.py

RET=$?

# 4. Cleanup
echo "🧹 Cleaning up..."
kill $GATEWAY_PID
wait $GATEWAY_PID || true

if [ $RET -eq 0 ]; then
    echo "✅ Verification SUCCESS"
else
    echo "❌ Verification FAILED"
    exit $RET
fi
