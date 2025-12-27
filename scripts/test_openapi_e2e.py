#!/usr/bin/env python3
"""
Zero X Infinity API E2E Test Suite

Comprehensive test coverage for all 15 API endpoints.
Designed for CI integration.

Usage:
    # Local testing
    python scripts/test_openapi_e2e.py
    
    # CI mode (exits with non-zero on failure)
    python scripts/test_openapi_e2e.py --ci

Requirements:
    pip install requests pynacl

Environment Variables:
    GATEWAY_URL: Gateway base URL (default: http://localhost:8080)
"""

import os
import sys
import json
import time
import argparse
from typing import Dict, List, Tuple, Optional
from dataclasses import dataclass

# Add scripts directory to path for imports
sys.path.insert(0, os.path.join(os.path.dirname(__file__), 'lib'))

try:
    from api_auth import get_test_client, ApiClient
except ImportError as e:
    print(f"Error: Cannot import api_auth. Detail: {e}")
    print(f"sys.path: {sys.path}")
    print(f"CWD: {os.getcwd()}")
    sys.exit(1)

try:
    import requests
except ImportError:
    print("Error: requests not installed. Run: pip install requests")
    sys.exit(1)


# =============================================================================
# Test Configuration
# =============================================================================

@dataclass
class TestResult:
    name: str
    passed: bool
    message: str
    duration_ms: float


class TestRunner:
    """API E2E Test Runner"""
    
    def __init__(self, base_url: str = None, verbose: bool = False):
        self.base_url = base_url or os.getenv("GATEWAY_URL", "http://localhost:8080")
        self.verbose = verbose
        self.results: List[TestResult] = []
        
        # Create authenticated client for private endpoints
        self.auth_client = get_test_client(base_url=self.base_url, user_id=1001)
        
    def log(self, msg: str):
        if self.verbose:
            print(f"  {msg}")
    
    def run_test(self, name: str, test_func) -> TestResult:
        """Run a single test and record result"""
        start = time.time()
        try:
            test_func()
            duration = (time.time() - start) * 1000
            result = TestResult(name, True, "OK", duration)
        except AssertionError as e:
            duration = (time.time() - start) * 1000
            result = TestResult(name, False, str(e), duration)
        except Exception as e:
            duration = (time.time() - start) * 1000
            result = TestResult(name, False, f"Error: {e}", duration)
        
        self.results.append(result)
        status = "✅" if result.passed else "❌"
        print(f"{status} {name} ({result.duration_ms:.1f}ms)")
        if not result.passed:
            print(f"   └─ {result.message}")
        return result

    # =========================================================================
    # Public Endpoints Tests (6)
    # =========================================================================
    
    def test_health(self):
        """GET /api/v1/health"""
        resp = requests.get(f"{self.base_url}/api/v1/health", timeout=10)
        assert resp.status_code == 200, f"Expected 200, got {resp.status_code}"
        data = resp.json()
        assert data["code"] == 0, f"Expected code=0, got {data['code']}"
        assert data["msg"] == "ok", f"Expected msg='ok', got {data['msg']}"
        assert "timestamp_ms" in data.get("data", {}), "Missing timestamp_ms in data"
        self.log(f"timestamp_ms: {data['data']['timestamp_ms']}")
    
    def test_depth(self):
        """GET /api/v1/public/depth"""
        resp = requests.get(
            f"{self.base_url}/api/v1/public/depth",
            params={"symbol": "BTC_USDT", "limit": 10},
            timeout=10
        )
        assert resp.status_code == 200, f"Expected 200, got {resp.status_code}"
        data = resp.json()
        assert data["code"] == 0, f"Expected code=0, got {data['code']}"
        depth = data.get("data", {})
        assert "bids" in depth, "Missing bids in depth data"
        assert "asks" in depth, "Missing asks in depth data"
        self.log(f"bids: {len(depth.get('bids', []))}, asks: {len(depth.get('asks', []))}")
    
    def test_klines(self):
        """GET /api/v1/public/klines"""
        resp = requests.get(
            f"{self.base_url}/api/v1/public/klines",
            params={"interval": "1m", "limit": 10},
            timeout=10
        )
        assert resp.status_code == 200, f"Expected 200, got {resp.status_code}"
        data = resp.json()
        assert data["code"] == 0, f"Expected code=0, got {data['code']}"
        self.log(f"klines count: {len(data.get('data', []))}")
    
    def test_assets(self):
        """GET /api/v1/public/assets"""
        resp = requests.get(f"{self.base_url}/api/v1/public/assets", timeout=10)
        assert resp.status_code == 200, f"Expected 200, got {resp.status_code}"
        data = resp.json()
        assert data["code"] == 0, f"Expected code=0, got {data['code']}"
        assets = data.get("data", [])
        assert len(assets) > 0, "Expected at least 1 asset"
        # Verify asset structure
        if assets:
            asset = assets[0]
            assert "asset_id" in asset, "Missing asset_id"
            assert "asset" in asset, "Missing asset symbol"
        self.log(f"assets: {[a['asset'] for a in assets]}")
    
    def test_symbols(self):
        """GET /api/v1/public/symbols"""
        resp = requests.get(f"{self.base_url}/api/v1/public/symbols", timeout=10)
        assert resp.status_code == 200, f"Expected 200, got {resp.status_code}"
        data = resp.json()
        assert data["code"] == 0, f"Expected code=0, got {data['code']}"
        symbols = data.get("data", [])
        assert len(symbols) > 0, "Expected at least 1 symbol"
        # Verify symbol structure
        if symbols:
            sym = symbols[0]
            assert "symbol_id" in sym, "Missing symbol_id"
            assert "symbol" in sym, "Missing symbol name"
        self.log(f"symbols: {[s['symbol'] for s in symbols]}")
    
    def test_exchange_info(self):
        """GET /api/v1/public/exchange_info"""
        resp = requests.get(f"{self.base_url}/api/v1/public/exchange_info", timeout=10)
        assert resp.status_code == 200, f"Expected 200, got {resp.status_code}"
        data = resp.json()
        assert data["code"] == 0, f"Expected code=0, got {data['code']}"
        info = data.get("data", {})
        assert "assets" in info, "Missing assets in exchange_info"
        assert "symbols" in info, "Missing symbols in exchange_info"
        assert "server_time" in info, "Missing server_time in exchange_info"
        self.log(f"server_time: {info['server_time']}")

    # =========================================================================
    # Private Endpoints Tests (9)
    # =========================================================================
    
    def test_get_orders(self):
        """GET /api/v1/private/orders (auth required)"""
        resp = self.auth_client.get("/api/v1/private/orders", params={"limit": 5})
        assert resp.status_code == 200, f"Expected 200, got {resp.status_code}"
        data = resp.json()
        assert data["code"] == 0, f"Expected code=0, got {data['code']}: {data.get('msg')}"
        self.log(f"orders count: {len(data.get('data', []))}")
    
    def test_get_trades(self):
        """GET /api/v1/private/trades (auth required)"""
        resp = self.auth_client.get("/api/v1/private/trades", params={"limit": 5})
        assert resp.status_code == 200, f"Expected 200, got {resp.status_code}"
        data = resp.json()
        assert data["code"] == 0, f"Expected code=0, got {data['code']}: {data.get('msg')}"
        self.log(f"trades count: {len(data.get('data', []))}")
    
    def test_get_balances(self):
        """GET /api/v1/private/balances (auth required)"""
        resp = self.auth_client.get("/api/v1/private/balances", params={"asset_id": 1})
        assert resp.status_code in [200, 404], f"Expected 200 or 404, got {resp.status_code}"
        data = resp.json()
        # 200 = balance found, 404 = no balance (both acceptable)
        self.log(f"balance response code: {data['code']}")
    
    def test_get_all_balances(self):
        """GET /api/v1/private/balances/all (auth required)"""
        resp = self.auth_client.get("/api/v1/private/balances/all")
        assert resp.status_code == 200, f"Expected 200, got {resp.status_code}"
        data = resp.json()
        assert data["code"] == 0, f"Expected code=0, got {data['code']}: {data.get('msg')}"
        balances = data.get("data", [])
        self.log(f"balances count: {len(balances)}")
    
    def test_create_order(self):
        """POST /api/v1/private/order (auth required)"""
        order = {
            "symbol": "BTC_USDT",
            "side": "BUY",
            "order_type": "LIMIT",
            "qty": "0.001",
            "price": "50000.00",
            "cid": f"test_{int(time.time() * 1000)}"
        }
        resp = self.auth_client.post("/api/v1/private/order", json_body=order)
        assert resp.status_code in [200, 202], f"Expected 200/202, got {resp.status_code}"
        data = resp.json()
        assert data["code"] == 0, f"Expected code=0, got {data['code']}: {data.get('msg')}"
        order_resp = data.get("data", {})
        assert "order_id" in order_resp, "Missing order_id in response"
        self.log(f"order_id: {order_resp.get('order_id')}")
        # Store for cancel test
        self._last_order_id = order_resp.get("order_id")
    
    def test_cancel_order(self):
        """POST /api/v1/private/cancel (auth required)"""
        # Use order from previous test or a dummy ID
        order_id = getattr(self, '_last_order_id', 999999)
        cancel_req = {"order_id": order_id}
        resp = self.auth_client.post("/api/v1/private/cancel", json_body=cancel_req)
        # Accept 200 (cancelled) or 400 (already filled/cancelled)
        assert resp.status_code in [200, 400], f"Expected 200/400, got {resp.status_code}"
        data = resp.json()
        self.log(f"cancel response: code={data['code']}, msg={data['msg']}")
    
    def test_get_order(self):
        """GET /api/v1/private/order/{order_id} (auth required)"""
        order_id = getattr(self, '_last_order_id', 1)
        resp = self.auth_client.get(f"/api/v1/private/order/{order_id}")
        # Accept 200 (found) or 404 (not found)
        assert resp.status_code in [200, 404], f"Expected 200/404, got {resp.status_code}"
        data = resp.json()
        self.log(f"get_order response: code={data['code']}")
    
    def test_create_transfer(self):
        """POST /api/v1/private/transfer (auth required)"""
        transfer_req = {
            "from": "spot",
            "to": "funding",
            "asset": "USDT",
            "amount": "1.00",
            "cid": f"test_transfer_{int(time.time() * 1000)}"
        }
        resp = self.auth_client.post("/api/v1/private/transfer", json_body=transfer_req)
        # Accept various responses depending on balance state
        # 200 = success, 400 = bad request, 422 = validation error, 503 = service unavailable
        assert resp.status_code in [200, 400, 422, 503], f"Unexpected status: {resp.status_code}"
        data = resp.json()
        self.log(f"transfer response: code={data['code']}, msg={data.get('msg', '')[:50]}")
        # Store req_id if successful
        if data["code"] == 0 and data.get("data"):
            self._last_transfer_id = data["data"].get("req_id")
    
    def test_get_transfer(self):
        """GET /api/v1/private/transfer/{req_id} (auth required)"""
        req_id = getattr(self, '_last_transfer_id', "01HXYZ123456789ABCDEF")
        resp = self.auth_client.get(f"/api/v1/private/transfer/{req_id}")
        # Accept various responses
        assert resp.status_code in [200, 400, 404, 503], f"Unexpected status: {resp.status_code}"
        data = resp.json()
        self.log(f"get_transfer response: code={data['code']}")

    # =========================================================================
    # OpenAPI Spec Tests
    # =========================================================================
    
    def test_openapi_json(self):
        """GET /api-docs/openapi.json"""
        resp = requests.get(f"{self.base_url}/api-docs/openapi.json", timeout=10)
        assert resp.status_code == 200, f"Expected 200, got {resp.status_code}"
        spec = resp.json()
        assert spec.get("openapi", "").startswith("3."), "Expected OpenAPI 3.x"
        assert spec.get("info", {}).get("title") == "Zero X Infinity Exchange API"
        paths = spec.get("paths", {})
        assert len(paths) >= 15, f"Expected at least 15 paths, got {len(paths)}"
        self.log(f"OpenAPI paths: {len(paths)}")
    
    def test_swagger_ui(self):
        """GET /docs (Swagger UI HTML)"""
        resp = requests.get(f"{self.base_url}/docs", timeout=10)
        assert resp.status_code == 200, f"Expected 200, got {resp.status_code}"
        # Check it returns HTML
        content_type = resp.headers.get("content-type", "")
        assert "text/html" in content_type, f"Expected HTML, got {content_type}"
        self.log("Swagger UI HTML served successfully")

    # =========================================================================
    # Run All Tests
    # =========================================================================
    
    def run_all(self) -> Tuple[int, int]:
        """Run all tests and return (passed, failed) counts"""
        print("\n" + "=" * 60)
        print("Zero X Infinity API E2E Test Suite")
        print(f"Gateway: {self.base_url}")
        print("=" * 60 + "\n")
        
        # Check gateway is reachable
        print("📡 Checking Gateway connectivity...")
        try:
            resp = requests.get(f"{self.base_url}/api/v1/health", timeout=5)
            if resp.status_code != 200:
                print(f"❌ Gateway returned status {resp.status_code}")
                return 0, 1
        except Exception as e:
            print(f"❌ Cannot connect to Gateway: {e}")
            print("   Ensure Gateway is running: cargo run --release -- --gateway --port 8080")
            return 0, 1
        print("✅ Gateway is reachable\n")
        
        # Public endpoints
        print("📊 Public Endpoints (6 tests)")
        print("-" * 40)
        self.run_test("health", self.test_health)
        self.run_test("depth", self.test_depth)
        self.run_test("klines", self.test_klines)
        self.run_test("assets", self.test_assets)
        self.run_test("symbols", self.test_symbols)
        self.run_test("exchange_info", self.test_exchange_info)
        
        # Private endpoints
        print("\n🔒 Private Endpoints (9 tests)")
        print("-" * 40)
        self.run_test("get_orders", self.test_get_orders)
        self.run_test("get_trades", self.test_get_trades)
        self.run_test("get_balances", self.test_get_balances)
        self.run_test("get_all_balances", self.test_get_all_balances)
        self.run_test("create_order", self.test_create_order)
        self.run_test("cancel_order", self.test_cancel_order)
        self.run_test("get_order", self.test_get_order)
        self.run_test("create_transfer", self.test_create_transfer)
        self.run_test("get_transfer", self.test_get_transfer)
        
        # OpenAPI verification
        print("\n📖 OpenAPI Verification (2 tests)")
        print("-" * 40)
        self.run_test("openapi_json", self.test_openapi_json)
        self.run_test("swagger_ui", self.test_swagger_ui)
        
        # Summary
        passed = sum(1 for r in self.results if r.passed)
        failed = sum(1 for r in self.results if not r.passed)
        total_time = sum(r.duration_ms for r in self.results)
        
        print("\n" + "=" * 60)
        print(f"Results: {passed} passed, {failed} failed ({total_time:.0f}ms)")
        print("=" * 60)
        
        if failed > 0:
            print("\n❌ Failed tests:")
            for r in self.results:
                if not r.passed:
                    print(f"   - {r.name}: {r.message}")
        
        return passed, failed


# =============================================================================
# Main
# =============================================================================

def main():
    parser = argparse.ArgumentParser(description="Zero X Infinity API E2E Test Suite")
    parser.add_argument("--ci", action="store_true", help="CI mode (exit 1 on failure)")
    parser.add_argument("--url", default=None, help="Gateway URL (default: localhost:8080)")
    parser.add_argument("-v", "--verbose", action="store_true", help="Verbose output")
    args = parser.parse_args()
    
    runner = TestRunner(base_url=args.url, verbose=args.verbose)
    passed, failed = runner.run_all()
    
    if args.ci and failed > 0:
        sys.exit(1)
    
    sys.exit(0 if failed == 0 else 1)


if __name__ == "__main__":
    main()
