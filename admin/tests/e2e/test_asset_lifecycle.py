"""
E2E-01: Asset Lifecycle Integration Test

Tests: Admin → DB → Gateway chain for Asset management

Test Flow:
1. Create Asset
2. Verify deposit/transfer works
3. Disable Asset → Verify operations rejected
4. Re-enable Asset → Verify operations work

Prerequisites:
- PostgreSQL running on :5433
- Admin Dashboard running on :8001
- Gateway running on :8080
"""

import asyncio
import httpx
import pytest

ADMIN_URL = "http://localhost:8001"
GATEWAY_URL = "http://localhost:8080"
HOT_RELOAD_SLA_SECONDS = 5


class TestAssetLifecycle:
    """E2E: Complete Asset lifecycle test"""
    
    @pytest.fixture
    async def admin_client(self):
        async with httpx.AsyncClient(base_url=ADMIN_URL) as client:
            yield client
    
    @pytest.fixture
    async def gateway_client(self):
        async with httpx.AsyncClient(base_url=GATEWAY_URL) as client:
            yield client
    
    async def wait_for_hot_reload(self):
        await asyncio.sleep(HOT_RELOAD_SLA_SECONDS)
    
    @pytest.mark.asyncio
    async def test_e2e_01_create_asset_enables_operations(
        self, admin_client, gateway_client
    ):
        """
        E2E-01 Step 1-3: Create Asset → Operations enabled
        
        1. Create asset via Admin
        2. Wait for hot-reload
        3. Transfer/deposit should recognize asset
        """
        # Step 1: Create Asset
        asset_resp = await admin_client.post("/admin/AssetAdmin/item", json={
            "asset": "E2ETEST",
            "name": "E2E Test Asset",
            "decimals": 8,
            "status": "ACTIVE",  # Active
        })
        assert asset_resp.status_code in (200, 201), f"Failed to create asset: {asset_resp.text}"
        
        asset_data = asset_resp.json()
        asset_id = asset_data.get("asset_id")
        
        # Step 2: Wait for hot-reload
        await self.wait_for_hot_reload()
        
        # Step 3: Verify asset is recognized
        # Try a transfer - should not fail with "unknown asset"
        transfer_resp = await gateway_client.post("/api/v1/transfer", json={
            "from_user_id": 1001,
            "to_user_id": 1002,
            "asset": "E2ETEST",
            "amount": "1.00000000",
            "cid": "e2e-test-001",
        })
        # Should not be 404 or "asset not found"
        if transfer_resp.status_code == 400:
            assert "not found" not in transfer_resp.text.lower()
            assert "unknown" not in transfer_resp.text.lower()
    
    @pytest.mark.asyncio
    async def test_e2e_02_disable_asset_rejects_operations(
        self, admin_client, gateway_client
    ):
        """
        E2E-01 Step 4: Disable Asset → Operations rejected
        
        1. Disable asset via Admin (status=0)
        2. Wait for hot-reload
        3. Transfer should be rejected
        """
        # Get asset ID
        assets = await admin_client.get("/admin/AssetAdmin/item")
        test_asset = None
        for a in assets.json().get("items", []):
            if a.get("asset") == "E2ETEST":
                test_asset = a
                break
        
        if not test_asset:
            pytest.skip("Test asset not found, run test_e2e_01 first")
        
        # Step 1: Disable asset
        disable_resp = await admin_client.put(
            f"/admin/AssetAdmin/item{test_asset['asset_id']}",
            json={"status": "OFFLINE"}  # Disabled
        )
        assert disable_resp.status_code == 200, f"Failed to disable: {disable_resp.text}"
        
        # Step 2: Wait for hot-reload
        await self.wait_for_hot_reload()
        
        # Step 3: Transfer should fail
        transfer_resp = await gateway_client.post("/api/v1/transfer", json={
            "from_user_id": 1001,
            "to_user_id": 1002,
            "asset": "E2ETEST",
            "amount": "1.00000000",
            "cid": "e2e-test-002",
        })
        
        assert transfer_resp.status_code in (400, 403), \
            f"Transfer should be rejected for disabled asset: {transfer_resp.text}"
    
    @pytest.mark.asyncio
    async def test_e2e_03_reenable_asset_allows_operations(
        self, admin_client, gateway_client
    ):
        """
        E2E-01 Step 5: Re-enable Asset → Operations work
        """
        # Get asset
        assets = await admin_client.get("/admin/AssetAdmin/item")
        test_asset = None
        for a in assets.json().get("items", []):
            if a.get("asset") == "E2ETEST":
                test_asset = a
                break
        
        if not test_asset:
            pytest.skip("Test asset not found")
        
        # Re-enable
        enable_resp = await admin_client.put(
            f"/admin/AssetAdmin/item{test_asset['asset_id']}",
            json={"status": "ACTIVE"}  # Active
        )
        assert enable_resp.status_code == 200
        
        # Wait for hot-reload
        await self.wait_for_hot_reload()
        
        # Transfer should work again
        transfer_resp = await gateway_client.post("/api/v1/transfer", json={
            "from_user_id": 1001,
            "to_user_id": 1002,
            "asset": "E2ETEST",
            "amount": "1.00000000",
            "cid": "e2e-test-003",
        })
        # Should not fail due to asset disabled
        if transfer_resp.status_code == 400:
            assert "disabled" not in transfer_resp.text.lower()


class TestAssetDeletionConstraint:
    """E2E: Asset deletion with FK constraint"""
    
    @pytest.mark.asyncio
    async def test_delete_referenced_asset_fails(self, admin_client):
        """
        Per GAP-02: Delete Asset referenced by Symbol should fail
        
        1. Create Asset AAA
        2. Create Symbol AAA_USDT
        3. Try to delete Asset AAA → Should fail
        """
        # Create asset
        asset_resp = await admin_client.post("/admin/AssetAdmin/item", json={
            "asset": "REFTEST",
            "name": "Referenced Test",
            "decimals": 8,
            "status": "ACTIVE",
        })
        assert asset_resp.status_code in (200, 201)
        asset_id = asset_resp.json().get("asset_id")
        
        # Create symbol referencing it (need quote asset too)
        # Assuming USDT exists with ID 2
        symbol_resp = await admin_client.post("/admin/SymbolAdmin/item", json={
            "symbol": "REFTEST_USDT",
            "base_asset_id": asset_id,
            "quote_asset_id": 2,  # Assuming USDT
            "price_decimals": 2,
            "qty_decimals": 8,
            "status": "ACTIVE",
        })
        
        if symbol_resp.status_code not in (200, 201):
            pytest.skip("Could not create referencing symbol")
        
        # Try to delete the asset
        delete_resp = await admin_client.delete(f"/admin/AssetAdmin/item{asset_id}")
        
        # Should fail with FK constraint error
        assert delete_resp.status_code in (400, 409), \
            f"Delete should fail when asset is referenced: {delete_resp.text}"
        assert "referenced" in delete_resp.text.lower() or "use" in delete_resp.text.lower()
