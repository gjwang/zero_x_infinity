"""
Admin Dashboard Main Entry Point
Phase 0x0F - Zero X Infinity

Run with:
    uvicorn main:app --host 0.0.0.0 --port 8001 --reload
"""

from fastapi import FastAPI
from fastapi_amis_admin.admin.settings import Settings
from fastapi_user_auth.admin import AuthAdminSite

import settings as app_settings
from admin import AssetAdmin, SymbolAdmin, VIPLevelAdmin, AuditLogAdmin
from auth import AuditLogMiddleware


# Use SQLite for auth backend (simple, built-in async support)
AUTH_DB_URL = "sqlite+aiosqlite:///./admin_auth.db"

# Create admin site with authentication
site = AuthAdminSite(
    settings=Settings(
        database_url_async=AUTH_DB_URL,  # Auth uses SQLite
        secret_key=app_settings.ADMIN_SECRET_KEY,
        site_title=app_settings.SITE_TITLE,
        site_icon=app_settings.SITE_ICON,
    ),
)


# Register admin pages
site.register_admin(AssetAdmin, SymbolAdmin, VIPLevelAdmin, AuditLogAdmin)

# Mount the site to the app (this is the key step!)
site.mount_app(site.fastapi)

# Get the FastAPI app
app = site.fastapi

# Add audit logging middleware
app.add_middleware(AuditLogMiddleware)


@app.get("/health")
async def health_check():
    """Health check endpoint"""
    return {"status": "ok", "service": "admin-dashboard"}


@app.on_event("startup")
async def on_startup():
    """Startup event: create tables and default admin user"""
    # Create auth tables (SQLite)
    await site.db.async_run_sync(site.auth.user_model.__table__.metadata.create_all, is_session=False)
    
    # Create default admin user using create_role_user
    try:
        await site.auth.create_role_user(role_key="admin")
        print(f"[Admin] Created default admin user")
    except Exception as e:
        # User may already exist
        print(f"[Admin] Admin user already exists or error: {e}")
    
    print(f"[Admin Dashboard] Started at http://{app_settings.ADMIN_HOST}:{app_settings.ADMIN_PORT}/admin")
    print(f"[Admin Dashboard] Login: admin / admin (default, first login)")


if __name__ == "__main__":
    import uvicorn
    uvicorn.run(
        "main:app",
        host=app_settings.ADMIN_HOST,
        port=app_settings.ADMIN_PORT,
        reload=True,
    )
