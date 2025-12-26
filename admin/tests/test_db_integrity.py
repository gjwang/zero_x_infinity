import pytest
import asyncio
from sqlalchemy import text
from database import AsyncSessionLocal

@pytest.mark.asyncio
async def test_db_schema_integrity():
    """
    CRITICAL E2E: Verify that the actual PostgreSQL database matches the models.
    This test will fail if columns are missing in the real DB.
    """
    async with AsyncSessionLocal() as session:
        # Test symbols_tb
        try:
            await session.execute(text("SELECT base_maker_fee, base_taker_fee FROM symbols_tb LIMIT 1"))
            print("✅ symbols_tb schema matches models")
        except Exception as e:
            pytest.fail(f"❌ Database schema mismatch in symbols_tb: {e}")

        # Test admin_audit_log
        try:
            await session.execute(text("SELECT id, action, entity_type FROM admin_audit_log LIMIT 1"))
            print("✅ admin_audit_log table exists and matches models")
        except Exception as e:
            pytest.fail(f"❌ Database schema mismatch in admin_audit_log: {e}")

if __name__ == "__main__":
    # This script is meant to be run via pytest
    pass
