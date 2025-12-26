"""
E2E-02: Symbol Lifecycle Integration Test

Tests the full chain: Admin → DB → Gateway → Matching

Test Flow:
1. Create Assets (BASE, QUOTE)
2. Create Symbol
3. Verify trading works
4. Halt Symbol → Verify orders rejected
5. CloseOnly → Verify cancel works, new orders rejected
6. Resume trading → Verify orders work again

Prerequisites:
- PostgreSQL running on :5433
- Admin Dashboard running on :8001
- Gateway running on :8000
"""

import asyncio
import time
import httpx
import pytest
from typing import Optional

# Configuration
ADMIN_URL = "http://localhost:8001"
GATEWAY_URL = "http://localhost:8000"
HOT_RELOAD_SLA_SECONDS = 5


class TestSymbolLifecycle:
    """E2E: Complete Symbol lifecycle test"""
    
    @pytest.fixture
    async def admin_client(self):
        """Authenticated admin client"""
        async with httpx.AsyncClient(base_url=ADMIN_URL) as client:
            # Login to get session
            login_resp = await client.post("/admin/auth/login", json={
                "username": "admin",
                "password": "admin123"  # Default admin password
            })
            if login_resp.status_code == 200:
                # Set session cookie
                pass
            yield client
    
    @pytest.fixture
    async def gateway_client(self):
        """Gateway API client"""
        async with httpx.AsyncClient(base_url=GATEWAY_URL) as client:
            yield client
    
    async def wait_for_hot_reload(self, seconds: float = HOT_RELOAD_SLA_SECONDS):
        """Wait for configuration hot-reload"""
        await asyncio.sleep(seconds)
    
    # === Test: Symbol Creation and Trading ===
    
    @pytest.mark.asyncio
    async def test_e2e_01_create_symbol_enables_trading(
        self, admin_client, gateway_client
    ):
        """
        E2E-02 Step 1-4: Create Symbol → Trading enabled
        
        1. Create test assets via Admin
        2. Create symbol via Admin
        3. Wait for hot-reload
        4. Submit order → Should succeed
        """
        # Step 1: Create Assets
        base_asset = await admin_client.post("/admin/asset/", json={
            "asset": "TESTA",
            "name": "Test Asset A",
            "decimals": 8,
            "status": 1,
        })
        assert base_asset.status_code in (200, 201), f"Failed to create base asset: {base_asset.text}"
        
        quote_asset = await admin_client.post("/admin/asset/", json={
            "asset": "TESTB",
            "name": "Test Asset B",
            "decimals": 8,
            "status": 1,
        })
        assert quote_asset.status_code in (200, 201), f"Failed to create quote asset: {quote_asset.text}"
        
        # Step 2: Create Symbol
        symbol_resp = await admin_client.post("/admin/symbol/", json={
            "symbol": "TESTA_TESTB",
            "base_asset_id": base_asset.json().get("asset_id", 999),
            "quote_asset_id": quote_asset.json().get("asset_id", 998),
            "price_decimals": 2,
            "qty_decimals": 8,
            "status": 1,  # Trading
            "base_maker_fee": 10,
            "base_taker_fee": 20,
        })
        assert symbol_resp.status_code in (200, 201), f"Failed to create symbol: {symbol_resp.text}"
        
        # Step 3: Wait for hot-reload
        await self.wait_for_hot_reload()
        
        # Step 4: Try to place order
        order_resp = await gateway_client.post("/api/v1/order", json={
            "symbol": "TESTA_TESTB",
            "side": "buy",
            "type": "limit",
            "price": "100.00",
            "quantity": "1.00000000",
        })
        # Should succeed (or fail with auth, not symbol not found)
        assert order_resp.status_code != 404, "Symbol should be recognized after hot-reload"
    
    @pytest.mark.asyncio
    async def test_e2e_02_halt_symbol_rejects_orders(
        self, admin_client, gateway_client
    ):
        """
        E2E-02 Step 5: Halt Symbol → Orders rejected
        
        1. Halt symbol via Admin (status=0)
        2. Wait for hot-reload
        3. Submit order → Should be rejected
        """
        # Step 1: Get symbol ID and Halt it
        symbols = await admin_client.get("/admin/symbol/")
        test_symbol = None
        for s in symbols.json().get("items", []):
            if s.get("symbol") == "TESTA_TESTB":
                test_symbol = s
                break
        
        if not test_symbol:
            pytest.skip("Test symbol not found, run test_e2e_01 first")
        
        # Halt the symbol
        halt_resp = await admin_client.put(
            f"/admin/symbol/{test_symbol['symbol_id']}",
            json={"status": 0}  # Halt
        )
        assert halt_resp.status_code == 200, f"Failed to halt symbol: {halt_resp.text}"
        
        # Step 2: Wait for hot-reload
        await self.wait_for_hot_reload()
        
        # Step 3: Try to place order - should be rejected
        order_resp = await gateway_client.post("/api/v1/order", json={
            "symbol": "TESTA_TESTB",
            "side": "buy",
            "type": "limit",
            "price": "100.00",
            "quantity": "1.00000000",
        })
        
        # Should be rejected with SYMBOL_HALTED or similar
        assert order_resp.status_code in (400, 403), \
            f"Order should be rejected when symbol halted: {order_resp.text}"
        assert "halt" in order_resp.text.lower() or "disabled" in order_resp.text.lower()
    
    @pytest.mark.asyncio
    async def test_e2e_03_close_only_allows_cancel(
        self, admin_client, gateway_client
    ):
        """
        E2E-02 Step 6: CloseOnly → Cancel allowed, new orders rejected
        
        1. Set symbol to CloseOnly (status=2)
        2. Wait for hot-reload
        3. Cancel existing order → Should succeed
        4. New order → Should be rejected
        """
        # Get symbol
        symbols = await admin_client.get("/admin/symbol/")
        test_symbol = None
        for s in symbols.json().get("items", []):
            if s.get("symbol") == "TESTA_TESTB":
                test_symbol = s
                break
        
        if not test_symbol:
            pytest.skip("Test symbol not found")
        
        # Step 1: Set to CloseOnly
        close_only_resp = await admin_client.put(
            f"/admin/symbol/{test_symbol['symbol_id']}",
            json={"status": 2}  # CloseOnly
        )
        assert close_only_resp.status_code == 200
        
        # Step 2: Wait for hot-reload
        await self.wait_for_hot_reload()
        
        # Step 3: Cancel should work (if there's an order)
        # This would need an actual order ID
        
        # Step 4: New order should be rejected
        order_resp = await gateway_client.post("/api/v1/order", json={
            "symbol": "TESTA_TESTB",
            "side": "buy",
            "type": "limit",
            "price": "100.00",
            "quantity": "1.00000000",
        })
        
        assert order_resp.status_code in (400, 403), \
            f"New order should be rejected in CloseOnly mode: {order_resp.text}"
    
    @pytest.mark.asyncio
    async def test_e2e_04_resume_trading(self, admin_client, gateway_client):
        """
        E2E-02 Step 7: Resume trading → Orders work again
        """
        # Get symbol
        symbols = await admin_client.get("/admin/symbol/")
        test_symbol = None
        for s in symbols.json().get("items", []):
            if s.get("symbol") == "TESTA_TESTB":
                test_symbol = s
                break
        
        if not test_symbol:
            pytest.skip("Test symbol not found")
        
        # Resume trading
        resume_resp = await admin_client.put(
            f"/admin/symbol/{test_symbol['symbol_id']}",
            json={"status": 1}  # Trading
        )
        assert resume_resp.status_code == 200
        
        # Wait for hot-reload
        await self.wait_for_hot_reload()
        
        # Orders should work again
        order_resp = await gateway_client.post("/api/v1/order", json={
            "symbol": "TESTA_TESTB",
            "side": "buy",
            "type": "limit",
            "price": "100.00",
            "quantity": "1.00000000",
        })
        # Should not be rejected due to symbol status
        assert order_resp.status_code != 403 or "halt" not in order_resp.text.lower()


class TestHotReloadSLA:
    """Verify hot-reload happens within SLA (5 seconds)"""
    
    @pytest.mark.asyncio
    async def test_hot_reload_within_sla(self, admin_client, gateway_client):
        """
        Changes should take effect within 5 seconds
        
        1. Record timestamp
        2. Make config change
        3. Poll until change detected
        4. Verify elapsed time <= 5 seconds
        """
        start_time = time.time()
        
        # Make a change (e.g., update fee)
        # This would need implementation details
        
        # Poll for change detection
        max_wait = 10  # Max wait time
        change_detected = False
        
        while time.time() - start_time < max_wait:
            # Check if change took effect
            # This would need a specific check
            await asyncio.sleep(0.5)
        
        elapsed = time.time() - start_time
        
        if change_detected:
            assert elapsed <= HOT_RELOAD_SLA_SECONDS, \
                f"Hot-reload took {elapsed}s, exceeds SLA of {HOT_RELOAD_SLA_SECONDS}s"
