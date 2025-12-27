"""
Database connection with dependency injection
FastAPI best practice: single source of truth for DB sessions
"""
from typing import AsyncGenerator
from sqlalchemy.ext.asyncio import (
    AsyncEngine,
    AsyncSession,
    create_async_engine,
    async_sessionmaker,
)

# Global engine and session maker
engine: AsyncEngine | None = None
SessionLocal: async_sessionmaker[AsyncSession] | None = None

def AsyncSessionLocal(*args, **kwargs):
    """Alias for SessionLocal - used in E2E tests"""
    if SessionLocal is None:
        raise RuntimeError("Database not initialized. Call init_db() first.")
    return SessionLocal(*args, **kwargs)


async def init_db(database_url: str):
    """
    Initialize database connection pool
    Called in lifespan startup
    """
    global engine, SessionLocal
    
    engine = create_async_engine(
        database_url,
        echo=True,
        pool_size=20,
        max_overflow=40,
        pool_pre_ping=True,
        pool_recycle=3600,
    )
    
    SessionLocal = async_sessionmaker(
        engine,
        class_=AsyncSession,
        expire_on_commit=False,
    )
    
    print(f"[DB] Connection pool initialized: {database_url.split('@')[1].split('/')[0]}")


async def close_db():
    """
    Close database connection pool
    Called in lifespan shutdown
    """
    global engine
    
    if engine:
        await engine.dispose()
        print("[DB] Connection pool closed")


async def get_db() -> AsyncGenerator[AsyncSession, None]:
    """
    Dependency: Get database session
    
    Usage:
        from database import get_db
        from fastapi import Depends
        
        @app.get("/items")
        async def list_items(db: AsyncSession = Depends(get_db)):
            result = await db.execute(select(Item))
            return result.scalars().all()
    """
    if SessionLocal is None:
        raise RuntimeError("Database not initialized. Call init_db() first.")
    
    async with SessionLocal() as session:
        try:
            yield session
            await session.commit()
        except Exception:
            await session.rollback()
            raise
