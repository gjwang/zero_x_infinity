"""
Asset Admin CRUD
AC-02, AC-03, AC-11: Create, Edit, Enable/Disable
"""

import re
from typing import Any

from fastapi_amis_admin.admin import admin
from pydantic import BaseModel, field_validator
from starlette.requests import Request

from models import Asset


class AssetCreateSchema(BaseModel):
    """Schema for creating/updating Assets with validation"""
    asset: str
    name: str
    decimals: int
    status: int = 1
    asset_flags: int = 7
    
    @field_validator("asset")
    @classmethod
    def validate_asset(cls, v: str) -> str:
        """Asset must be uppercase letters only"""
        v = v.upper()
        if not re.match(r"^[A-Z]+$", v):
            raise ValueError("Asset must contain only uppercase letters")
        if len(v) > 16:
            raise ValueError("Asset must be 16 characters or less")
        return v
    
    @field_validator("decimals")
    @classmethod
    def validate_decimals(cls, v: int) -> int:
        """Decimals must be 0-18"""
        if not 0 <= v <= 18:
            raise ValueError("Decimals must be between 0 and 18")
        return v
    
    @field_validator("status")
    @classmethod
    def validate_status(cls, v: int) -> int:
        """Status: 0=disabled, 1=active"""
        if v not in (0, 1):
            raise ValueError("Status must be 0 (disabled) or 1 (active)")
        return v


class AssetAdmin(admin.ModelAdmin):
    """Admin interface for Asset management"""
    
    page_schema = admin.PageSchema(label="Assets", icon="fa fa-coins")
    model = Asset
    
    # List columns
    list_display = [
        Asset.asset_id,
        Asset.asset,
        Asset.name,
        Asset.decimals,
        Asset.status,
        Asset.asset_flags,
        Asset.created_at,
    ]
    
    # Search fields
    search_fields = [Asset.asset, Asset.name]
    
    # Enable actions
    enable_bulk_create = False  # Prevent bulk create for safety
    
    # Custom schemas with validation
    schema_create = AssetCreateSchema
    schema_update = AssetCreateSchema
