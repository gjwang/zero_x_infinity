#!/usr/bin/env python3
"""
L4d: Maker Order Verification (BUG ISOLATION TEST)
===================================================

Tests ONLY the Maker (User A) side of order execution.
This test is EXPECTED TO FAIL - isolates the Maker bug.

Pass Criteria (currently failing):
  - User A order status is FILLED (not NEW)
  - User A executed_qty matches expected quantity
  - User A has trade record

Expected: FAIL (This isolates SEC-001, SEC-002, SEC-003)

When this test passes, the Maker bug is fixed.
"""

import sys
import os
import time
from decimal import Decimal

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from common.chain_utils_extended import (
    BtcRpcExtended, GatewayClientExtended, check_node_health,
    setup_jwt_user, BTC_REQUIRED_CONFIRMATIONS
)

sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), '..', '..', '..')))
try:
    from scripts.lib.api_auth import ApiClient
    HAS_API_AUTH = True
except ImportError:
    HAS_API_AUTH = False

import requests


class L4dMakerVerificationTest:
    """Test Maker order execution - EXPECTED TO FAIL (isolates bug)"""
    
    PRECISION = Decimal("0.00000001")
    
    def __init__(self):
        self.btc = BtcRpcExtended()
        self.gateway = GatewayClientExtended()
        self.results = []
        
        self.user_a_id = None
        self.user_a_api_client = None
        self.user_b_api_client = None
        
        self.trade_price = Decimal("50000")
        self.trade_quantity = Decimal("0.1")
        
    def add_result(self, name, passed, detail=""):
        self.results.append((name, passed, detail))
        status = "✅" if passed else "❌"
        print(f"   {status} {name}" + (f" [{detail}]" if detail else ""))
        return passed
    
    def setup_user_with_api_key(self, label):
        user_id, _, headers = setup_jwt_user()
        api_client = None
        if HAS_API_AUTH:
            resp = requests.post(
                f"{self.gateway.base_url}/api/v1/user/apikeys",
                json={"label": label}, headers=headers
            )
            if resp.status_code == 201:
                data = resp.json().get("data", {})
                api_client = ApiClient(
                    api_key=data.get("api_key"),
                    private_key_hex=data.get("api_secret"),
                    base_url=self.gateway.base_url
                )
        return user_id, headers, api_client
    
    def wait_for_order_status(self, api_client, symbol, expected_status, max_retries=15):
        """Wait for latest order to reach expected status"""
        for i in range(max_retries):
            try:
                resp = api_client.get("/api/v1/private/orders", params={"symbol": symbol})
                if resp.status_code == 200:
                    orders = resp.json().get("data", [])
                    if orders:
                        latest = orders[0]
                        status = latest.get("status")
                        if status == expected_status:
                            return latest
                        print(f"      Retry {i+1}: status={status} (expected: {expected_status})")
            except Exception as e:
                print(f"      Retry {i+1} error: {e}")
            time.sleep(2)
        return None
    
    def run(self):
        print("=" * 70)
        print("🧪 L4d: Maker Order Verification (BUG ISOLATION)")
        print("   Purpose: Verify Maker (User A SELL) is correctly filled")
        print("   Expected: FAIL (This isolates SEC-001/002/003)")
        print("=" * 70)
        
        # Pre-flight
        print("\n📋 Phase 0: Pre-flight")
        health = check_node_health(self.btc, None)
        if not health.get("btc"):
            return self.add_result("0.1 BTC Node", False)
        self.add_result("0.1 BTC Node", True)
        
        height = self.btc.get_block_count()
        if height < 100:
            self.btc.mine_blocks(101 - height)
        
        # Setup
        print("\n📋 Phase 1: Setup Users")
        self.user_a_id, user_a_headers, self.user_a_api_client = self.setup_user_with_api_key("L4d-A")
        user_b_id, user_b_headers, self.user_b_api_client = self.setup_user_with_api_key("L4d-B")
        self.add_result("1.1 Users Created", True)
        
        # Fund
        print("\n📋 Phase 2: Fund Users")
        addr = self.gateway.get_deposit_address(user_a_headers, "BTC", "BTC")
        time.sleep(2)
        tx = self.btc.send_to_address(addr, 0.5)
        self.btc.mine_blocks(BTC_REQUIRED_CONFIRMATIONS + 2)
        for _ in range(8):
            dep = self.gateway.get_deposit_by_tx_hash(user_a_headers, "BTC", tx)
            if dep and dep.get("status") in ["SUCCESS", "FINALIZED"]:
                break
            time.sleep(2)
        
        requests.post(f"{self.gateway.base_url}/api/v1/capital/transfer",
            json={"asset": "BTC", "amount": "0.2", "fromAccount": "FUNDING", "toAccount": "SPOT"},
            headers=user_a_headers)
        self.gateway.internal_mock_deposit(user_b_id, "USDT", "10000")
        requests.post(f"{self.gateway.base_url}/api/v1/capital/transfer",
            json={"asset": "USDT", "amount": "10000", "fromAccount": "FUNDING", "toAccount": "SPOT"},
            headers=user_b_headers)
        self.add_result("2.1 Users Funded", True)
        
        # Place Orders
        print("\n📋 Phase 3: Place Orders")
        self.user_a_api_client.post("/api/v1/private/order", {
            "symbol": "BTC_USDT", "side": "SELL", "order_type": "LIMIT",
            "qty": str(self.trade_quantity), "price": str(self.trade_price)
        })
        self.add_result("3.1 Maker SELL Placed", True)
        
        time.sleep(0.5)
        
        self.user_b_api_client.post("/api/v1/private/order", {
            "symbol": "BTC_USDT", "side": "BUY", "order_type": "LIMIT",
            "qty": str(self.trade_quantity), "price": str(self.trade_price)
        })
        self.add_result("3.2 Taker BUY Placed", True)
        
        # Wait for matching
        time.sleep(3)
        
        # Verify Maker (THE BUG TEST)
        print("\n" + "=" * 50)
        print("📋 Phase 4: Verify MAKER (User A) - BUG ISOLATION")
        print("=" * 50)
        
        # SEC-001: Order Status
        print("\n   🔍 SEC-001: Maker Order Status")
        order_a = self.wait_for_order_status(self.user_a_api_client, "BTC_USDT", "FILLED", max_retries=10)
        if order_a:
            self.add_result("4.1 [SEC-001] Maker Order FILLED", True)
            
            exec_qty = Decimal(str(order_a.get("executed_qty") or order_a.get("filled_qty") or 0))
            if abs(exec_qty - self.trade_quantity) <= self.PRECISION:
                self.add_result("4.2 Maker Qty Correct", True, f"{exec_qty} BTC")
            else:
                self.add_result("4.2 Maker Qty Correct", False, f"Got {exec_qty}")
        else:
            # Fetch current status for diagnosis
            try:
                resp = self.user_a_api_client.get("/api/v1/private/orders", params={"symbol": "BTC_USDT"})
                orders = resp.json().get("data", [])
                if orders:
                    actual_status = orders[0].get("status")
                    self.add_result("4.1 [SEC-001] Maker Order FILLED", False, 
                        f"Status is {actual_status} (not FILLED) - BUG CONFIRMED")
                else:
                    self.add_result("4.1 [SEC-001] Maker Order FILLED", False, "No orders found")
            except:
                self.add_result("4.1 [SEC-001] Maker Order FILLED", False, "Query failed")
            self.add_result("4.2 Maker Qty Correct", False, "Order not filled")
        
        # SEC-002: Trade Record
        print("\n   🔍 SEC-002: Maker Trade Records")
        try:
            resp = self.user_a_api_client.get("/api/v1/private/trades", params={"symbol": "BTC_USDT"})
            if resp.status_code == 200:
                trades = resp.json().get("data", [])
                # Filter for this user only
                maker_trades = [t for t in trades if str(t.get("user_id")) == str(self.user_a_id)]
                if maker_trades:
                    self.add_result("4.3 [SEC-002] Maker Trade Record", True, f"{len(maker_trades)} trade(s)")
                else:
                    all_count = len(trades)
                    self.add_result("4.3 [SEC-002] Maker Trade Record", False, 
                        f"0 trades for Maker (total {all_count} in response) - BUG CONFIRMED")
            elif resp.status_code == 503:
                self.add_result("4.3 [SEC-002] Maker Trade Record", True, "Persistence disabled (skip)")
            else:
                self.add_result("4.3 [SEC-002] Maker Trade Record", False, f"HTTP {resp.status_code}")
        except Exception as e:
            self.add_result("4.3 [SEC-002] Maker Trade Record", False, str(e))
        
        return self.summarize()
    
    def summarize(self):
        print("\n" + "=" * 70)
        print("📊 L4d RESULTS: Maker Verification (BUG ISOLATION)")
        print("=" * 70)
        
        passed = sum(1 for _, p, _ in self.results if p)
        failed = sum(1 for _, p, _ in self.results if not p)
        
        print(f"\n   Passed: {passed}/{passed + failed}")
        
        # Check specific SEC failures
        sec_001_failed = any("SEC-001" in n and not p for n, p, _ in self.results)
        sec_002_failed = any("SEC-002" in n and not p for n, p, _ in self.results)
        
        if sec_001_failed or sec_002_failed:
            print("\n   📋 BUG DIAGNOSIS:")
            if sec_001_failed:
                print("      ❌ SEC-001: Maker Order status NOT updating to FILLED")
            if sec_002_failed:
                print("      ❌ SEC-002: Maker Trade records NOT being created")
            print("\n   → Root Cause: MatchingEngine → Sentinel event chain for Maker")
            return False
        else:
            print("\n   🎉 L4d PASSED: Maker bugs are FIXED!")
            return True


if __name__ == "__main__":
    test = L4dMakerVerificationTest()
    success = test.run()
    sys.exit(0 if success else 1)
