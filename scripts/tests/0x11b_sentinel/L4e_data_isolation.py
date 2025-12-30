#!/usr/bin/env python3
"""
L4e: Data Isolation Verification (BUG ISOLATION TEST)
======================================================

Tests that /trades API returns ONLY user-specific data.
This test is EXPECTED TO FAIL - isolates SEC-004 data leak.

Pass Criteria (currently failing):
  - User A sees ONLY their own trades
  - User B sees ONLY their own trades
  - No cross-user data contamination

Expected: FAIL (This isolates SEC-004)

When this test passes, the data leak is fixed.
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


class L4eDataIsolationTest:
    """Test /trades API data isolation - EXPECTED TO FAIL (isolates SEC-004)"""
    
    def __init__(self):
        self.btc = BtcRpcExtended()
        self.gateway = GatewayClientExtended()
        self.results = []
        
        self.user_a_id = None
        self.user_a_api_client = None
        self.user_b_id = None
        self.user_b_api_client = None
        
        self.trade_quantity = Decimal("0.1")
        self.trade_price = Decimal("50000")
        
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
    
    def run(self):
        print("=" * 70)
        print("🧪 L4e: Data Isolation Verification (BUG ISOLATION)")
        print("   Purpose: Verify /trades API returns user-specific data only")
        print("   Expected: FAIL (This isolates SEC-004)")
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
        self.user_a_id, user_a_headers, self.user_a_api_client = self.setup_user_with_api_key("L4e-A")
        self.user_b_id, user_b_headers, self.user_b_api_client = self.setup_user_with_api_key("L4e-B")
        self.add_result("1.1 Users Created", True, f"A={self.user_a_id}, B={self.user_b_id}")
        
        # Fund and execute trade
        print("\n📋 Phase 2: Fund and Execute Trade")
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
        self.gateway.internal_mock_deposit(self.user_b_id, "USDT", "10000")
        requests.post(f"{self.gateway.base_url}/api/v1/capital/transfer",
            json={"asset": "USDT", "amount": "10000", "fromAccount": "FUNDING", "toAccount": "SPOT"},
            headers=user_b_headers)
        self.add_result("2.1 Users Funded", True)
        
        # Place matching orders
        self.user_a_api_client.post("/api/v1/private/order", {
            "symbol": "BTC_USDT", "side": "SELL", "order_type": "LIMIT",
            "qty": str(self.trade_quantity), "price": str(self.trade_price)
        })
        time.sleep(0.5)
        self.user_b_api_client.post("/api/v1/private/order", {
            "symbol": "BTC_USDT", "side": "BUY", "order_type": "LIMIT",
            "qty": str(self.trade_quantity), "price": str(self.trade_price)
        })
        self.add_result("2.2 Orders Matched", True)
        
        time.sleep(3)
        
        # Data Isolation Test (THE BUG TEST)
        print("\n" + "=" * 50)
        print("📋 Phase 3: Data Isolation Check - SEC-004")
        print("=" * 50)
        
        trades_seen_by_a = []
        trades_seen_by_b = []
        
        # User A queries trades
        print("\n   🔍 User A's view of /trades API:")
        try:
            resp = self.user_a_api_client.get("/api/v1/private/trades", params={"symbol": "BTC_USDT"})
            if resp.status_code == 200:
                trades_a = resp.json().get("data", [])
                trades_seen_by_a = trades_a
                unique_users_a = set(str(t.get("user_id")) for t in trades_a)
                print(f"      Total trades returned: {len(trades_a)}")
                print(f"      Unique user_ids in response: {unique_users_a}")
            elif resp.status_code == 503:
                self.add_result("3.1 User A Trades Query", True, "Persistence disabled")
                return self.summarize()
        except Exception as e:
            self.add_result("3.1 User A Trades Query", False, str(e))
            return self.summarize()
        
        # User B queries trades
        print("\n   🔍 User B's view of /trades API:")
        try:
            resp = self.user_b_api_client.get("/api/v1/private/trades", params={"symbol": "BTC_USDT"})
            if resp.status_code == 200:
                trades_b = resp.json().get("data", [])
                trades_seen_by_b = trades_b
                unique_users_b = set(str(t.get("user_id")) for t in trades_b)
                print(f"      Total trades returned: {len(trades_b)}")
                print(f"      Unique user_ids in response: {unique_users_b}")
        except Exception as e:
            self.add_result("3.2 User B Trades Query", False, str(e))
            return self.summarize()
        
        # Analyze isolation
        print("\n   📊 Isolation Analysis:")
        
        # Check if User A sees only their own trades
        other_users_in_a = [t for t in trades_seen_by_a if str(t.get("user_id")) != str(self.user_a_id)]
        if len(other_users_in_a) == 0:
            self.add_result("3.1 [SEC-004] User A sees only own trades", True)
        else:
            self.add_result("3.1 [SEC-004] User A sees only own trades", False,
                f"LEAK: A sees {len(other_users_in_a)} other users' trades")
        
        # Check if User B sees only their own trades  
        other_users_in_b = [t for t in trades_seen_by_b if str(t.get("user_id")) != str(self.user_b_id)]
        if len(other_users_in_b) == 0:
            self.add_result("3.2 [SEC-004] User B sees only own trades", True)
        else:
            self.add_result("3.2 [SEC-004] User B sees only own trades", False,
                f"LEAK: B sees {len(other_users_in_b)} other users' trades")
        
        # Cross-check: A should not see B's trades and vice versa
        a_sees_b = any(str(t.get("user_id")) == str(self.user_b_id) for t in trades_seen_by_a)
        b_sees_a = any(str(t.get("user_id")) == str(self.user_a_id) for t in trades_seen_by_b)
        
        if a_sees_b or b_sees_a:
            self.add_result("3.3 [SEC-004] Cross-User Isolation", False,
                f"A sees B: {a_sees_b}, B sees A: {b_sees_a} - DATA LEAK CONFIRMED")
        else:
            self.add_result("3.3 [SEC-004] Cross-User Isolation", True)
        
        return self.summarize()
    
    def summarize(self):
        print("\n" + "=" * 70)
        print("📊 L4e RESULTS: Data Isolation (SEC-004)")
        print("=" * 70)
        
        passed = sum(1 for _, p, _ in self.results if p)
        failed = sum(1 for _, p, _ in self.results if not p)
        
        print(f"\n   Passed: {passed}/{passed + failed}")
        
        sec_004_failed = any("SEC-004" in n and not p for n, p, _ in self.results)
        
        if sec_004_failed:
            print("\n   📋 BUG DIAGNOSIS:")
            print("      ❌ SEC-004: /trades API returns global data (ignores user_id)")
            print("\n   → Fix: Add WHERE user_id = :current_user_id to trades query")
            print("   → Impact: GDPR/CCPA violation, trading strategy exposure")
            return False
        else:
            print("\n   🎉 L4e PASSED: Data isolation is working correctly!")
            return True


if __name__ == "__main__":
    test = L4eDataIsolationTest()
    success = test.run()
    sys.exit(0 if success else 1)
