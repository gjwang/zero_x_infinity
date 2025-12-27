#!/usr/bin/env python3
"""
Zero X Infinity Phase 0x10.5 Backend Gap E2E Test
"""

import sys
import json
import time
import argparse
import threading
import requests
import websocket
from dataclasses import dataclass
from typing import List, Any

# =============================================================================
# Configuration
# =============================================================================

@dataclass
class TestResult:
    name: str
    passed: bool
    message: str
    duration_ms: float

class TestRunner:
    def __init__(self, base_url: str = None, ws_url: str = None, verbose: bool = False):
        self.base_url = base_url or "http://localhost:8080"
        # Derive WS URL from HTTP URL if not provided
        if not ws_url:
            self.ws_url = self.base_url.replace("http", "ws") + "/ws?user_id=0"
        else:
            self.ws_url = ws_url
            
        self.verbose = verbose
        self.results: List[TestResult] = []
        
    def log(self, msg: str):
        if self.verbose:
            print(f"  {msg}")
            
    def run_test(self, name: str, test_func) -> TestResult:
        start = time.time()
        try:
            test_func()
            duration = (time.time() - start) * 1000
            result = TestResult(name, True, "OK", duration)
        except AssertionError as e:
            duration = (time.time() - start) * 1000
            result = TestResult(name, False, str(e), duration)
        except Exception as e:
            duration = (time.time() - start) * 1000
            result = TestResult(name, False, f"Error: {e}", duration)
        
        self.results.append(result)
        status = "✅" if result.passed else "❌"
        print(f"{status} {name} ({result.duration_ms:.1f}ms)")
        if not result.passed:
            print(f"   └─ {result.message}")
        return result

    # =========================================================================
    # REST API Tests
    # =========================================================================

    def test_public_trades(self):
        """GET /api/v1/public/trades"""
        url = f"{self.base_url}/api/v1/public/trades"
        params = {"symbol": "BTC_USDT", "limit": 5}
        
        self.log(f"Fetching {url}")
        resp = requests.get(url, params=params, timeout=5)
        
        # Expect 200 OK
        assert resp.status_code == 200, f"Expected 200, got {resp.status_code}"
        
        data = resp.json()
        assert data["code"] == 0, f"Expected code=0, got {data['code']}"
        
        trades = data.get("data", [])
        assert isinstance(trades, list), "Expected data to be a list"
        
        # Even if empty, the endpoint structure is verified.
        # If we had mocking, we could assert non-empty.
        if trades:
            t = trades[0]
            assert "price" in t
            assert "qty" in t
            assert "time" in t
            assert "is_buyer_maker" in t

    # =========================================================================
    # WebSocket Tests
    # =========================================================================
    
    def _ws_connect_and_wait(self, topic: str, expected_event: str, timeout: float = 5.0):
        """Helper to connect, subscribe, and wait for a specific event"""
        received = []
        error = None
        event_found = threading.Event()
        
        def on_message(ws, message):
            try:
                msg = json.loads(message)
                received.append(msg)
                # Check for event type
                if msg.get("e") == expected_event:
                    event_found.set()
            except:
                pass

        def on_error(ws, err):
            nonlocal error
            error = err
            event_found.set()

        def on_open(ws):
            # Subscribe
            sub = {
                "op": "subscribe",
                "args": [topic]
            }
            ws.send(json.dumps(sub))

        ws = websocket.WebSocketApp(
            self.ws_url,
            on_message=on_message,
            on_error=on_error,
            on_open=on_open
        )
        
        # Run in thread
        wst = threading.Thread(target=ws.run_forever)
        wst.daemon = True
        wst.start()
        
        # Wait
        event_found.wait(timeout)
        ws.close()
        
        if error:
            raise Exception(f"WebSocket error: {error}")
            
        # Analyze results
        found = any(msg.get("e") == expected_event for msg in received)
        return found, received

    def test_ws_ticker(self):
        """WebSocket market.ticker subscription"""
        # We expect '24hTicker' event
        found, msgs = self._ws_connect_and_wait("market.ticker.BTC_USDT", "24hTicker", timeout=3.0)
        
        if not found:
            # Check if we at least got a subscription success or if connection failed
            # For now, just assert not found
            assert False, f"Did not receive '24hTicker' event within timeout. Msgs: {len(msgs)}"

    def test_ws_depth(self):
        """WebSocket market.depth subscription"""
        # We expect 'depthUpdate' or partial depth
        found, msgs = self._ws_connect_and_wait("market.depth.BTC_USDT", "depthUpdate", timeout=3.0)
        
        if not found:
            assert False, f"Did not receive 'depthUpdate' event within timeout. Msgs: {len(msgs)}"

    def test_ws_trade(self):
        """WebSocket market.trade subscription"""
        # We expect 'trade' event
        # Note: This might timeout if no trades happen, but we want to verify the CHANNEL exists
        # Ideally we would trigger a trade, but for Gap Analysis, 
        # failure to subscribe or receive anything is the signal.
        found, msgs = self._ws_connect_and_wait("market.trade.BTC_USDT", "trade", timeout=3.0)
        
        # It's hard to force a trade in this isolated script without auth/orders.
        # However, if the subscription fails (channel invalid), that's a legit failure.
        # If the channel is unimplemented, we might get error or nothing.
        
        # Strict check: If we don't implement it, we accept failure.
        if not found:
             assert False, f"Did not receive 'trade' event. (Note: requires active trading or mock)"


    # =========================================================================
    # Run All
    # =========================================================================

    def run_all(self):
        print("\n" + "=" * 60)
        print("Phase 0x10.5 Backend Gap Analysis")
        print(f"Gateway: {self.base_url}")
        print(f"WS URL:  {self.ws_url}")
        print("=" * 60 + "\n")

        # 1. Public Trades (REST)
        self.run_test("REST Public Trades", self.test_public_trades)
        
        # 2. WebSocket Ticker
        self.run_test("WS Ticker", self.test_ws_ticker)
        
        # 3. WebSocket Depth
        self.run_test("WS Depth", self.test_ws_depth)
        
        # 4. WebSocket Trade
        self.run_test("WS Trade", self.test_ws_trade)
        
        # Summary
        passed = sum(1 for r in self.results if r.passed)
        failed = sum(1 for r in self.results if not r.passed)
        
        print("\n" + "=" * 60)
        print(f"Results: {passed} passed, {failed} failed")
        print("=" * 60)
        
        return failed

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--ci", action="store_true", help="Exit code 1 on failure")
    parser.add_argument("--url", default=None)
    args = parser.parse_args()
    
    runner = TestRunner(base_url=args.url)
    failed = runner.run_all()
    
    if args.ci and failed > 0:
        sys.exit(1)

if __name__ == "__main__":
    main()
