
import asyncio
import os
import sys
# Make sure we can import from current dir
sys.path.append(os.getcwd())

from settings import settings
from sqlalchemy.ext.asyncio import create_async_engine
from sqlalchemy import text

async def check():
    print(f"DEBUG: DATABASE_URL_ASYNC env var: {os.environ.get('DATABASE_URL_ASYNC')}")
    print(f"DEBUG: settings.database_url: {settings.database_url}")
    
    engine = create_async_engine(settings.database_url)
    async with engine.connect() as conn:
        print("Connected!")
        try:
            result = await conn.execute(text("SELECT current_schema()"))
            print(f"Current Schema: {result.scalar()}")
            
            result = await conn.execute(text("SELECT table_name FROM information_schema.tables WHERE table_schema='public'"))
            tables = result.scalars().all()
            print(f"Visible Tables: {tables}")

            result = await conn.execute(text("SELECT count(*) FROM assets_tb"))
            print(f"assets_tb count: {result.scalar()}")
        except Exception as e:
            print(f"assets_tb fetch failed: {e}")

        try:
            result = await conn.execute(text("SELECT count(*) FROM symbols_tb"))
            print(f"symbols_tb count: {result.scalar()}")
        except Exception as e:
            print(f"symbols_tb fetch failed: {e}")

if __name__ == "__main__":
    asyncio.run(check())
