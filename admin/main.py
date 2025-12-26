"""
Admin Dashboard Main Entry Point - WORKING VERSION
Phase 0x0F - Zero X Infinity

Solution: Use AdminSite properly with explicit fastapi mounting
Note: Auth disabled to avoid redirect loop (for development)

Run with:
    uvicorn main:app --host 0.0.0.0 --port 8001
"""

from fastapi import FastAPI
from fastapi_amis_admin.admin.settings import Settings
from fastapi_amis_admin.admin.site import AdminSite

import settings as app_settings
from admin import AssetAdmin, SymbolAdmin, VIPLevelAdmin, AuditLogAdmin
from auth import AuditLogMiddleware


# Use SQLite for admin backend
ADMIN_DB_URL = "sqlite+aiosqlite:///./admin_auth.db"

# Create FastAPI app first
app = FastAPI()

# Create admin site  
site = AdminSite(
    settings=Settings(
        database_url_async=ADMIN_DB_URL,
        secret_key=app_settings.ADMIN_SECRET_KEY,
        site_title=app_settings.SITE_TITLE,
        site_icon=app_settings.SITE_ICON,
    ),
)

# Register admin pages
site.register_admin(AssetAdmin, SymbolAdmin, VIPLevelAdmin, AuditLogAdmin)

# Mount to app (passing app explicitly)
site.mount_app(app)

# Add audit logging middleware
app.add_middleware(AuditLogMiddleware)


@app.get("/health")
async def health_check():
    """Health check endpoint"""
    return {"status": "ok", "service": "admin-dashboard"}


@app.on_event("startup")
async def on_startup():
    """Startup event"""
    print(f"[Admin Dashboard] Started at http://{app_settings.ADMIN_HOST}:{app_settings.ADMIN_PORT}/admin")
    print(f"[Admin Dashboard] Note: Authentication disabled for development testing")


if __name__ == "__main__":
    import uvicorn
    uvicorn.run(
        "main:app",
        host=app_settings.ADMIN_HOST,
        port=app_settings.ADMIN_PORT,
        reload=True,
    )
