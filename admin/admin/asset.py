"""
Asset Admin CRUD
FastAPI Best Practice: Import schemas from centralized location
"""
from fastapi_amis_admin.admin import admin
from models import Asset
from schemas.asset import AssetCreateSchema, AssetUpdateSchema


class AssetAdmin(admin.ModelAdmin):
    """Admin interface for Asset management"""
    
    page_schema = admin.PageSchema(label="Assets", icon="fa fa-coins")
    pk_name = "asset_id"
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
    
    # Disable bulk operations for safety
    enable_bulk_create = False
    
    # Use optimized Pydantic schemas
    schema_create = AssetCreateSchema
    schema_update = AssetUpdateSchema
