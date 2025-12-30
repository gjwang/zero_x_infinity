#!/usr/bin/env python3
"""
L4a: Two-User Isolation Test
============================

Extracted from L4_two_user_matching.py - Tests ONLY user creation and isolation.
This is the first step in decomposed L4 testing.

Pass Criteria:
  - User A and User B are created successfully
  - Both users start with 0 BTC balance
  - User A deposit does NOT affect User B balance

Expected: PASS (if this fails, fundamental user isolation is broken)
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


class L4aUserIsolationTest:
    """Test user creation and balance isolation"""
    
    PRECISION = Decimal("0.00000001")
    
    def __init__(self):
        self.btc = BtcRpcExtended()
        self.gateway = GatewayClientExtended()
        self.results = []
        
        self.user_a_id = None
        self.user_a_headers = None
        self.user_b_id = None
        self.user_b_headers = None
        
        self.deposit_amount = Decimal("0.5")  # Smaller amount for faster test
        
    def add_result(self, name, passed, detail=""):
        self.results.append((name, passed, detail))
        status = "✅" if passed else "❌"
        print(f"   {status} {name}" + (f" [{detail}]" if detail else ""))
        return passed
    
    def run(self):
        print("=" * 70)
        print("🧪 L4a: Two-User Isolation Test")
        print("   Purpose: Verify user independence before order matching")
        print("=" * 70)
        
        # Phase 0: Pre-flight
        print("\n📋 Phase 0: Pre-flight")
        health = check_node_health(self.btc, None)
        if not health.get("btc"):
            return self.add_result("0.1 BTC Node", False)
        self.add_result("0.1 BTC Node", True)
        
        # Ensure chain height
        height = self.btc.get_block_count()
        if height < 100:
            self.btc.mine_blocks(101 - height)
        
        # Phase 1: Create Two Users
        print("\n📋 Phase 1: Create Two Users")
        try:
            self.user_a_id, _, self.user_a_headers = setup_jwt_user()
            self.add_result("1.1 User A Created", True, f"ID: {self.user_a_id}")
        except Exception as e:
            return self.add_result("1.1 User A Created", False, str(e))
        
        try:
            self.user_b_id, _, self.user_b_headers = setup_jwt_user()
            self.add_result("1.2 User B Created", True, f"ID: {self.user_b_id}")
        except Exception as e:
            return self.add_result("1.2 User B Created", False, str(e))
        
        # Phase 2: Verify Initial Isolation
        print("\n📋 Phase 2: Initial Balance Isolation")
        balance_a = Decimal(str(self.gateway.get_balance(self.user_a_headers, "BTC") or 0))
        balance_b = Decimal(str(self.gateway.get_balance(self.user_b_headers, "BTC") or 0))
        
        if balance_a == 0:
            self.add_result("2.1 User A Initial 0 BTC", True)
        else:
            self.add_result("2.1 User A Initial 0 BTC", False, f"Got {balance_a}")
        
        if balance_b == 0:
            self.add_result("2.2 User B Initial 0 BTC", True)
        else:
            self.add_result("2.2 User B Initial 0 BTC", False, f"Got {balance_b}")
        
        # Phase 3: User A Deposit (verify B unaffected)
        print("\n📋 Phase 3: Deposit Isolation Test")
        try:
            addr = self.gateway.get_deposit_address(self.user_a_headers, "BTC", "BTC")
            self.add_result("3.1 User A Address", True)
        except Exception as e:
            return self.add_result("3.1 User A Address", False, str(e))
        
        # Wait for Sentinel to register address
        time.sleep(3)
        
        try:
            tx_hash = self.btc.send_to_address(addr, float(self.deposit_amount))
            self.add_result("3.2 Send BTC to A", True, f"TX: {tx_hash[:16]}...")
        except Exception as e:
            return self.add_result("3.2 Send BTC to A", False, str(e))
        
        # Mine and wait for finalization
        self.btc.mine_blocks(BTC_REQUIRED_CONFIRMATIONS + 2)
        
        # Poll for deposit
        for i in range(10):
            deposit = self.gateway.get_deposit_by_tx_hash(self.user_a_headers, "BTC", tx_hash)
            if deposit and deposit.get("status") in ["SUCCESS", "FINALIZED"]:
                break
            time.sleep(2)
        
        # Verify balances
        print("\n📋 Phase 4: Post-Deposit Verification")
        time.sleep(2)  # Allow balance propagation
        
        balance_a_after = Decimal(str(self.gateway.get_balance(self.user_a_headers, "BTC") or 0))
        balance_b_after = Decimal(str(self.gateway.get_balance(self.user_b_headers, "BTC") or 0))
        
        if balance_a_after >= self.deposit_amount - self.PRECISION:
            self.add_result("4.1 User A Has Deposit", True, f"{balance_a_after} BTC")
        else:
            self.add_result("4.1 User A Has Deposit", False, f"Expected {self.deposit_amount}, got {balance_a_after}")
        
        # CRITICAL: User B must still be 0
        if balance_b_after == 0:
            self.add_result("4.2 User B Still 0 BTC", True, "Isolation verified")
        else:
            self.add_result("4.2 User B Still 0 BTC", False, f"LEAK: B has {balance_b_after}")
        
        return self.summarize()
    
    def summarize(self):
        print("\n" + "=" * 70)
        print("📊 L4a RESULTS: User Isolation")
        print("=" * 70)
        
        passed = sum(1 for _, p, _ in self.results if p)
        failed = sum(1 for _, p, _ in self.results if not p)
        
        print(f"\n   Passed: {passed}/{passed + failed}")
        
        if failed == 0:
            print("\n   🎉 L4a PASSED: User isolation verified")
            print("   → Proceed to L4b (Order Placement)")
            return True
        else:
            print("\n   ❌ L4a FAILED: User isolation broken!")
            print("   → Fix basic user/deposit before testing matching")
            return False


if __name__ == "__main__":
    test = L4aUserIsolationTest()
    success = test.run()
    sys.exit(0 if success else 1)
