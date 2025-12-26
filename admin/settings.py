"""
Admin Dashboard Settings
Phase 0x0F - Zero X Infinity
"""

import os
from pathlib import Path
from dataclasses import dataclass


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


@dataclass
class Settings:
    """Application settings with GAP-05 security configuration"""
    
    # Database
    DATABASE_URL: str = DATABASE_URL
    
    # Secret key (for JWT)
    SECRET_KEY: str = ADMIN_SECRET_KEY
    
    # Session security (per GAP-05)
    ACCESS_TOKEN_EXPIRE_MINUTES: int = 15  # Access token expiry
    REFRESH_TOKEN_EXPIRE_HOURS: int = 24   # Refresh token expiry
    IDLE_TIMEOUT_MINUTES: int = 30         # Idle timeout
    
    # Sensitive operations requiring re-auth (per GAP-05)
    REAUTH_REQUIRED_OPS: list = None
    
    def __post_init__(self):
        if self.REAUTH_REQUIRED_OPS is None:
            self.REAUTH_REQUIRED_OPS = [
                "asset_disable",
                "symbol_halt",
                "vip_modify",
            ]


# Global settings instance
settings = Settings()

