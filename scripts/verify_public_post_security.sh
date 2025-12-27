#!/bin/bash
set -e

# Cleanup function
cleanup() {
    echo "Stopping Gateway..."
    pkill -f "zero_x_infinity --gateway" || true
}
trap cleanup EXIT

# 1. Start Gateway
echo "Building and Starting Gateway..."
cargo build --release --bin zero_x_infinity --quiet
./target/release/zero_x_infinity --gateway --port 8080 > /tmp/gateway_public.log 2>&1 &
GW_PID=$!
sleep 5

# 2. Test Public REST API (Anonymous - No Auth Header/Param)
echo "Testing Public REST API..."
# We expect 200 OK even if empty data, just checking access
HTTP_CODE=$(curl -s -o /dev/null -w "%{http_code}" "http://localhost:8080/api/v1/public/trades?symbol=BTC_USDT")
if [ "$HTTP_CODE" == "200" ]; then
    echo "✅ REST API Public Access: OK (200)"
else
    echo "❌ REST API Public Access: FAILED ($HTTP_CODE)"
    cat /tmp/gateway_public.log
    exit 1
fi

# 3. Test Public WebSocket (Anonymous - No User ID)
echo "Testing Public WebSocket (Anonymous)..."

cat <<EOF > /tmp/test_ws_anon.py
import asyncio
import websockets
import sys

async def test():
    uri = "ws://localhost:8080/ws"
    try:
        async with websockets.connect(uri) as websocket:
            print("Connected successfully")
            # Send subscribe command
            await websocket.send('{"op": "subscribe", "args": ["market.trade.BTC_USDT"]}')
            response = await websocket.recv()
            print(f"Response: {response}")
            return True
    except Exception as e:
        print(f"Connection failed: {e}")
        return False

if __name__ == "__main__":
    if asyncio.run(test()):
        sys.exit(0)
    else:
        sys.exit(1)
EOF

if python3 /tmp/test_ws_anon.py; then
    echo "✅ WebSocket Public Access: OK"
else
    echo "❌ WebSocket Public Access: FAILED"
    cat /tmp/gateway_public.log
    exit 1
fi

echo "✅ All Regression Tests Passed: Security Fix did NOT break Public Access."
