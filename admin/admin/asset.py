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
    
    # Default ordering descending (UX-09)
    ordering = [Asset.asset_id.desc()]
    
    # Search fields
    search_fields = [Asset.asset, Asset.name]
    
    # Disable bulk operations for safety
    enable_bulk_create = False
    
    # Use optimized Pydantic schemas
    schema_create = AssetCreateSchema
    schema_update = AssetUpdateSchema

    def error_execute_sql(self, request: object, error: Exception):
        import sys
        import traceback
        print(f"DEBUG: SQL Error Caught in AssetAdmin: {error}")
        traceback.print_exc(file=sys.stdout)
        sys.stdout.flush()
        # Call super to maintain default behavior (raising 422/500)
        return super().error_execute_sql(request, error)
