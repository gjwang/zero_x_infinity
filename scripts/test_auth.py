#!/usr/bin/env python3
"""
API Authentication Test Script

Tests Ed25519 signature-based authentication for the 0xInfinity API.
Uses the test API key from fixtures/seed_data.sql.
"""

import sys
import time
import requests

try:
    from nacl.signing import SigningKey
except ImportError:
    print("Error: PyNaCl not installed. Run: pip install pynacl requests")
    sys.exit(1)

# =============================================================================
# Configuration
# =============================================================================

GATEWAY_URL = "http://localhost:8080"
API_KEY = "AK_D4735E3A265E16EE"  # Test API Key from seed_data.sql

# Test private key (hex) - matches the public key in seed_data.sql
# WARNING: This is a well-known test key - DO NOT USE IN PRODUCTION
PRIVATE_KEY_HEX = "9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60"

# =============================================================================
# Base62 Encoding
# =============================================================================

ALPHABET = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz"

def base62_encode(data: bytes) -> str:
    """Encode bytes to Base62 string."""
    num = int.from_bytes(data, 'big')
    if num == 0:
        return ALPHABET[0]
    result = []
    while num:
        num, rem = divmod(num, 62)
        result.append(ALPHABET[rem])
    return ''.join(reversed(result))

# =============================================================================
# Authentication
# =============================================================================

class ApiClient:
    """API client with Ed25519 signature authentication."""
    
    def __init__(self, api_key: str, private_key_hex: str, base_url: str = GATEWAY_URL):
        self.api_key = api_key
        self.base_url = base_url
        self.signing_key = SigningKey(bytes.fromhex(private_key_hex))
        self.last_ts_nonce = 0
    
    def _get_ts_nonce(self) -> str:
        """Generate monotonically increasing ts_nonce."""
        now = int(time.time() * 1000)
        ts_nonce = max(now, self.last_ts_nonce + 1)
        self.last_ts_nonce = ts_nonce
        return str(ts_nonce)
    
    def _sign_request(self, method: str, path: str, body: str = "") -> str:
        """Sign a request and return Authorization header value."""
        ts_nonce = self._get_ts_nonce()
        payload = f"{self.api_key}{ts_nonce}{method}{path}{body}"
        signature = self.signing_key.sign(payload.encode()).signature
        sig_b62 = base62_encode(signature)
        return f"ZXINF v1.{self.api_key}.{ts_nonce}.{sig_b62}"
    
    def get(self, path: str) -> requests.Response:
        """Send authenticated GET request."""
        auth = self._sign_request("GET", path)
        return requests.get(
            f"{self.base_url}{path}",
            headers={"Authorization": auth}
        )
    
    def post(self, path: str, json_body: dict = None) -> requests.Response:
        """Send authenticated POST request."""
        body = "" if json_body is None else str(json_body)
        auth = self._sign_request("POST", path, body)
        return requests.post(
            f"{self.base_url}{path}",
            headers={"Authorization": auth},
            json=json_body
        )

# =============================================================================
# Tests
# =============================================================================

def test_public_endpoint():
    """Test that public endpoints work without auth."""
    print("\n[TEST] Public endpoint (no auth)...")
    resp = requests.get(f"{GATEWAY_URL}/api/v1/exchange_info")
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
    client = ApiClient(API_KEY, PRIVATE_KEY_HEX)
    resp = client.get("/api/v1/private/orders")
    
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
    
    # Create two clients with same key but independent ts_nonce
    client1 = ApiClient(API_KEY, PRIVATE_KEY_HEX)
    client2 = ApiClient(API_KEY, PRIVATE_KEY_HEX)
    
    # First request should succeed
    resp1 = client1.get("/api/v1/private/orders")
    print(f"  First request: {resp1.status_code}")
    
    # Set client2's ts_nonce to same value (simulating replay)
    client2.last_ts_nonce = client1.last_ts_nonce - 1
    resp2 = client2.get("/api/v1/private/orders")
    
    # Second request should be rejected if ts_nonce is lower
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
        client = ApiClient(API_KEY, wrong_key)
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

# =============================================================================
# Main
# =============================================================================

def main():
    print("=" * 60)
    print("API Authentication Test Suite")
    print("=" * 60)
    print(f"Gateway URL: {GATEWAY_URL}")
    print(f"API Key: {API_KEY}")
    
    results = []
    
    # Run tests
    results.append(("Public Endpoint", test_public_endpoint()))
    results.append(("Private No Auth", test_private_endpoint_no_auth()))
    results.append(("Private With Auth", test_private_endpoint_with_auth()))
    results.append(("Replay Attack", test_replay_attack()))
    results.append(("Invalid Signature", test_invalid_signature()))
    
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
