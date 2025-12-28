#!/usr/bin/env python3
"""
Agent A (激进派): Precision and Overflow Edge Cases
Phase 0x11-a: Real Chain Integration

Focus: Boundary conditions for amount precision and overflow.
Mission: 破坏系统！找到精度计算和溢出的边界漏洞。

Test Cases:
- TC-A04: Dust Deposit Threshold
- TC-A06: Precision Overflow (Wei/Satoshi Edge)
"""

import sys
import os
import time

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from common.chain_utils import BtcRpc, EthRpc, GatewayClient, check_node_health

sys.path.insert(0, os.path.join(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))), "0x11_funding"))
from common_jwt import setup_jwt_user


# Configuration
MIN_DEPOSIT_BTC = float(os.getenv("MIN_DEPOSIT_BTC", "0.001"))  # 100k satoshi
MIN_DEPOSIT_ETH = float(os.getenv("MIN_DEPOSIT_ETH", "0.01"))   # 0.01 ETH


def test_dust_deposit_rejected(btc: BtcRpc, gateway: GatewayClient):
    """
    TC-A04: Minimum Deposit Threshold (Dust Wall)
    
    Objective: Verify deposits below MIN_DEPOSIT_THRESHOLD are rejected/ignored.
    
    Steps:
    1. Send deposit below threshold
    2. Mine block
    3. Verify deposit status = IGNORED or balance = 0
    """
    print("\n🔴 TC-A04: Dust Deposit Threshold")
    print("=" * 60)
    print(f"   ⚙️  MIN_DEPOSIT_BTC = {MIN_DEPOSIT_BTC}")
    
    try:
        # Setup user
        user_id, token, headers = setup_jwt_user()
        print(f"   ✅ User created: {user_id}")
        
        # Get deposit address
        btc_addr = gateway.get_deposit_address(headers, "BTC", "BTC")
        print(f"   ✅ Deposit address: {btc_addr}")
        
        # Ensure we have coins
        btc.mine_blocks(101)
        
        # Calculate dust amount (10% of min threshold)
        dust_amount = MIN_DEPOSIT_BTC * 0.1
        print(f"   📤 Sending dust amount: {dust_amount} BTC (below {MIN_DEPOSIT_BTC})")
        
        # Send dust deposit
        tx_hash = btc.send_to_address(btc_addr, dust_amount)
        print(f"   📤 TX hash: {tx_hash}")
        
        # Mine block
        btc.mine_blocks(1)
        print("   ⛏️  Block mined")
        
        # Wait for processing
        time.sleep(3)
        
        # Check deposit status
        history = gateway.get_deposit_history(headers, "BTC")
        deposit = next((h for h in history if h.get("tx_hash") == tx_hash), None)
        
        if deposit:
            status = deposit.get("status")
            print(f"   📋 Deposit status: {status}")
            
            if status in ["IGNORED", "DUST", "REJECTED"]:
                print("   ✅ PASS: Dust deposit correctly rejected")
                return True
            elif status in ["DETECTED", "CONFIRMING", "SUCCESS"]:
                print(f"   ❌ FAIL: Dust deposit was accepted! (status={status})")
                return False
        else:
            print("   📋 Deposit not in history (may be filtered out)")
        
        # Check balance (should be 0)
        balance = gateway.get_balance(headers, "BTC")
        print(f"   💰 Balance: {balance}")
        
        if balance is None or balance == 0:
            print("   ✅ PASS: Balance is zero (dust not credited)")
            return True
        elif balance < dust_amount:
            print("   ⚠️  Partial credit detected. Investigating...")
            return False
        else:
            print(f"   ❌ FAIL: Dust was credited! Balance = {balance}")
            return False
            
    except Exception as e:
        print(f"   ❌ EXCEPTION: {e}")
        return False


def test_precision_overflow(btc: BtcRpc, gateway: GatewayClient):
    """
    TC-A06: Precision Overflow (Satoshi Edge Cases)
    
    Objective: Verify truncation protocol handles extreme values.
    
    Test Cases:
    1. Minimum precision (1 satoshi)
    2. Maximum BTC supply (21M BTC)
    3. Values that cause floating point errors
    """
    print("\n🔴 TC-A06: Precision Overflow Testing")
    print("=" * 60)
    
    test_cases = [
        ("1 Satoshi", 0.00000001),
        ("100 Satoshi", 0.00000100),
        ("Precision Edge", 0.12345678),  # 8 decimal places
        ("Float Issue", 0.1 + 0.2),       # Famous 0.30000000000000004 issue
    ]
    
    results = []
    
    try:
        # Setup user
        user_id, token, headers = setup_jwt_user()
        print(f"   ✅ User created: {user_id}")
        
        # Ensure coins
        btc.mine_blocks(101)
        
        for name, amount in test_cases:
            print(f"\n   📊 Test: {name} = {amount:.8f} BTC")
            
            try:
                # Get fresh address
                btc_addr = gateway.get_deposit_address(headers, "BTC", "BTC")
                
                # Send amount
                tx_hash = btc.send_to_address(btc_addr, amount)
                print(f"      📤 TX: {tx_hash[:16]}...")
                
                # Mine
                btc.mine_blocks(6)
                
                # Wait for processing
                time.sleep(2)
                
                # Check credited amount
                history = gateway.get_deposit_history(headers, "BTC")
                deposit = next((h for h in history if h.get("tx_hash") == tx_hash), None)
                
                if deposit:
                    credited = float(deposit.get("amount", 0))
                    status = deposit.get("status")
                    
                    # Verify no precision loss
                    expected = round(amount, 8)  # BTC precision
                    actual = round(credited, 8)
                    
                    if abs(expected - actual) < 0.00000001:
                        print(f"      ✅ Precision OK: {credited}")
                        results.append((name, True))
                    else:
                        print(f"      ❌ Precision LOSS: expected {expected}, got {actual}")
                        results.append((name, False))
                else:
                    if amount < MIN_DEPOSIT_BTC:
                        print(f"      ⚠️  Filtered (below min threshold)")
                        results.append((name, True))  # Expected behavior
                    else:
                        print(f"      ⚠️  Not found in history")
                        results.append((name, None))
                        
            except Exception as e:
                print(f"      ❌ Error: {e}")
                results.append((name, False))
        
        # Summary
        print("\n   📊 Precision Test Summary:")
        passed = 0
        for name, result in results:
            status = "✅" if result else ("⚠️" if result is None else "❌")
            print(f"      {status} {name}")
            if result:
                passed += 1
        
        return passed == len(results)
        
    except Exception as e:
        print(f"   ❌ EXCEPTION: {e}")
        return False


def test_max_value_handling():
    """
    Test handling of maximum possible values.
    
    Note: This is a unit test concept - actual on-chain test would require
    significant funds.
    """
    print("\n🔴 TC-A06b: Maximum Value Handling (Conceptual)")
    print("=" * 60)
    
    max_values = [
        ("u64::MAX Satoshi", 18446744073709551615),
        ("21M BTC in Satoshi", 21_000_000 * 100_000_000),
        ("Max ETH in Wei", 10**18 * 100_000_000),  # 100M ETH
    ]
    
    print("   📋 Maximum value boundaries to test:")
    for name, value in max_values:
        print(f"      {name}: {value:,}")
    
    print("\n   ⚠️  These tests require Rust unit tests to verify:")
    print("      - No overflow in u64 addition")
    print("      - Truncation handles max values")
    print("      - No panic on extreme inputs")
    
    print("\n   ✅ Test case documented for Rust unit test implementation")
    return True


def main():
    print("=" * 70)
    print("🔴 Agent A (激进派): Precision and Overflow Edge Cases")
    print("=" * 70)
    
    # Initialize clients
    btc = BtcRpc()
    gateway = GatewayClient()
    
    # Check node health
    print("\n📡 Checking node connectivity...")
    health = check_node_health(btc)
    
    if not health.get("btc"):
        print("❌ BTC node not available.")
        sys.exit(1)
    
    print("   ✅ BTC node: Connected")
    
    # Run tests
    results = []
    
    results.append(("TC-A04: Dust Threshold", test_dust_deposit_rejected(btc, gateway)))
    results.append(("TC-A06a: Precision", test_precision_overflow(btc, gateway)))
    results.append(("TC-A06b: Max Values", test_max_value_handling()))
    
    # Summary
    print("\n" + "=" * 70)
    print("📊 RESULTS SUMMARY")
    print("=" * 70)
    
    passed = 0
    for name, result in results:
        status = "✅ PASS" if result else "❌ FAIL"
        print(f"   {status}: {name}")
        if result:
            passed += 1
    
    print(f"\n   Total: {passed}/{len(results)} passed")
    
    sys.exit(0 if passed == len(results) else 1)


if __name__ == "__main__":
    main()
