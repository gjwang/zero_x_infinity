#!/usr/bin/env python3
"""
test_websocket_public_e2e.py - Verify WebSocket Public Trade Stream

Steps:
1. Connect to Gateway WebSocket
2. Subscribe to market.trade.BTC_USDT
3. Inject matching orders to trigger a trade
4. Validates receipt of public_trade message
"""

import asyncio
import websockets
import json
import sys
import os
import subprocess
import time

# Add scripts directory to path
SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, SCRIPT_DIR)

try:
    from inject_orders import submit_order
except ImportError:
    print("❌ Failed to import inject_orders.py")
    sys.exit(1)

GATEWAY_WS = "ws://localhost:8080/ws"
GATEWAY_URL = "http://localhost:8080"

# ANSI escape codes for colors
GREEN = "\033[92m"
RESET = "\033[0m"

async def test_websocket_stream():
    print(f"🔌 Connecting to {GATEWAY_WS}...")
    
    async with websockets.connect(f"{GATEWAY_WS}?user_id=0") as websocket:
        print(f"{GREEN}✅ Connected!{RESET}")

        # 1. Subscribe
        sub_msg = {
            "op": "subscribe",
            "args": ["market.trade.BTC_USDT"]
        }
        await websocket.send(json.dumps(sub_msg))
        print(f"📤 Sent subscription: {sub_msg}")

        # Wait for subscription confirmation (optional, currently strictly fire-and-forget in handler)
        # But our handler sends "subscribed" message back.
        response = await websocket.recv()
        print(f"📥 Received: {response}")
        resp_json = json.loads(response)
        
        # Expect welcome message first?
        # Handler: send "connected", then loop.
        if resp_json.get("type") == "connected":
            print("   (Welcome message received)")
            # Wait for next message (subscription confirm)
            response = await websocket.recv()
            print(f"📥 Received: {response}")
            resp_json = json.loads(response)

        if resp_json.get("type") == "subscribed":
             print("✅ Subscription confirmed!")
        else:
             print("⚠️  Warning: Did not receive subscription confirmation immediately")

        # 2. Trigger Trade via Order Injection
        print("\n🚀 Injecting orders to trigger trade...")
        
        # Sell Order (Maker)
        sell_order = {
            "order_id": int(time.time() * 1000),
            "user_id": 1002,
            "symbol": "BTC_USDT",
            "side": "sell",
            "price": "50000.00",
            "qty": "0.1"
        }
        
        # Buy Order (Taker) - Matches immediately
        buy_order = {
            "order_id": int(time.time() * 1000) + 1,
            "user_id": 1001,
            "symbol": "BTC_USDT",
            "side": "buy",
            "price": "50000.00",
            "qty": "0.1"
        }
        
        # Run sync injection in background logic or just call sequential
        # We need to do this while listening.
        
        loop = asyncio.get_event_loop()
        
        print("   -> Submitting Sell Order...")
        await loop.run_in_executor(None, lambda: submit_order(sell_order))
        
        print("   -> Submitting Buy Order (should match)...")
        await loop.run_in_executor(None, lambda: submit_order(buy_order))

        # 3. Wait for public_trade message
        print("\n👂 Listening for public_trade event (timeout 10s)...")
        try:
            start_time = time.time()
            while time.time() - start_time < 10:
                msg = await asyncio.wait_for(websocket.recv(), timeout=5.0)
                data = json.loads(msg)
                print(f"📥 Received: {data}")
                
                if data.get("type") == "public_trade":
                    print("\n✅ RECEIVED PUBLIC TRADE EVENT!")
                    print(json.dumps(data, indent=2))
                    
                    # Validate content
                    assert data["symbol"] == "BTC_USDT"
                    assert data["price"] == "50000.00"
                    assert data["qty"] == "0.100000"
                    # Quote qty 50000 * 0.1 = 5000.00
                    assert data["quote_qty"] == "5000.00"
                    assert "user_id" not in data
                    assert "order_id" not in data
                    
                    print("✅ Validation Passed: Correct Content & No Sensitive Data")
                    return True
                    
        except asyncio.TimeoutError:
            print("❌ Timeout waiting for public_trade event")
            return False

    return False

if __name__ == "__main__":
    try:
        if asyncio.run(test_websocket_stream()):
            sys.exit(0)
        else:
            sys.exit(1)
    except KeyboardInterrupt:
        sys.exit(130)
    except Exception as e:
        print(f"❌ Error: {e}")
        sys.exit(1)
