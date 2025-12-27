import pytest
import asyncio
from httpx import AsyncClient, ASGITransport
from main import app
from database import init_db, AsyncSessionLocal
from settings import settings
from sqlalchemy import text
from datetime import datetime

@pytest.mark.asyncio
async def test_audit_log_recording():
    """Verify that creating an asset triggers an audit log entry"""
    # 1. Initialize DB
    await init_db(settings.database_url)
    
    # 2. Use AsyncClient with the real app (triggers middleware)
    transport = ASGITransport(app=app)
    async with AsyncClient(transport=transport, base_url="http://test") as client:
        # Create a unique asset for this test
        asset_code = f"AUDIT_{int(datetime.now().timestamp())}"
        asset_data = {
            "asset": asset_code,
            "name": "Audit Test Asset",
            "decimals": 8,
            "status": 1
        }
        
        print(f"Creating asset {asset_code}...")
        # Note: we use the internal API path that site.mount_app(app) creates
        # Typically fastapi-amis-admin mounts at /admin/api/
        resp = await client.post("/admin/api/asset/create", json=asset_data)
        assert resp.status_code in [200, 201], f"Failed to create asset: {resp.text}"
        
        # 3. Verify audit log entry exists in real DB
        async with AsyncSessionLocal() as session:
            await asyncio.sleep(0.5)  # Wait for middleware to finish commit
            result = await session.execute(
                text("SELECT action, path, entity_type FROM admin_audit_log WHERE path LIKE '%asset%' ORDER BY id DESC LIMIT 1")
            )
            row = result.fetchone()
            assert row is not None, "No audit log entry found!"
            print(f"✅ Audit Log Entry Found: {row.action} {row.path} {row.entity_type}")
            assert row.action == "POST"
            assert "asset" in row.path

if __name__ == "__main__":
    import sys
    import pytest
    sys.exit(pytest.main([__file__, "-v"]))
