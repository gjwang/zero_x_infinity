
import asyncio
import os
import sys

# Ensure we can import settings
sys.path.append(os.path.dirname(os.path.abspath(__file__)))

from sqlalchemy.ext.asyncio import create_async_engine
from sqlalchemy import text
from settings import settings

async def cleanup():
    # Use the FIXED database URL (should be 127.0.0.1 if propagated correctly)
    # If not, fallback to hardcoded safe 127.0.0.1 for this script
    db_url = settings.database_url
    if "localhost" in db_url:
        db_url = db_url.replace("localhost", "127.0.0.1")
    
    print(f"Cleaning test data from: {db_url.split('@')[1]}")
    
    engine = create_async_engine(db_url)
    async with engine.begin() as conn:
        print("TRUNCATING ALL TABLES (assets, symbols, audit_log, vip_levels)...")
        await conn.execute(text("TRUNCATE TABLE assets_tb, symbols_tb, admin_audit_log, vip_levels_tb RESTART IDENTITY CASCADE"))
        
        result = await conn.execute(text("SELECT count(*) FROM assets_tb"))
        count = result.scalar()
        print(f"Assets count after truncate: {count}")
        
        print("Cleanup complete.")

if __name__ == "__main__":
    asyncio.run(cleanup())
