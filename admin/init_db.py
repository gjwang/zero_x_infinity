#!/usr/bin/env python3
"""
Initialize admin database tables.
Run this ONCE before starting the server.

Usage:
    cd admin && source venv/bin/activate && python init_db.py
"""

import asyncio
import sys
import os

# Add current directory to path
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

async def init_db():
    """Initialize database tables"""
    from fastapi_amis_admin.admin.settings import Settings
    from fastapi_user_auth.admin import AuthAdminSite
    import settings as app_settings
    
    AUTH_DB_URL = "sqlite+aiosqlite:///./admin_auth.db"
    
    site = AuthAdminSite(
        settings=Settings(
            database_url_async=AUTH_DB_URL,
            secret_key=app_settings.ADMIN_SECRET_KEY,
            site_title=app_settings.SITE_TITLE,
            site_icon=app_settings.SITE_ICON,
        ),
    )
    
    # Create ALL auth tables
    await site.db.async_run_sync(site.auth.user_model.metadata.create_all, is_session=False)
    print("[Admin] Database tables created")
    
    # Create default admin user
    try:
        await site.auth.create_role_user(role_key="admin")
        print("[Admin] Created default admin user: admin/admin")
    except Exception as e:
        print(f"[Admin] Admin user exists or error: {e}")
    
    print("[Admin] Database initialized successfully!")
    print("\nNow start the server with:")
    print("  uvicorn main:app --host 0.0.0.0 --port 8001")


if __name__ == "__main__":
    asyncio.run(init_db())
