#!/bin/bash
set -e

# Configuration
GATEWAY_PORT=8080
LOG_FILE="/tmp/zerox_test.log"

echo "=========================================="
echo "   WebSocket Push Integration Test"
echo "=========================================="

cleanup() {
    echo ""
    echo "[Clean] Stopping services..."
    pkill -f "zero_x_infinity" || true
    if lsof -i:$GATEWAY_PORT >/dev/null; then
        lsof -ti:$GATEWAY_PORT | xargs kill -9 >/dev/null 2>&1 || true
    fi
}
trap cleanup EXIT

# 1. Check Dependencies
echo "[1] Checking Dependencies..."
if [ ! -f "scripts/check_deps.sh" ]; then
    echo "❌ scripts/check_deps.sh not found!"
    exit 1
fi
./scripts/check_deps.sh > /dev/null
echo "✅ Dependencies ready."

# 2. Start Service
echo "[2] Starting Gateway..."
nohup cargo run --release -- --gateway --port $GATEWAY_PORT > "$LOG_FILE" 2>&1 &
GATEWAY_PID=$!
echo "   Gateway PID: $GATEWAY_PID"
echo "   Logs: $LOG_FILE"

echo "   Waiting for port $GATEWAY_PORT..."
MAX_RETRIES=30
COUNT=0
READY=false
while [ $COUNT -lt $MAX_RETRIES ]; do
    if lsof -i:$GATEWAY_PORT >/dev/null; then
        # Check if HTTP endpoint is responsive
        if curl -s "http://localhost:$GATEWAY_PORT/api/v1/orders?user_id=1001" >/dev/null 2>&1; then
            READY=true
            break
        fi
    fi
    sleep 1
    ((COUNT++))
    echo -n "."
done
echo ""

if [ "$READY" = false ]; then
    echo "❌ Gateway failed to start."
    echo "--- Last 20 lines of log ---"
    tail -n 20 "$LOG_FILE"
    exit 1
fi
echo "✅ Gateway is UP and Listening."

# 3. Run Logic Test
echo "[3] Running Logic Test (Python)..."

# Ensure venv exists
if [ ! -d ".venv_test" ]; then
    echo "   Creating Python venv..."
    python3 -m venv .venv_test
    source .venv_test/bin/activate
    pip install --quiet aiohttp websockets
else
    source .venv_test/bin/activate
fi

python3 test_push_logic.py

TEST_EXIT_CODE=$?

if [ $TEST_EXIT_CODE -eq 0 ]; then
    echo ""
    echo "🎉 ALL TESTS PASSED"
else
    echo ""
    echo "❌ TEST FAILED"
    echo "--- Gateway Logs (Push Related) ---"
    grep -E "PUSH|WsService|Settlement" "$LOG_FILE" | tail -n 20 || echo "(No push logs found)"
fi

exit $TEST_EXIT_CODE
