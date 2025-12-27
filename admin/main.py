"""
Admin Dashboard Main Entry Point
Phase 0x0F - Zero X Infinity

FastAPI Best Practices:
- Single PostgreSQL database
- Dependency injection for DB sessions
- Lifespan events for startup/shutdown
- Proper middleware ordering

Run with:
    uvicorn main:app --host 0.0.0.0 --port 8001
"""
from contextlib import asynccontextmanager

from fastapi import FastAPI
from fastapi_amis_admin.admin.settings import Settings as AdminSettings
from fastapi_amis_admin.admin.site import AdminSite

from settings import settings
from database import init_db, close_db
from admin import AssetAdmin, SymbolAdmin, VIPLevelAdmin, AuditLogAdmin
from auth import AuditLogMiddleware


@asynccontextmanager
async def lifespan(app: FastAPI):
    """
    Lifespan context manager
    Replaces deprecated @app.on_event("startup")
    """
    # Startup
    await init_db(settings.database_url)
    print(f"[Admin] Started at http://{settings.admin_host}:{settings.admin_port}/admin")
    print(f"[Admin] Database: PostgreSQL")
    yield
    # Shutdown
    await close_db()
    print("[Admin] Shutdown complete")


# Create FastAPI app with lifespan
app = FastAPI(
    title="Zero X Infinity Admin",
    version="0.0F",
    lifespan=lifespan,
)

# Add middleware BEFORE mounting (correct order)
app.add_middleware(AuditLogMiddleware)

# Create admin site with PostgreSQL
site = AdminSite(
    settings=AdminSettings(
        database_url_async=settings.database_url,
        secret_key=settings.admin_secret_key,
        site_title=settings.site_title,
        site_icon=settings.site_icon,
    ),
)

# Register admin pages
site.register_admin(AssetAdmin, SymbolAdmin, VIPLevelAdmin, AuditLogAdmin)

# CRITICAL: Add database middleware BEFORE mounting to properly wrap admin routes
# Without this, UPDATE operations won't persist to database!
app.add_middleware(site.db.asgi_middleware)

# Mount to app (AFTER adding db middleware)
site.mount_app(app)


@app.get("/health")
def health_check():
    """
    Health check endpoint
    Simple check without DB dependency for test compatibility
    """
    return {
        "status": "ok",
        "service": "admin-dashboard"
    }


if __name__ == "__main__":
    import uvicorn
    uvicorn.run(
        "main:app",
        host=settings.admin_host,
        port=settings.admin_port,
        reload=True,
    )
