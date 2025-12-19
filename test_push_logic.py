import asyncio
import aiohttp
import websockets
import json
import sys
import os

# Configuration
GATEWAY_PORT = 8080
GATEWAY_URL = f"http://localhost:{GATEWAY_PORT}"
WS_URL = f"ws://localhost:{GATEWAY_PORT}/ws?user_id=1001"
LOG_PREFIX = "[LOGIC_TEST]"

def log(msg):
    print(f"{LOG_PREFIX} {msg}", flush=True)

def error(msg):
    print(f"{LOG_PREFIX} ❌ {msg}", flush=True)
    sys.exit(1)

async def place_order(session, user_id, side, price, qty):
    order = {
        "user_id": user_id,
        "symbol": "BTC_USDT",
        "side": side,
        "order_type": "LIMIT",
        "price": str(price),
        "qty": str(qty)
    }
    
    headers = {"X-User-ID": str(user_id)}
    log(f"Placing {side} order for User {user_id}...")
    
    try:
        async with session.post(f"{GATEWAY_URL}/api/v1/create_order", json=order, headers=headers) as resp:
            try:
                data = await resp.json()
            except Exception:
                text = await resp.text()
                error(f"Failed to parse response: {text}, Status: {resp.status}")
                return

            if data.get("code") == 0:
                log(f"  Order placed: ID {data.get('data', {}).get('order_id')} (HTTP {resp.status})")
            else:
                error(f"Failed to place order: {json.dumps(data)}")
    except Exception as e:
        error(f"Request failed: {e}")

async def listen_ws(received_events):
    log(f"Connecting to WebSocket: {WS_URL}")
    try:
        async with websockets.connect(WS_URL) as ws:
            log("✅ WebSocket connected.")
            while True:
                try:
                    msg = await asyncio.wait_for(ws.recv(), timeout=10.0)
                    data = json.loads(msg)
                    msg_type = data.get("type")
                    log(f"📨 WS Received: {msg_type}")
                    log(f"   {json.dumps(data, ensure_ascii=False)}")
                    received_events.append(data)
                except asyncio.TimeoutError:
                    log("WS Listener timed out (no new messages).")
                    break
    except Exception as e:
        error(f"WebSocket error: {e}")

async def main():
    try:
        async with aiohttp.ClientSession() as session:
            # 1. Start WS Listener
            received_events = []
            ws_task = asyncio.create_task(listen_ws(received_events))
            await asyncio.sleep(2) # Wait for connection

            # 2. Execute Trades
            # Sell Order (Maker)
            await place_order(session, 1002, "SELL", 30000, 0.1)
            await asyncio.sleep(0.5)
            
            # Buy Order (Taker - should match)
            await place_order(session, 1001, "BUY", 30000, 0.1)

            # 3. Wait for events
            log("Waiting for push events...")
            await asyncio.sleep(3)
            
            if not ws_task.done():
                ws_task.cancel()
                try:
                    await ws_task
                except asyncio.CancelledError:
                    pass

            # 4. Verify
            log("========================================")
            log("Verification Results:")
            
            order_updates = [e for e in received_events if e.get("type") == "order_update"]
            trades = [e for e in received_events if e.get("type") == "trade"]
            balance_updates = [e for e in received_events if e.get("type") == "balance_update"]
            
            success = True
            
            if order_updates:
                log(f"✅ Found {len(order_updates)} OrderUpdate events")
            else:
                log("❌ No OrderUpdate events found")
                success = False

            if trades:
                log(f"✅ Found {len(trades)} Trade events")
            else:
                log("❌ No Trade events found")
                success = False
            
            if balance_updates:
                log(f"✅ Found {len(balance_updates)} BalanceUpdate events")
            else:
                log("❌ No BalanceUpdate events found")
                success = False
            
            log("========================================")
            
            if success:
                log("🎉 LOGIC TEST PASSED")
                sys.exit(0)
            else:
                log("🔥 LOGIC TEST FAILED")
                sys.exit(1)

    except Exception as e:
        error(f"Unexpected error: {e}")

if __name__ == "__main__":
    if not os.path.exists(".venv_test"):
         error("Virtual environment not found. Please run run_test.sh")
    
    asyncio.run(main())
