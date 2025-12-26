#!/usr/bin/env python3
"""
Zero X Infinity Python SDK

Type-safe SDK generated from OpenAPI spec, built on top of api_auth.py.
Provides convenient methods for all public and private API endpoints.

Usage:
    from lib.zero_x_infinity_sdk import ZeroXInfinityClient
    
    # For public endpoints (no auth needed)
    client = ZeroXInfinityClient()
    depth = client.get_depth(symbol="BTC_USDT", limit=20)
    
    # For private endpoints (auth required)
    client = ZeroXInfinityClient(
        api_key="AK_0000000000001001",
        private_key_hex="9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60"
    )
    orders = client.get_orders(limit=10)
"""

from typing import Optional, Dict, Any, List
from dataclasses import dataclass
import requests

from .api_auth import ApiClient, get_test_client, TEST_API_KEY, TEST_PRIVATE_KEY_HEX


# =============================================================================
# Response Types (Generated from OpenAPI schemas)
# =============================================================================

@dataclass
class DepthData:
    """Order book depth data"""
    symbol: str
    bids: List[List[str]]  # [[price, qty], ...]
    asks: List[List[str]]
    last_update_id: int


@dataclass  
class AssetInfo:
    """Asset information"""
    asset_id: int
    asset: str
    name: str
    decimals: int
    can_deposit: bool
    can_withdraw: bool
    can_trade: bool


@dataclass
class SymbolInfo:
    """Trading pair information"""
    symbol_id: int
    symbol: str
    base_asset: str
    quote_asset: str
    price_decimals: int
    qty_decimals: int
    is_tradable: bool
    is_visible: bool


@dataclass
class OrderResponse:
    """Order placement response"""
    order_id: int
    cid: Optional[str]
    order_status: str
    accepted_at: int


# =============================================================================
# SDK Client
# =============================================================================

class ZeroXInfinityClient:
    """
    Zero X Infinity API Client
    
    Provides type-safe access to all API endpoints.
    Built on top of api_auth.py for Ed25519 authentication.
    """
    
    def __init__(
        self,
        api_key: str = None,
        private_key_hex: str = None,
        base_url: str = "http://localhost:8080"
    ):
        """
        Initialize client.
        
        Args:
            api_key: API key for private endpoints (optional for public only)
            private_key_hex: Ed25519 private key hex (optional for public only)
            base_url: Gateway base URL
        """
        self.base_url = base_url
        self._auth_client = None
        
        if api_key and private_key_hex:
            self._auth_client = ApiClient(
                api_key=api_key,
                private_key_hex=private_key_hex,
                base_url=base_url
            )
    
    @classmethod
    def from_test_user(cls, user_id: int = 1001, base_url: str = None) -> "ZeroXInfinityClient":
        """Create client with test credentials for a specific user."""
        client = get_test_client(base_url=base_url, user_id=user_id)
        return cls(
            api_key=client.api_key,
            private_key_hex=client.signing_key._signing_key.hex()[:64],
            base_url=client.base_url
        )
    
    def _get(self, path: str, params: Dict = None) -> Dict:
        """Unauthenticated GET request."""
        resp = requests.get(
            f"{self.base_url}{path}",
            params=params,
            timeout=10
        )
        return resp.json()
    
    def _auth_get(self, path: str, params: Dict = None) -> Dict:
        """Authenticated GET request."""
        if not self._auth_client:
            raise RuntimeError("Auth required. Initialize with api_key and private_key_hex")
        resp = self._auth_client.get(path, params=params)
        return resp.json()
    
    def _auth_post(self, path: str, json_body: Dict = None) -> Dict:
        """Authenticated POST request."""
        if not self._auth_client:
            raise RuntimeError("Auth required. Initialize with api_key and private_key_hex")
        resp = self._auth_client.post(path, json_body=json_body)
        return resp.json()
    
    # =========================================================================
    # Public Endpoints (Market Data) - No Auth Required
    # =========================================================================
    
    def health_check(self) -> Dict:
        """GET /api/v1/health - Check service health"""
        return self._get("/api/v1/health")
    
    def get_depth(self, symbol: str = None, limit: int = 20) -> Dict:
        """
        GET /api/v1/public/depth - Get order book depth
        
        Args:
            symbol: Trading pair (e.g., "BTC_USDT")
            limit: Number of levels (default: 20, max: 100)
        """
        params = {"limit": limit}
        if symbol:
            params["symbol"] = symbol
        return self._get("/api/v1/public/depth", params=params)
    
    def get_klines(self, interval: str = "1m", limit: int = 100) -> Dict:
        """
        GET /api/v1/public/klines - Get K-line/candlestick data
        
        Args:
            interval: K-line interval (1m, 5m, 15m, 30m, 1h, 1d)
            limit: Number of K-lines (default: 100, max: 1000)
        """
        return self._get("/api/v1/public/klines", params={
            "interval": interval,
            "limit": limit
        })
    
    def get_assets(self) -> Dict:
        """GET /api/v1/public/assets - Get all assets"""
        return self._get("/api/v1/public/assets")
    
    def get_symbols(self) -> Dict:
        """GET /api/v1/public/symbols - Get all trading pairs"""
        return self._get("/api/v1/public/symbols")
    
    def get_exchange_info(self) -> Dict:
        """GET /api/v1/public/exchange_info - Get exchange metadata"""
        return self._get("/api/v1/public/exchange_info")
    
    # =========================================================================
    # Private Endpoints (Trading) - Auth Required
    # =========================================================================
    
    def create_order(
        self,
        symbol: str,
        side: str,
        order_type: str,
        qty: str,
        price: str = None,
        cid: str = None
    ) -> Dict:
        """
        POST /api/v1/private/order - Place an order
        
        Args:
            symbol: Trading pair (e.g., "BTC_USDT")
            side: "BUY" or "SELL"
            order_type: "LIMIT" or "MARKET"
            qty: Order quantity (e.g., "0.001")
            price: Order price for LIMIT orders
            cid: Client order ID (optional)
        """
        body = {
            "symbol": symbol,
            "side": side,
            "order_type": order_type,
            "qty": qty,
        }
        if price:
            body["price"] = price
        if cid:
            body["cid"] = cid
        return self._auth_post("/api/v1/private/order", json_body=body)
    
    def cancel_order(self, order_id: int) -> Dict:
        """
        POST /api/v1/private/cancel - Cancel an order
        
        Args:
            order_id: Order ID to cancel
        """
        return self._auth_post("/api/v1/private/cancel", json_body={
            "order_id": order_id
        })
    
    def get_order(self, order_id: int) -> Dict:
        """
        GET /api/v1/private/order/{order_id} - Get order details
        
        Args:
            order_id: Order ID
        """
        return self._auth_get(f"/api/v1/private/order/{order_id}")
    
    def get_orders(self, limit: int = 10) -> Dict:
        """
        GET /api/v1/private/orders - Get user's orders
        
        Args:
            limit: Number of orders (default: 10)
        """
        return self._auth_get("/api/v1/private/orders", params={"limit": limit})
    
    def get_trades(self, limit: int = 100) -> Dict:
        """
        GET /api/v1/private/trades - Get trade history
        
        Args:
            limit: Number of trades (default: 100)
        """
        return self._auth_get("/api/v1/private/trades", params={"limit": limit})
    
    def get_balances(self, asset_id: int) -> Dict:
        """
        GET /api/v1/private/balances - Get balance for an asset
        
        Args:
            asset_id: Asset ID
        """
        return self._auth_get("/api/v1/private/balances", params={"asset_id": asset_id})
    
    def get_all_balances(self) -> Dict:
        """GET /api/v1/private/balances/all - Get all balances"""
        return self._auth_get("/api/v1/private/balances/all")
    
    def create_transfer(
        self,
        from_account: str,
        to_account: str,
        asset: str,
        amount: str,
        cid: str = None
    ) -> Dict:
        """
        POST /api/v1/private/transfer - Create internal transfer
        
        Args:
            from_account: Source account type ("spot" or "funding")
            to_account: Destination account type
            asset: Asset symbol (e.g., "USDT")
            amount: Transfer amount (e.g., "100.00")
            cid: Client idempotency key (optional)
        """
        body = {
            "from": from_account,
            "to": to_account,
            "asset": asset,
            "amount": amount,
        }
        if cid:
            body["cid"] = cid
        return self._auth_post("/api/v1/private/transfer", json_body=body)
    
    def get_transfer(self, req_id: str) -> Dict:
        """
        GET /api/v1/private/transfer/{req_id} - Get transfer status
        
        Args:
            req_id: Transfer request ID (ULID format)
        """
        return self._auth_get(f"/api/v1/private/transfer/{req_id}")


# =============================================================================
# CLI Test
# =============================================================================

if __name__ == "__main__":
    print("Testing Zero X Infinity SDK...")
    
    # Test public endpoints (no auth)
    client = ZeroXInfinityClient()
    print("\n📊 Testing public endpoints:")
    
    # Health check
    try:
        health = client.health_check()
        print(f"  Health: {health.get('code')} - {health.get('msg')}")
    except Exception as e:
        print(f"  Health: Error - {e}")
    
    # Exchange info
    try:
        info = client.get_exchange_info()
        if info.get('code') == 0:
            data = info.get('data', {})
            print(f"  Exchange: {len(data.get('assets', []))} assets, {len(data.get('symbols', []))} symbols")
    except Exception as e:
        print(f"  Exchange: Error - {e}")
    
    # Test private endpoint (with test auth)
    print("\n🔐 Testing private endpoints (test user 1001):")
    try:
        auth_client = ZeroXInfinityClient.from_test_user(user_id=1001)
        orders = auth_client.get_orders(limit=5)
        print(f"  Orders: code={orders.get('code')}")
    except Exception as e:
        print(f"  Orders: Error - {e}")
    
    print("\n✅ SDK test complete")
