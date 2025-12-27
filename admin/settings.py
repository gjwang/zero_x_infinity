"""
Application Settings with Pydantic validation
FastAPI best practice: type-safe configuration with .env support
"""
from pydantic import Field
from pydantic_settings import BaseSettings, SettingsConfigDict


class Settings(BaseSettings):
    """Type-safe settings with automatic .env loading"""
    
    model_config = SettingsConfigDict(
        env_file=".env",
        env_file_encoding="utf-8",
        case_sensitive=False,
        extra="ignore",
    )
    
    # Database - defaults to local dev, CI sets DATABASE_URL_ASYNC
    database_url: str = Field(
        default="postgresql+asyncpg://trading:trading123@localhost:5433/exchange_info_db",
        validation_alias="DATABASE_URL_ASYNC"
    )
    
    # Security - MUST be set in production
    admin_secret_key: str = "dev-secret-key-change-in-production-32chars+"
    
    # Server - defaults OK for development
    admin_host: str = "0.0.0.0"
    admin_port: int = Field(default=8002, validation_alias="ADMIN_PORT")  # Dev default, CI overrides
    
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
