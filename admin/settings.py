"""
Application Settings with Pydantic validation
FastAPI best practice: type-safe configuration with .env support
"""
from pydantic_settings import BaseSettings, SettingsConfigDict


class Settings(BaseSettings):
    """Type-safe settings with automatic .env loading"""
    
    model_config = SettingsConfigDict(
        env_file=".env",
        env_file_encoding="utf-8",
        case_sensitive=False,
        extra="ignore",
    )
    
    # Database - MUST be set via environment variable (CI standard)
    # Source: db_env.sh exports DATABASE_URL_ASYNC
    database_url: str
    
    # Security - MUST be set in production
    admin_secret_key: str = "dev-secret-key-change-in-production-32chars+"
    
    # Server - defaults OK for development
    admin_host: str = "0.0.0.0"
    admin_port: int = 8001
    
    # Session security (per GAP-05)
    access_token_expire_minutes: int = 15
    refresh_token_expire_hours: int = 24
    idle_timeout_minutes: int = 30
    
    # Site branding
    site_title: str = "Zero X Infinity Admin"
    site_icon: str = "https://raw.githubusercontent.com/gjwang/zero_x_infinity/main/docs/assets/logo.png"
    
    # Default admin credentials - OK for development
    default_admin_username: str = "admin"
    default_admin_password: str = "admin123"


# Global settings instance
settings = Settings()
