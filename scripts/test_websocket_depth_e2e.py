#!/usr/bin/env python3
"""
test_websocket_depth_e2e.py - Verify WebSocket Depth Stream
"""

import asyncio
import websockets
import json
import sys
import os
import requests
import time

# Add scripts directory to path for lib imports
SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, SCRIPT_DIR)

try:
    from lib.api_auth import get_test_client
except ImportError:
    print("❌ Failed to import lib.api_auth")
    sys.exit(1)

# Color codes
GREEN = "\033[92m"
RED = "\033[91m"
RESET = "\033[0m"

GATEWAY_WS = "ws://localhost:8080/ws"

async def test_websocket_stream():
    print(f"🔌 Connecting to {GATEWAY_WS}...")
    
    # 0 = Anonymous
    async with websockets.connect(f"{GATEWAY_WS}") as websocket:
        print("✅ Connected!")

        # 1. Subscribe
        sub_msg = {
            "op": "subscribe",
            "args": ["market.depth.BTC_USDT"]
        }
        await websocket.send(json.dumps(sub_msg))
        print(f"📤 Sent subscription: {sub_msg}")

        # Wait for connected/subscribed messages
        while True:
            response = await websocket.recv()
            print(f"📥 Received: {response}")
            resp_json = json.loads(response)
            if resp_json.get("type") == "subscribed":
                print("✅ Subscription confirmed!")
                break

        # 2. Inject Orders to change depth
        print("\n🚀 Injecting orders to change depth...")
        
        # Sell Order (Maker) - Adds to Asks
        sell_order = {
            "symbol": "BTC_USDT",
            "side": "SELL",
            "order_type": "LIMIT",
            "price": "50100.00",
            "qty": "0.5",
            "time_in_force": "GTC"
        }
        
        client_seller = get_test_client(base_url="http://localhost:8080", user_id=1002)
        res = client_seller.post("/api/v1/private/order", sell_order)
        
        if res.status_code not in [200, 201, 202]:
            print(f"{RED}❌ Sell Order Failed (Status {res.status_code}): {res.text}{RESET}")
            return False
        print("   -> Submitting Sell Order (New Ask Level)...")
        
        # Buy Order (Maker) - Adds to Bids
        buy_order = {
            "symbol": "BTC_USDT",
            "side": "BUY",
            "order_type": "LIMIT",
            "price": "49900.00",
            "qty": "0.3",
            "time_in_force": "GTC"
        }
        client_buyer = get_test_client(base_url="http://localhost:8080", user_id=1001)
        res = client_buyer.post("/api/v1/private/order", buy_order)
        
        if res.status_code not in [200, 201, 202]:
            print(f"{RED}❌ Buy Order Failed (Status {res.status_code}): {res.text}{RESET}")
            return False
        print("   -> Submitting Buy Order (New Bid Level)...")

        # 3. Wait for depthUpdate message
        print("\n👂 Listening for depthUpdate event (timeout 10s)...")
        try:
            start_time = time.time()
            found_bid = False
            found_ask = False
            
            while time.time() - start_time < 10:
                msg = await asyncio.wait_for(websocket.recv(), timeout=5.0)
                data = json.loads(msg)
                
                if data.get("e") == "depthUpdate":
                    print(f"📥 Received Depth Update: {json.dumps(data, indent=2)}")
                    
                    # Validate content
                    assert data["s"] == "BTC_USDT"
                    
                    # Check if our orders are visible
                    # Bids format: [["price", "qty"], ...]
                    # We look for price="49900.00"
                    for bid in data["b"]:
                        if bid[0] == "49900.00":
                            print(f"{GREEN}✅ Found Bid: {bid}{RESET}")
                            found_bid = True
                            
                    for ask in data["a"]:
                        if ask[0] == "50100.00":
                            print(f"{GREEN}✅ Found Ask: {ask}{RESET}")
                            found_ask = True
                    
                    if found_bid and found_ask:
                        print("\n✅ Verified Bid and Ask levels in depth update!")
                        return True
                    
        except asyncio.TimeoutError:
            print(f"{RED}❌ Timeout waiting for depthUpdate event{RESET}")
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
