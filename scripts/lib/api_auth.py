#!/usr/bin/env python3
"""
Reusable API Authentication Library

Provides Ed25519 signature-based authentication for 0xInfinity API.
Can be imported by other scripts for authenticated API calls.

Usage:
    from lib.api_auth import ApiClient, base62_encode
    
    client = ApiClient(
        api_key="AK_D4735E3A265E16EE",
        private_key_hex="9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60"
    )
    resp = client.get("/api/v1/private/orders?user_id=1")
"""

import time
try:
    from nacl.signing import SigningKey
except ImportError:
    raise ImportError("PyNaCl not installed. Run: pip install pynacl")

try:
    import requests
except ImportError:
    raise ImportError("requests not installed. Run: pip install requests")


# =============================================================================
# Base62 Encoding
# =============================================================================

ALPHABET = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz"

def base62_encode(data: bytes) -> str:
    """
    Encode bytes to Base62 string.
    
    Args:
        data: Bytes to encode
        
    Returns:
        Base62 encoded string
    """
    num = int.from_bytes(data, 'big')
    if num == 0:
        return ALPHABET[0]
    result = []
    while num:
        num, rem = divmod(num, 62)
        result.append(ALPHABET[rem])
    return ''.join(reversed(result))


def base62_decode(s: str) -> bytes:
    """
    Decode Base62 string to bytes.
    
    Args:
        s: Base62 encoded string
        
    Returns:
        Decoded bytes
    """
    num = 0
    for char in s:
        num = num * 62 + ALPHABET.index(char)
    # Convert back to 64 bytes for Ed25519 signature
    byte_length = (num.bit_length() + 7) // 8
    return num.to_bytes(max(byte_length, 1), 'big')


# =============================================================================
# API Client with Ed25519 Authentication
# =============================================================================

class ApiClient:
    """
    API client with Ed25519 signature authentication.
    
    All private endpoints require signed requests with format:
        Authorization: ZXINF v1.{api_key}.{ts_nonce}.{signature_base62}
    
    Signature payload:
        {api_key}{ts_nonce}{method}{path}{body}
    
    Note: Server currently uses empty string for body in signature verification.
    """
    
    DEFAULT_BASE_URL = "http://localhost:8080"
    
    def __init__(
        self, 
        api_key: str, 
        private_key_hex: str, 
        base_url: str = None
    ):
        """
        Initialize API client.
        
        Args:
            api_key: API key (e.g., "AK_D4735E3A265E16EE")
            private_key_hex: Ed25519 private key as hex string (64 chars)
            base_url: Gateway base URL (default: http://localhost:8080)
        """
        self.api_key = api_key
        self.base_url = base_url or self.DEFAULT_BASE_URL
        self.signing_key = SigningKey(bytes.fromhex(private_key_hex))
        self.last_ts_nonce = 0
    
    def _get_ts_nonce(self) -> str:
        """
        Generate monotonically increasing ts_nonce.
        
        Returns millisecond timestamp, guaranteed to be strictly greater
        than previous value (prevents replay attacks).
        """
        now = int(time.time() * 1000)
        ts_nonce = max(now, self.last_ts_nonce + 1)
        self.last_ts_nonce = ts_nonce
        return str(ts_nonce)
    
    def _sign_request(self, method: str, path: str, body: str = "") -> str:
        """
        Sign a request and return Authorization header value.
        
        Args:
            method: HTTP method (GET, POST, etc.)
            path: Request path including query string
            body: Request body (currently ignored by server)
            
        Returns:
            Authorization header value
        """
        ts_nonce = self._get_ts_nonce()
        payload = f"{self.api_key}{ts_nonce}{method}{path}{body}"
        signature = self.signing_key.sign(payload.encode()).signature
        sig_b62 = base62_encode(signature)
        return f"ZXINF v1.{self.api_key}.{ts_nonce}.{sig_b62}"
    
    def get(self, path: str, **kwargs) -> requests.Response:
        """
        Send authenticated GET request.
        
        Args:
            path: Request path (e.g., "/api/v1/private/orders")
            **kwargs: Additional arguments passed to requests.get
            
        Returns:
            Response object
        """
        # Prepare full path with query params for signing
        params = kwargs.get("params")
        signed_path = path
        if params:
            from requests.models import PreparedRequest
            req = PreparedRequest()
            req.prepare_url(f"http://placeholder{path}", params)
            signed_path = req.url.replace("http://placeholder", "")
            
        auth = self._sign_request("GET", signed_path)
        headers = kwargs.pop("headers", {})
        headers["Authorization"] = auth
        return requests.get(
            f"{self.base_url}{path}",
            headers=headers,
            timeout=kwargs.get("timeout", 10),
            **kwargs
        )
    
    def post(self, path: str, json_body: dict = None, **kwargs) -> requests.Response:
        """
        Send authenticated POST request.
        
        Note: Server currently ignores body in signature verification (body="").
        This matches server-side middleware.rs line 85.
        
        Args:
            path: Request path (e.g., "/api/v1/private/order")
            json_body: JSON body to send
            **kwargs: Additional arguments passed to requests.post
            
        Returns:
            Response object
        """
        # Server uses empty body for signature verification
        auth = self._sign_request("POST", path, "")
        headers = kwargs.pop("headers", {})
        headers["Authorization"] = auth
        return requests.post(
            f"{self.base_url}{path}",
            headers=headers,
            json=json_body,
            timeout=kwargs.get("timeout", 10),
            **kwargs
        )
    
    def delete(self, path: str, **kwargs) -> requests.Response:
        """
        Send authenticated DELETE request.
        
        Args:
            path: Request path
            **kwargs: Additional arguments passed to requests.delete
            
        Returns:
            Response object
        """
        auth = self._sign_request("DELETE", path)
        headers = kwargs.pop("headers", {})
        headers["Authorization"] = auth
        return requests.delete(
            f"{self.base_url}{path}",
            headers=headers,
            timeout=kwargs.get("timeout", 10),
            **kwargs
        )


# =============================================================================
# Test Keys (for development/testing only)
# =============================================================================

# These keys are from fixtures/seed_data.sql
# WARNING: DO NOT USE IN PRODUCTION
# These keys are from fixtures/seed_data.sql
# WARNING: DO NOT USE IN PRODUCTION
TEST_API_KEY = "AK_D4735E3A265E16EE" # User 1 (System)
TEST_PRIVATE_KEY_HEX = "9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60"

# User ID to (API Key, Private Key) mapping
USER_KEYS = {
    # System User
    1: (TEST_API_KEY, TEST_PRIVATE_KEY_HEX),
    # Test User 1001
    1001: ("AK_0000000000001001", TEST_PRIVATE_KEY_HEX),
    # Test User 1002
    1002: ("AK_0000000000001002", TEST_PRIVATE_KEY_HEX),
}


def get_test_client(base_url: str = None, user_id: int = 1) -> ApiClient:
    """
    Get an API client configured with test credentials.
    
    Args:
        base_url: Optional base URL override
        user_id: User ID to authenticate as (1, 1001, or 1002)
        
    Returns:
        Configured ApiClient instance
    """
    api_key, private_key = USER_KEYS.get(user_id, (TEST_API_KEY, TEST_PRIVATE_KEY_HEX))
    
    return ApiClient(
        api_key=api_key,
        private_key_hex=private_key,
        base_url=base_url
    )


# =============================================================================
# Module test
# =============================================================================

if __name__ == "__main__":
    print("Testing auth library...")
    
    # Test Base62 encoding
    test_bytes = b'\x00\x01\x02\x03'
    encoded = base62_encode(test_bytes)
    print(f"Base62 encode test: {test_bytes.hex()} -> {encoded}")
    
    # Test client creation
    client = get_test_client()
    print(f"Client created with API key: {client.api_key}")
    
    # Test signature generation
    auth = client._sign_request("GET", "/api/v1/private/orders")
    print(f"Sample auth header: {auth[:50]}...")
    
    print("\n✅ Auth library loaded successfully")
