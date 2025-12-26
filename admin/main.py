"""
Admin Dashboard Main Entry Point
Phase 0x0F - Zero X Infinity

Run with:
    uvicorn main:app --host 0.0.0.0 --port 8001 --reload
"""

from contextlib import asynccontextmanager

from fastapi import FastAPI
from fastapi_amis_admin.admin.settings import Settings
from fastapi_user_auth.admin import AuthAdminSite

import settings as app_settings
from models import Base
from admin import AssetAdmin, SymbolAdmin, VIPLevelAdmin, AuditLogAdmin
from auth import AuditLogMiddleware


# Create admin site with authentication
site = AuthAdminSite(
    settings=Settings(
        database_url_async=app_settings.DATABASE_URL,
        secret_key=app_settings.ADMIN_SECRET_KEY,
        site_title=app_settings.SITE_TITLE,
        site_icon=app_settings.SITE_ICON,
    ),
)


# Register admin pages
site.register_admin(AssetAdmin, SymbolAdmin, VIPLevelAdmin, AuditLogAdmin)


# Create FastAPI app
app = site.fastapi


# Add audit logging middleware
app.add_middleware(AuditLogMiddleware)


@app.get("/health")
async def health_check():
    """Health check endpoint"""
    return {"status": "ok", "service": "admin-dashboard"}


@app.on_event("startup")
async def on_startup():
    """Startup event: create default admin user if not exists"""
    await site.db.async_run_sync(Base.metadata.create_all, is_session=False)
    # Create default admin user
    await site.auth.create_role_user(
        role_key="admin",
        user={
            "username": app_settings.DEFAULT_ADMIN_USERNAME,
            "password": app_settings.DEFAULT_ADMIN_PASSWORD,
        },
    )
    print(f"[Admin Dashboard] Started at http://{app_settings.ADMIN_HOST}:{app_settings.ADMIN_PORT}/admin")
    print(f"[Admin Dashboard] Default login: {app_settings.DEFAULT_ADMIN_USERNAME} / {app_settings.DEFAULT_ADMIN_PASSWORD}")


if __name__ == "__main__":
    import uvicorn
    uvicorn.run(
        "main:app",
        host=app_settings.ADMIN_HOST,
        port=app_settings.ADMIN_PORT,
        reload=True,
    )
