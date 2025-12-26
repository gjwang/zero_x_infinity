"""
VIP Level Admin CRUD
FastAPI Best Practice: Import schemas from centralized location
"""
from fastapi_amis_admin.admin import admin
from models import VIPLevel
from schemas.vip_level import VIPLevelCreateSchema, VIPLevelUpdateSchema


class VIPLevelAdmin(admin.ModelAdmin):
    """Admin interface for VIP Level management"""
    
    page_schema = admin.PageSchema(label="VIP Levels", icon="fa fa-star")
    pk_name = "level"
    model = VIPLevel
    
    # List columns
    list_display = [
        VIPLevel.level,
        VIPLevel.discount_percent,
        VIPLevel.min_volume,
        VIPLevel.description,
    ]
    
    # Disable bulk operations
    enable_bulk_create = False
    
    # Use optimized Pydantic schemas
    schema_create = VIPLevelCreateSchema
    schema_update = VIPLevelUpdateSchema
