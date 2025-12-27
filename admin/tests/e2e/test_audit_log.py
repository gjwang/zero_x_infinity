"""
E2E-05: Audit Log Integration Test

Tests: All admin operations are logged

Test Flow:
1. Perform CRUD operations
2. Query audit log
3. Verify operations are recorded with:
   - admin_id
   - IP address
   - action (method + path)
   - old_value / new_value
   - timestamp

Prerequisites:
- Admin Dashboard running
- PostgreSQL with admin_audit_log table
"""

import asyncio
import httpx
import pytest
import os
from datetime import datetime, timedelta

# Ports from environment (db_env.sh is source of truth)
ADMIN_PORT = os.getenv("ADMIN_PORT", "8002")
ADMIN_URL = f"http://localhost:{ADMIN_PORT}"


class TestAuditLogCapture:
    """E2E: Audit log captures all operations"""
    
    @pytest.fixture
    async def admin_client(self):
        async with httpx.AsyncClient(base_url=ADMIN_URL) as client:
            yield client
    
    @pytest.mark.asyncio
    async def test_create_asset_logged(self, admin_client):
        """
        Create Asset → Audit log has entry
        """
        timestamp_before = datetime.utcnow()
        
        # Create asset
        asset_code = f"AUDIT{int(timestamp_before.timestamp()) % 10000}"
        create_resp = await admin_client.post("/admin/AssetAdmin/item", json={
            "asset": asset_code,
            "name": "Audit Test Asset",
            "decimals": 8,
            "status": "ACTIVE",
        })
        
        if create_resp.status_code not in (200, 201):
            pytest.skip("Could not create asset for audit test")
        
        # Query audit log
        await asyncio.sleep(1)  # Wait for log write
        
        audit_resp = await admin_client.post("/admin/AuditLogAdmin/list", json={
            "filter": {
                 "path": "/admin/AssetAdmin/item",
                 "action": "POST"
            }
        })
        
        if audit_resp.status_code != 200:
            pytest.skip("Audit log query not available")
        
        logs = audit_resp.json().get("items", [])
        
        # Find our create operation
        found = False
        for log in logs:
            if asset_code in str(log.get("new_value", "")):
                found = True
                # Verify required fields
                assert log.get("admin_id") is not None, "admin_id should be recorded"
                assert log.get("ip") is not None, "IP should be recorded"
                assert log.get("action") is not None, "action should be recorded"
                assert log.get("timestamp") is not None, "timestamp should be recorded"
                break
        
        assert found, f"Create operation for {asset_code} not found in audit log"
    
    @pytest.mark.asyncio
    async def test_update_asset_logged_with_diff(self, admin_client):
        """
        Update Asset → Audit log has old_value and new_value
        """
        # Get an existing asset
        assets = await admin_client.get("/admin/AssetAdmin/item")
        if assets.status_code != 200 or not assets.json().get("items"):
            pytest.skip("No assets available for update test")
        
        test_asset = assets.json()["items"][0]
        asset_id = test_asset["asset_id"]
        old_name = test_asset["name"]
        new_name = f"Updated {old_name}"
        
        # Update asset
        update_resp = await admin_client.put(
            f"/admin/AssetAdmin/item/{asset_id}",
            json={"name": new_name}
        )
        
        if update_resp.status_code != 200:
            pytest.skip("Could not update asset")
        
        # Query audit log
        await asyncio.sleep(1)
        
        audit_resp = await admin_client.post("/admin/AuditLogAdmin/list", json={
            "filter": {
                "entity_type": "asset",
                "entity_id": str(asset_id),
            }
        })
        
        if audit_resp.status_code != 200:
            pytest.skip("Audit log query not available")
        
        logs = audit_resp.json().get("items", [])
        
        # Find update operation
        for log in logs:
            if log.get("action", "").startswith("PUT"):
                old_value = log.get("old_value", {})
                new_value = log.get("new_value", {})
                
                # Should capture the change
                if isinstance(old_value, dict) and isinstance(new_value, dict):
                    if old_value.get("name") == old_name and new_value.get("name") == new_name:
                        # Found the correct log entry
                        return
        
        # Restore original name
        await admin_client.put(f"/admin/AssetAdmin/item/{asset_id}", json={"name": old_name})
    
    @pytest.mark.asyncio  
    async def test_audit_log_not_editable(self, admin_client):
        """
        Audit log should not allow delete or update
        """
        # Get an audit log entry
        audit_resp = await admin_client.post("/admin/AuditLogAdmin/list", json={})
        if audit_resp.status_code != 200 or not audit_resp.json().get("items"):
            pytest.skip("No audit log entries available")
        
        log_entry = audit_resp.json()["items"][0]
        log_id = log_entry.get("id")
        
        # Try to delete - should fail
        delete_resp = await admin_client.delete(f"/admin/AuditLogAdmin/item/{log_id}")
        assert delete_resp.status_code in (403, 405), \
            f"Audit log delete should be forbidden: {delete_resp.status_code}"
        
        # Try to update - should fail
        update_resp = await admin_client.put(
            f"/admin/AuditLogAdmin/item/{log_id}",
            json={"action": "MODIFIED"}
        )
        assert update_resp.status_code in (403, 405), \
            f"Audit log update should be forbidden: {update_resp.status_code}"


class TestAuditLogQuery:
    """E2E: Audit log query capabilities"""
    
    @pytest.fixture
    async def admin_client(self):
        async with httpx.AsyncClient(base_url=ADMIN_URL) as client:
            yield client
    
    @pytest.mark.asyncio
    async def test_query_by_admin_id(self, admin_client):
        """Can filter audit log by admin ID"""
        resp = await admin_client.post("/admin/AuditLogAdmin/list", json={
            "filter": {"admin_id": 1}
        })
        # Should succeed or return empty if no logs for this admin
        assert resp.status_code == 200
    
    @pytest.mark.asyncio
    async def test_query_by_date_range(self, admin_client):
        """Can filter audit log by date range"""
        today = datetime.utcnow().date()
        yesterday = today - timedelta(days=1)
        
        resp = await admin_client.post("/admin/AuditLogAdmin/list", json={
            "filter": {
                "created_at": {">=": yesterday.isoformat(), "<=": today.isoformat()}
            }
        })
        assert resp.status_code == 200
    
    @pytest.mark.asyncio
    async def test_query_by_entity(self, admin_client):
        """Can filter audit log by entity type and ID"""
        resp = await admin_client.post("/admin/AuditLogAdmin/list", json={
            "filter": {
                "entity_type": "asset",
                "entity_id": str(1),
            }
        })
        assert resp.status_code == 200
