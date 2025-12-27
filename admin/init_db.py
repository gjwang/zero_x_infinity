
import asyncio
from sqlalchemy.ext.asyncio import create_async_engine
from settings import settings
from models import Base
import models # Force import to register models

async def main():
    print(f"Initializing database at: {settings.database_url.split('@')[1]}")
    engine = create_async_engine(settings.database_url, echo=True)

    async with engine.begin() as conn:
        print("Creating table schema...")
        await conn.run_sync(Base.metadata.create_all)
    
    print("Database initialized successfully.")
    await engine.dispose()

if __name__ == "__main__":
    asyncio.run(main())
