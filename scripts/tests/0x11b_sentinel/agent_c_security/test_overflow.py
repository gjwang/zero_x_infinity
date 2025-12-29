#!/usr/bin/env python3
"""
Agent C (Security Expert): Integer Overflow & Limits
Phase 0x11-b: Sentinel Hardening

Focus: Database Integer Limits & Overflow Protection
Mission: Verify system behavior when limits are exceeded.

Test Cases:
- TC-C11: Integer Overflow Injection (> INT64_MAX)
- TC-C12: The 9.22 ETH Barrier (INT64 limit for 18 decimals)
"""

import sys
import os
import time
from decimal import Decimal

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from common.chain_utils_extended import (
    GatewayClientExtended, print_test_header, print_test_result
)

def test_tc_c11_integer_overflow(gateway: GatewayClientExtended):
    """
    TC-C11: Integer Overflow Injection
    
    Attack: Submit amount > MAX_INT64 (9,223,372,036,854,775,807)
    Context: 
      - Rust i64 max: 9.22e18
      - Attacker sends: 10e18 (Atomic units) -> ~10 ETH
      
    Actually, let's try a properly huge number that definitely overflows 64-bit
    e.g. 2^70
    """
    print_test_header("TC-C11", "Integer Overflow Injection", "C")
    
    try:
        # 100 BTC = 10,000,000,000 sat (Safe)
        # 2^63-1 satoshis = 92 billion BTC (Safe usage, but max int)
        # Let's try to send 200 Billion BTC (which would be > i64 max satoshis)
        # 200,000,000,000 * 10^8 = 20,000,000,000,000,000,000 > 9,223,372,036,854,775,807
        
        huge_btc = "200000000000.0" 
        user_id = 666
        
        print(f"   🗡️  Attack: Deposit {huge_btc} BTC")
        
        # We expect this to fail gracefully (400 or 500 but handled), NOT crash the server
        try:
            resp = gateway.internal_mock_deposit(user_id, "BTC", huge_btc)
            if resp:
                 print(f"   ❌ System ACCEPTED overflow amount! (Dangerous)")
                 print_test_result(False, "Overflow accepted")
                 return False
        except Exception as e:
            print(f"   ✅ System rejected overflow: {e}")
            
        print_test_result(True, "Overflow rejected")
        return True

    except Exception as e:
        print(f"   ⚠️  {e}")
        return False

def test_tc_c12_large_amount_deposit(gateway: GatewayClientExtended):
    """
    TC-C12: Large Amount Deposit (User Requirement Validation)
    
    Requirement: Users must be able to deposit reasonable amounts (e.g., 100 ETH).
    The System MUST handle 18-decimal assets correctly without artificial i64 limits.
    
    Scenario: User deposits 100 ETH.
    Expected: SUCCESS.
    
    If this fails with "Number too large" or "500", it is a CRITICAL DEFECT.
    """
    print_test_header("TC-C12", "Large Amount Deposit (100 ETH)", "C")
    
    try:
        # Create a valid user first
        from common.chain_utils_extended import setup_jwt_user
        user_id, _, _ = setup_jwt_user()
        
        amount_eth = "100.0" 
        
        print(f"   🧪 Attempting to deposit {amount_eth} ETH...")
        print(f"   (Standard User Expectation: This MUST work)")
        
        try:
            resp = gateway.internal_mock_deposit(user_id, "ETH", amount_eth)
            if resp:
                print(f"   ✅ Deposit of {amount_eth} ETH Accepted")
                print_test_result(True, "100 ETH Deposit Succeeded")
                return True
            else:
                print(f"   ❌ Deposit Failed")
                print_test_result(False, "Deposit Failed")
                return False
                
        except Exception as e:
            print(f"   ❌ CRITICAL FAILURE: System rejected valid amount {amount_eth} ETH")
            print(f"   Error details: {e}")
            
            if "500" in str(e) or "Number too large" in str(e):
                print("   🚨 Root Cause: Backend uses i64/u64 for 18-decimal assets.")
                print("   🚨 Max i64 = ~9.22 ETH. This is a BLOCKING DEFECT.")
            
            print_test_result(False, "System cannot handle 100 ETH (Critical Defect)")
            return False

    except Exception as e:
        print(f"   ⚠️  {e}")
        return False

def main():
    print("=" * 70)
    print("🛡️  Agent C: Security Overflow Testing")
    print("   Verifying DB Integer Limits (9.22 ETH Case)")
    print("=" * 70)
    
    gateway = GatewayClientExtended()
    
    results = []
    results.append(("TC-C11: Integer Overflow", test_tc_c11_integer_overflow(gateway)))
    results.append(("TC-C12: 100 ETH Deposit", test_tc_c12_large_amount_deposit(gateway)))
    
    print("\n" + "=" * 70)
    print("📊 AGENT C RESULTS - Overflow")
    print("=" * 70)
    
    passed = 0
    for name, result in results:
        status = "✅ PASS" if result else "❌ FAIL"
        print(f"   {status}: {name}")
        if result: passed += 1
        
    print(f"\n   Total: {passed}/{len(results)} passed")
    return passed == len(results)

if __name__ == "__main__":
    success = main()
    sys.exit(0 if success else 1)
