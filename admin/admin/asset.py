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
        """Handle SQL/validation errors with proper JSON-serializable messages"""
        import sys
        import traceback
        from fastapi import HTTPException
        
        # Log for debugging
        print(f"DEBUG: Error caught in AssetAdmin: {type(error).__name__}: {error}")
        traceback.print_exc(file=sys.stdout)
        sys.stdout.flush()
        
        # Convert error to string to avoid "ValueError is not JSON serializable"
        error_msg = str(error) if error else "Unknown error"
        
        # Raise HTTPException with string message (JSON-serializable)
        raise HTTPException(status_code=422, detail=error_msg)
