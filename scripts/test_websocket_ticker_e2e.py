import asyncio
import websockets
import json
import requests
import time
import sys
import os

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

API_URL = "http://localhost:8080/api/v1/private/order"

async def test_ticker_e2e():
    uri = "ws://localhost:8080/ws"
    
    print(f"🔌 Connecting to {uri}...")
    try:
        async with websockets.connect(uri) as websocket:
            print(f"{GREEN}✅ Connected!{RESET}")

            # 1. Subscribe to Ticker
            sub_msg = {
                "op": "subscribe",
                "args": ["market.ticker.BTC_USDT"]
            }
            await websocket.send(json.dumps(sub_msg))
            print(f"📤 Sent subscription: {sub_msg}")

            # 2. Wait for subscription confirmation
            subscribed = False
            while not subscribed:
                response = await asyncio.wait_for(websocket.recv(), timeout=5.0)
                data = json.loads(response)
                print(f"📥 Received: {json.dumps(data)}")
                if data.get("type") == "subscribed":
                    subscribed = True
                    print(f"{GREEN}✅ Subscription confirmed!{RESET}")

            # 3. Inject Trade via REST API
            print(f"\n🚀 Injecting orders to trigger trade...")
            
            # Use timestamp to ensure unique orders
            ts = int(time.time() * 1000)
            
            # Sell Order (Maker)
            sell_order = {
                "symbol": "BTC_USDT",
                "side": "SELL",
                "order_type": "LIMIT",
                "price": "50000.00",
                "qty": "0.1",
                "time_in_force": "GTC"
            }
            
            client_seller = get_test_client(base_url="http://localhost:8080", user_id=1002)
            res = client_seller.post("/api/v1/private/order", sell_order)
            
            if res.status_code not in [200, 201, 202]:
                print(f"{RED}❌ Sell Order Failed (Status {res.status_code}): {res.text}{RESET}")
                return
            print("   -> Submitting Sell Order...")
            
            # Buy Order (Taker) - Should match
            buy_order = {
                "symbol": "BTC_USDT",
                "side": "BUY",
                "order_type": "LIMIT",
                "price": "50000.00",
                "qty": "0.1",
                "time_in_force": "GTC"
            }
            client_buyer = get_test_client(base_url="http://localhost:8080", user_id=1001)
            res = client_buyer.post("/api/v1/private/order", buy_order)
            
            if res.status_code not in [200, 201, 202]:
                print(f"{RED}❌ Buy Order Failed (Status {res.status_code}): {res.text}{RESET}")
                return
            print("   -> Submitting Buy Order (should match)...")

            # 4. Listen for Ticker event
            print(f"\n👂 Listening for ticker event (timeout 10s)...")
            try:
                start_time = time.time()
                while time.time() - start_time < 10:
                    response = await asyncio.wait_for(websocket.recv(), timeout=10.0)
                    data = json.loads(response)
                    
                    if data.get("type") == "ticker":
                        print(f"📥 Received: {data}")
                        print(f"\n{GREEN}✅ RECEIVED TICKER EVENT!{RESET}")
                        print(json.dumps(data, indent=2))
                        
                        # Validate Content
                        assert data["symbol"] == "BTC_USDT"
                        # Since we restarted, open price = trade price = 50000.00
                        # So change should be 0 unless multiple trades happened
                        print("Price Change:", data["price_change"])
                        assert data["last_price"] == "50000.00"
                        assert "user_id" not in data
                        
                        print(f"{GREEN}✅ Validation Passed{RESET}")
                        return

            except asyncio.TimeoutError:
                print(f"{RED}❌ Error: Timeout waiting for ticker event{RESET}")
                sys.exit(1)

    except Exception as e:
        print(f"{RED}❌ Error: {e}{RESET}")
        sys.exit(1)

if __name__ == "__main__":
    asyncio.run(test_ticker_e2e())
