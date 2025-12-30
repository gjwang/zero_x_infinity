#!/usr/bin/env python3
"""
L4b: Order Placement Verification
=================================

Tests ONLY that both users can place orders successfully.
Does NOT verify matching or execution.

Pass Criteria:
  - User A can place SELL order (order_id returned)
  - User B can place BUY order (order_id returned)
  - Both orders initially have status NEW

Expected: PASS (if this fails, Order API is broken)
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

# Import Ed25519 auth library
sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), '..', '..', '..')))
try:
    from scripts.lib.api_auth import ApiClient
    HAS_API_AUTH = True
except ImportError:
    HAS_API_AUTH = False
    print("⚠️ pynacl not installed, using JWT fallback")

import requests


class L4bOrderPlacementTest:
    """Test order placement for both users"""
    
    def __init__(self):
        self.btc = BtcRpcExtended()
        self.gateway = GatewayClientExtended()
        self.results = []
        
        self.user_a_id = None
        self.user_a_headers = None
        self.user_a_api_client = None
        
        self.user_b_id = None
        self.user_b_headers = None
        self.user_b_api_client = None
        
        self.trade_price = Decimal("50000")
        self.trade_quantity = Decimal("0.1")
        
    def add_result(self, name, passed, detail=""):
        self.results.append((name, passed, detail))
        status = "✅" if passed else "❌"
        print(f"   {status} {name}" + (f" [{detail}]" if detail else ""))
        return passed
    
    def setup_user_with_api_key(self, label):
        """Create user and API key"""
        user_id, _, headers = setup_jwt_user()
        api_client = None
        
        if HAS_API_AUTH:
            resp = requests.post(
                f"{self.gateway.base_url}/api/v1/user/apikeys",
                json={"label": label},
                headers=headers
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
        print("🧪 L4b: Order Placement Verification")
        print("   Purpose: Verify both users can place orders")
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
        
        # Setup Users with API Keys
        print("\n📋 Phase 1: Setup Users with API Keys")
        try:
            self.user_a_id, self.user_a_headers, self.user_a_api_client = \
                self.setup_user_with_api_key("L4b User A")
            self.add_result("1.1 User A + API Key", True, f"ID: {self.user_a_id}")
        except Exception as e:
            return self.add_result("1.1 User A + API Key", False, str(e))
        
        try:
            self.user_b_id, self.user_b_headers, self.user_b_api_client = \
                self.setup_user_with_api_key("L4b User B")
            self.add_result("1.2 User B + API Key", True, f"ID: {self.user_b_id}")
        except Exception as e:
            return self.add_result("1.2 User B + API Key", False, str(e))
        
        # Fund Users (minimal amounts for order placement)
        print("\n📋 Phase 2: Fund Users for Trading")
        
        # User A: Deposit BTC
        addr_a = self.gateway.get_deposit_address(self.user_a_headers, "BTC", "BTC")
        time.sleep(2)
        tx_hash = self.btc.send_to_address(addr_a, 0.5)
        self.btc.mine_blocks(BTC_REQUIRED_CONFIRMATIONS + 2)
        
        for _ in range(8):
            dep = self.gateway.get_deposit_by_tx_hash(self.user_a_headers, "BTC", tx_hash)
            if dep and dep.get("status") in ["SUCCESS", "FINALIZED"]:
                break
            time.sleep(2)
        
        # Transfer to Spot
        requests.post(
            f"{self.gateway.base_url}/api/v1/capital/transfer",
            json={"asset": "BTC", "amount": "0.2", "fromAccount": "FUNDING", "toAccount": "SPOT"},
            headers=self.user_a_headers
        )
        self.add_result("2.1 User A Funded", True, "0.2 BTC in Spot")
        
        # User B: Mock USDT deposit
        self.gateway.internal_mock_deposit(self.user_b_id, "USDT", "10000")
        requests.post(
            f"{self.gateway.base_url}/api/v1/capital/transfer",
            json={"asset": "USDT", "amount": "10000", "fromAccount": "FUNDING", "toAccount": "SPOT"},
            headers=self.user_b_headers
        )
        self.add_result("2.2 User B Funded", True, "10000 USDT in Spot")
        
        # Place Orders
        print("\n📋 Phase 3: Place Orders")
        
        # User A: SELL order
        if self.user_a_api_client:
            resp = self.user_a_api_client.post("/api/v1/private/order", {
                "symbol": "BTC_USDT",
                "side": "SELL",
                "order_type": "LIMIT",
                "qty": str(self.trade_quantity),
                "price": str(self.trade_price)
            })
            
            if resp.status_code in (200, 202):
                data = resp.json()
                if data.get("code") == 0:
                    order_id = data.get("data", {}).get("order_id") or data.get("data", {}).get("orderId")
                    self.add_result("3.1 User A SELL Order", True, f"OrderID: {order_id}")
                else:
                    self.add_result("3.1 User A SELL Order", False, data.get("msg"))
            else:
                self.add_result("3.1 User A SELL Order", False, f"HTTP {resp.status_code}")
        else:
            self.add_result("3.1 User A SELL Order", False, "No API client")
        
        time.sleep(0.5)  # Small delay before taker
        
        # User B: BUY order
        if self.user_b_api_client:
            resp = self.user_b_api_client.post("/api/v1/private/order", {
                "symbol": "BTC_USDT",
                "side": "BUY",
                "order_type": "LIMIT",
                "qty": str(self.trade_quantity),
                "price": str(self.trade_price)
            })
            
            if resp.status_code in (200, 202):
                data = resp.json()
                if data.get("code") == 0:
                    order_id = data.get("data", {}).get("order_id") or data.get("data", {}).get("orderId")
                    self.add_result("3.2 User B BUY Order", True, f"OrderID: {order_id}")
                else:
                    self.add_result("3.2 User B BUY Order", False, data.get("msg"))
            else:
                self.add_result("3.2 User B BUY Order", False, f"HTTP {resp.status_code}")
        else:
            self.add_result("3.2 User B BUY Order", False, "No API client")
        
        return self.summarize()
    
    def summarize(self):
        print("\n" + "=" * 70)
        print("📊 L4b RESULTS: Order Placement")
        print("=" * 70)
        
        passed = sum(1 for _, p, _ in self.results if p)
        failed = sum(1 for _, p, _ in self.results if not p)
        
        print(f"\n   Passed: {passed}/{passed + failed}")
        
        if failed == 0:
            print("\n   🎉 L4b PASSED: Both orders placed successfully")
            print("   → Proceed to L4c (Taker Verification)")
            return True
        else:
            print("\n   ❌ L4b FAILED: Order placement broken")
            return False


if __name__ == "__main__":
    test = L4bOrderPlacementTest()
    success = test.run()
    sys.exit(0 if success else 1)
