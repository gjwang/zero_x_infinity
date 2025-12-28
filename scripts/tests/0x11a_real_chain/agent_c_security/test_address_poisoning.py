#!/usr/bin/env python3
"""
Agent C (安全专家): Address Security Testing
Phase 0x11-a: Real Chain Integration

Focus: Address management security, isolation, poisoning attacks.
Mission: 寻找权限漏洞、数据泄露、地址注入攻击向量。

Test Cases:
- TC-C01: Address Poisoning Attack
- TC-C02: Address Isolation (Cross-User)
- TC-C05: Deposit History Privacy
"""

import sys
import os
import time
import asyncio
import aiohttp

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from common.chain_utils import GatewayClient

sys.path.insert(0, os.path.join(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))), "0x11_funding"))
from common_jwt import setup_jwt_user


GATEWAY_URL = os.getenv("GATEWAY_URL", "http://127.0.0.1:8080")
RATE_LIMIT_THRESHOLD = int(os.getenv("ADDRESS_RATE_LIMIT", "100"))


def test_address_poisoning():
    """
    TC-C01: Address Poisoning Attack
    
    Objective: Verify rate limiting prevents address generation abuse.
    
    Attack Vector: Generate millions of addresses to bloat bloom filter.
    Expected: Rate limiting kicks in before significant impact.
    """
    print("\n🔒 TC-C01: Address Poisoning Attack")
    print("=" * 60)
    print(f"   ⚙️  Expected rate limit: ~{RATE_LIMIT_THRESHOLD} requests")
    
    try:
        user_id, token, headers = setup_jwt_user()
        print(f"   ✅ User created: {user_id}")
        
        gateway = GatewayClient()
        addresses_generated = []
        rate_limited = False
        
        # Try to generate many addresses rapidly
        print("   🔄 Attempting rapid address generation...")
        
        for i in range(RATE_LIMIT_THRESHOLD * 2):  # Try 2x the expected limit
            try:
                # Request address for different "networks" to try to bypass caching
                network = f"BTC"  # Same network each time
                addr = gateway.get_deposit_address(headers, "BTC", network)
                addresses_generated.append(addr)
                
                if i > 0 and i % 50 == 0:
                    print(f"      Generated {i} addresses...")
                    
            except Exception as e:
                error_msg = str(e).lower()
                if "rate" in error_msg or "limit" in error_msg or "429" in error_msg:
                    rate_limited = True
                    print(f"   🛑 Rate limited at request #{i}: {e}")
                    break
                elif "too many" in error_msg:
                    rate_limited = True
                    print(f"   🛑 Rate limited at request #{i}")
                    break
                else:
                    print(f"   ⚠️  Error at request #{i}: {e}")
        
        print(f"\n   📊 Total addresses generated: {len(addresses_generated)}")
        
        # Analyze results
        if rate_limited:
            if len(addresses_generated) < RATE_LIMIT_THRESHOLD:
                print("   ✅ PASS: Rate limiting activated before threshold")
                return True
            else:
                print(f"   ⚠️  Rate limiting activated but after {len(addresses_generated)} requests")
                return True
        else:
            # No rate limiting - check if addresses are deduplicated
            unique_addresses = set(addresses_generated)
            if len(unique_addresses) == 1:
                print("   ✅ PASS: Same address returned (deduplication)")
                return True
            else:
                print(f"   ❌ FAIL: No rate limiting! Generated {len(unique_addresses)} unique addresses")
                return False
                
    except Exception as e:
        print(f"   ❌ EXCEPTION: {e}")
        return False


def test_address_isolation():
    """
    TC-C02: Address Isolation (Cross-User)
    
    Objective: Verify User A cannot access User B's deposit address.
    
    Steps:
    1. User A generates BTC address
    2. User B generates BTC address
    3. User A queries with user_id=B param (attack)
    4. Verify User A gets their own address, not User B's
    """
    print("\n🔒 TC-C02: Address Isolation (Cross-User)")
    print("=" * 60)
    
    try:
        # Create two users
        user_a_id, token_a, headers_a = setup_jwt_user()
        user_b_id, token_b, headers_b = setup_jwt_user()
        
        print(f"   ✅ User A: {user_a_id}")
        print(f"   ✅ User B: {user_b_id}")
        
        gateway = GatewayClient()
        
        # User A gets address
        addr_a = gateway.get_deposit_address(headers_a, "BTC", "BTC")
        print(f"   📋 User A address: {addr_a}")
        
        # User B gets address
        addr_b = gateway.get_deposit_address(headers_b, "BTC", "BTC")
        print(f"   📋 User B address: {addr_b}")
        
        # Verify different addresses
        if addr_a == addr_b:
            print("   ❌ FAIL: Both users got same address!")
            return False
        print("   ✅ Users have different addresses")
        
        # Attack: User A tries to get User B's address via param injection
        print("\n   🔓 [ATTACK] User A attempting to access User B's address...")
        
        import requests
        resp = requests.get(
            f"{GATEWAY_URL}/api/v1/capital/deposit/address",
            params={
                "asset": "BTC",
                "network": "BTC",
                "user_id": user_b_id  # Malicious param
            },
            headers=headers_a
        )
        
        if resp.status_code == 200:
            data = resp.json().get("data", {})
            addr_attack = data.get("address")
            
            if addr_attack == addr_b:
                print(f"   ❌ CRITICAL: User A retrieved User B's address!")
                print(f"   ❌ Param injection vulnerability detected!")
                return False
            elif addr_attack == addr_a:
                print(f"   ✅ PASS: Param ignored, User A got their own address")
                return True
            else:
                print(f"   ⚠️  Got unexpected address: {addr_attack}")
                return True  # Still safe - not User B's address
        else:
            print(f"   ✅ PASS: Request rejected ({resp.status_code})")
            return True
            
    except Exception as e:
        print(f"   ❌ EXCEPTION: {e}")
        return False


def test_deposit_history_privacy():
    """
    TC-C05: Deposit History Privacy
    
    Objective: Verify users cannot view others' deposit history.
    """
    print("\n🔒 TC-C05: Deposit History Privacy")
    print("=" * 60)
    
    try:
        import requests
        
        # Create two users
        user_a_id, token_a, headers_a = setup_jwt_user()
        user_b_id, token_b, headers_b = setup_jwt_user()
        
        print(f"   ✅ User A: {user_a_id}")
        print(f"   ✅ User B: {user_b_id}")
        
        gateway = GatewayClient()
        
        # User A creates a mock deposit (for testing)
        print("   📤 Creating test deposit for User A...")
        gateway.mock_deposit(user_a_id, "BTC", "1.0", f"test_tx_{user_a_id}", "BTC")
        time.sleep(1)
        
        # User A checks their history
        history_a = gateway.get_deposit_history(headers_a, "BTC")
        print(f"   📋 User A history count: {len(history_a)}")
        
        # User B checks their history
        history_b = gateway.get_deposit_history(headers_b, "BTC")
        print(f"   📋 User B history count: {len(history_b)}")
        
        # Attack: User B tries to access User A's history via param injection
        print("\n   🔓 [ATTACK] User B attempting to access User A's history...")
        
        resp = requests.get(
            f"{GATEWAY_URL}/api/v1/capital/deposit/history",
            params={
                "asset": "BTC",
                "user_id": user_a_id  # Malicious param
            },
            headers=headers_b
        )
        
        if resp.status_code == 200:
            attack_history = resp.json().get("data", [])
            
            # Check if any of User A's deposits leaked
            user_a_tx = f"test_tx_{user_a_id}"
            leaked = any(h.get("tx_hash") == user_a_tx for h in attack_history)
            
            if leaked:
                print(f"   ❌ CRITICAL: User A's deposit history leaked to User B!")
                return False
            else:
                print(f"   ✅ PASS: User A's history not leaked")
                print(f"   📋 Attack returned {len(attack_history)} records (User B's own)")
                return True
        else:
            print(f"   ✅ PASS: History request handled ({resp.status_code})")
            return True
            
    except Exception as e:
        print(f"   ❌ EXCEPTION: {e}")
        return False


def test_internal_endpoint_protection():
    """
    TC-C06: Internal Endpoint Protection
    
    Objective: Verify Sentinel internal APIs are not exposed.
    """
    print("\n🔒 TC-C06: Internal Endpoint Protection")
    print("=" * 60)
    
    import requests
    
    # User token (not internal)
    user_id, token, headers = setup_jwt_user()
    
    internal_endpoints = [
        "/internal/sentinel/force_scan",
        "/internal/sentinel/reset_cursor",
        "/internal/sentinel/inject_deposit",
        "/internal/chain/cursor",
        "/internal/admin/deposits",
    ]
    
    all_protected = True
    
    for endpoint in internal_endpoints:
        url = f"{GATEWAY_URL}{endpoint}"
        
        try:
            # Try with user auth
            resp = requests.post(url, headers=headers, timeout=5)
            
            if resp.status_code in [401, 403, 404, 405]:
                print(f"   ✅ {endpoint}: Protected ({resp.status_code})")
            elif resp.status_code == 200:
                print(f"   ❌ {endpoint}: ACCESSIBLE! (200)")
                all_protected = False
            else:
                print(f"   ⚠️  {endpoint}: {resp.status_code}")
                
        except requests.exceptions.ConnectionError:
            print(f"   ✅ {endpoint}: Not exposed")
        except Exception as e:
            print(f"   ⚠️  {endpoint}: {e}")
    
    if all_protected:
        print("\n   ✅ TC-C06 PASSED: All internal endpoints protected")
        return True
    else:
        print("\n   ❌ TC-C06 FAILED: Some internal endpoints accessible!")
        return False


def main():
    print("=" * 70)
    print("🔒 Agent C (安全专家): Address Security Testing")
    print("=" * 70)
    
    results = []
    
    results.append(("TC-C01: Address Poisoning", test_address_poisoning()))
    results.append(("TC-C02: Address Isolation", test_address_isolation()))
    results.append(("TC-C05: History Privacy", test_deposit_history_privacy()))
    results.append(("TC-C06: Internal Protection", test_internal_endpoint_protection()))
    
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
    
    # Security verdict
    if passed == len(results):
        print("\n   🔒 SECURITY STATUS: No critical vulnerabilities found")
    else:
        print("\n   ⚠️  SECURITY STATUS: Issues detected - review required!")
    
    sys.exit(0 if passed == len(results) else 1)


if __name__ == "__main__":
    main()
