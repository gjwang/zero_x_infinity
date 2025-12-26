"""
Admin Dashboard Settings
Phase 0x0F - Zero X Infinity
"""

import os
from pathlib import Path

# Base directory
BASE_DIR = Path(__file__).resolve().parent

# Database
DATABASE_URL = os.getenv(
    "DATABASE_URL",
    "postgresql+asyncpg://postgres:postgres@localhost:5432/exchange_info_db"
)

# Admin settings
ADMIN_SECRET_KEY = os.getenv("ADMIN_SECRET_KEY", "change-me-in-production-0x0F")
ADMIN_HOST = os.getenv("ADMIN_HOST", "0.0.0.0")
ADMIN_PORT = int(os.getenv("ADMIN_PORT", "8001"))

# Default super admin
DEFAULT_ADMIN_USERNAME = os.getenv("DEFAULT_ADMIN_USERNAME", "admin")
DEFAULT_ADMIN_PASSWORD = os.getenv("DEFAULT_ADMIN_PASSWORD", "admin123")

# Site config
SITE_TITLE = "Zero X Infinity Admin"
SITE_ICON = "https://raw.githubusercontent.com/gjwang/zero_x_infinity/main/docs/assets/logo.png"
