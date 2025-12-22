#!/usr/bin/env python3
"""
API Authentication Test Script

Tests Ed25519 signature-based authentication for the 0xInfinity API.
Uses the test API key from fixtures/seed_data.sql.

This script imports the reusable auth library from lib/auth.py.
"""

import sys
import os
import time

# Add scripts directory to path for lib imports
SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, SCRIPT_DIR)

try:
    import requests
    from nacl.signing import SigningKey
except ImportError:
    print("Error: Missing dependencies. Run: pip install pynacl requests")
    sys.exit(1)

# Import from shared auth library
from lib.api_auth import (
    ApiClient,
    base62_encode,
    TEST_API_KEY,
    TEST_PRIVATE_KEY_HEX,
    get_test_client,
)

# =============================================================================
# Configuration
# =============================================================================

GATEWAY_URL = os.environ.get("GATEWAY_URL", "http://localhost:8080")

# Use test credentials from lib
API_KEY = TEST_API_KEY
PRIVATE_KEY_HEX = TEST_PRIVATE_KEY_HEX

# =============================================================================
# Tests
# =============================================================================

def test_public_endpoint():
    """Test that public endpoints work without auth."""
    print("\n[TEST] Public endpoint (no auth)...")
    resp = requests.get(f"{GATEWAY_URL}/api/v1/public/exchange_info")
    if resp.status_code == 200:
        data = resp.json()
        print(f"  ✅ Status: {resp.status_code}")
        print(f"  ✅ Assets: {len(data.get('data', {}).get('assets', []))}")
        return True
    else:
        print(f"  ❌ Status: {resp.status_code}")
        print(f"  ❌ Response: {resp.text[:200]}")
        return False

def test_private_endpoint_no_auth():
    """Test that private endpoints reject requests without auth."""
    print("\n[TEST] Private endpoint without auth...")
    resp = requests.get(f"{GATEWAY_URL}/api/v1/private/orders")
    if resp.status_code == 401:
        print(f"  ✅ Correctly rejected (401)")
        return True
    else:
        print(f"  ⚠️  Status: {resp.status_code} (expected 401)")
        return False

def test_private_endpoint_with_auth():
    """Test authenticated access to private endpoints."""
    print("\n[TEST] Private endpoint with auth...")
    client = get_test_client(GATEWAY_URL)
    # Note: path must include query params as they are part of the signed payload
    resp = client.get("/api/v1/private/orders?user_id=1")
    
    if resp.status_code == 200:
        print(f"  ✅ Status: {resp.status_code}")
        print(f"  ✅ Response: {resp.text[:100]}...")
        return True
    else:
        print(f"  ❌ Status: {resp.status_code}")
        print(f"  ❌ Response: {resp.text[:200]}")
        return False

def test_replay_attack():
    """Test that replay attacks are rejected."""
    print("\n[TEST] Replay attack detection...")
    
    client = ApiClient(API_KEY, PRIVATE_KEY_HEX, GATEWAY_URL)
    path = "/api/v1/private/orders?user_id=1"
    
    # First request - capture the auth header used
    auth1 = client._sign_request("GET", path)
    resp1 = requests.get(f"{GATEWAY_URL}{path}", headers={"Authorization": auth1})
    print(f"  First request: {resp1.status_code}")
    
    # Replay attack - use EXACT SAME auth header again
    resp2 = requests.get(f"{GATEWAY_URL}{path}", headers={"Authorization": auth1})
    
    # Second request should be rejected (replay detected via ts_nonce)
    if resp2.status_code == 401:
        print(f"  ✅ Replay correctly rejected (401)")
        return True
    else:
        print(f"  ⚠️  Status: {resp2.status_code}")
        return False

def test_invalid_signature():
    """Test that invalid signatures are rejected."""
    print("\n[TEST] Invalid signature...")
    
    # Create a request with wrong private key
    wrong_key = "0" * 64  # Invalid key
    try:
        client = ApiClient(API_KEY, wrong_key, GATEWAY_URL)
        resp = client.get("/api/v1/private/orders")
        if resp.status_code == 401:
            print(f"  ✅ Invalid signature rejected (401)")
            return True
        else:
            print(f"  ⚠️  Status: {resp.status_code}")
            return False
    except Exception as e:
        print(f"  ✅ Exception (expected for invalid key): {type(e).__name__}")
        return True

def test_post_request_signature():
    """Test POST request with signature (body included in payload)."""
    print("\n[TEST] POST request with signature...")
    client = get_test_client(GATEWAY_URL)
    
    # POST /api/v1/private/order - creates an order (auth required)
    order_data = {
        "symbol": "BTC_USDT",
        "side": "BUY",
        "type": "LIMIT",
        "price": "30000.00",
        "qty": "0.001"
    }
    
    resp = client.post("/api/v1/private/order", order_data)
    
    # We expect either 200 (order created) or 400 (validation error) 
    # but NOT 401 (auth failure)
    if resp.status_code == 401:
        print(f"  ❌ Auth failed (401)")
        print(f"  ❌ Response: {resp.text[:200]}")
        return False
    else:
        print(f"  ✅ Auth passed, status: {resp.status_code}")
        print(f"  ✅ Response: {resp.text[:100]}...")
        return True

def test_ts_nonce_time_window():
    """Test that ts_nonce outside time window is rejected."""
    print("\n[TEST] ts_nonce time window (30s)...")
    
    # Create expired ts_nonce (60 seconds in the past)
    old_ts_nonce = str(int(time.time() * 1000) - 60 * 1000)
    
    # Sign with old ts_nonce
    signing_key = SigningKey(bytes.fromhex(PRIVATE_KEY_HEX))
    path = "/api/v1/private/orders"
    payload = f"{API_KEY}{old_ts_nonce}GET{path}"
    signature = signing_key.sign(payload.encode()).signature
    sig_b62 = base62_encode(signature)
    auth = f"ZXINF v1.{API_KEY}.{old_ts_nonce}.{sig_b62}"
    
    resp = requests.get(f"{GATEWAY_URL}{path}", headers={"Authorization": auth})
    
    if resp.status_code == 401:
        try:
            data = resp.json()
            error_code = data.get("data", {}).get("code", "")
            if "TS_NONCE" in str(error_code) or "TS_NONCE" in resp.text:
                print(f"  ✅ Expired ts_nonce rejected (401) - TS_NONCE_TOO_FAR")
            else:
                print(f"  ✅ Expired ts_nonce rejected (401)")
        except:
            print(f"  ✅ Expired ts_nonce rejected (401)")
        return True
    else:
        print(f"  ❌ Status: {resp.status_code} (expected 401)")
        return False

def test_invalid_api_key():
    """Test that non-existent API key is rejected."""
    print("\n[TEST] Invalid API key...")
    
    invalid_api_key = "AK_INVALID_KEY_12345"
    
    # Sign with valid private key but invalid api_key
    signing_key = SigningKey(bytes.fromhex(PRIVATE_KEY_HEX))
    ts_nonce = str(int(time.time() * 1000))
    path = "/api/v1/private/orders"
    payload = f"{invalid_api_key}{ts_nonce}GET{path}"
    signature = signing_key.sign(payload.encode()).signature
    sig_b62 = base62_encode(signature)
    auth = f"ZXINF v1.{invalid_api_key}.{ts_nonce}.{sig_b62}"
    
    resp = requests.get(f"{GATEWAY_URL}{path}", headers={"Authorization": auth})
    
    if resp.status_code == 401:
        print(f"  ✅ Invalid API key rejected (401)")
        return True
    else:
        print(f"  ❌ Status: {resp.status_code} (expected 401)")
        return False

# =============================================================================
# Main
# =============================================================================

def main():
    print("=" * 60)
    print("API Authentication Test Suite")
    print("=" * 60)
    print(f"Gateway URL: {GATEWAY_URL}")
    print(f"API Key: {API_KEY}")
    print(f"Auth Library: lib/auth.py")
    
    results = []
    
    # Run tests
    results.append(("Public Endpoint", test_public_endpoint()))
    results.append(("Private No Auth", test_private_endpoint_no_auth()))
    results.append(("Private With Auth", test_private_endpoint_with_auth()))
    results.append(("Replay Attack", test_replay_attack()))
    results.append(("Invalid Signature", test_invalid_signature()))
    results.append(("POST Request Signature", test_post_request_signature()))
    results.append(("ts_nonce Time Window", test_ts_nonce_time_window()))
    results.append(("Invalid API Key", test_invalid_api_key()))
    
    # Summary
    print("\n" + "=" * 60)
    print("RESULTS")
    print("=" * 60)
    passed = sum(1 for _, r in results if r)
    total = len(results)
    
    for name, result in results:
        status = "✅ PASS" if result else "❌ FAIL"
        print(f"  {status}: {name}")
    
    print(f"\nTotal: {passed}/{total} tests passed")
    
    return 0 if passed == total else 1

if __name__ == "__main__":
    sys.exit(main())
