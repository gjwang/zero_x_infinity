"""
Symbol Admin CRUD
FastAPI Best Practice: Import schemas from centralized location
"""
from fastapi_amis_admin.admin import admin
from models import Symbol
from schemas.symbol import SymbolCreateSchema, SymbolUpdateSchema


class SymbolAdmin(admin.ModelAdmin):
    """Admin interface for Symbol management"""
    
    page_schema = admin.PageSchema(label="Symbols", icon="fa fa-exchange-alt")
    pk_name = "symbol_id"
    model = Symbol
    
    # List columns
    list_display = [
        Symbol.symbol_id,
        Symbol.symbol,
        Symbol.base_asset_id,
        Symbol.quote_asset_id,
        Symbol.price_decimals,
        Symbol.qty_decimals,
        Symbol.status,
        Symbol.base_maker_fee,
        Symbol.base_taker_fee,
        Symbol.created_at,
    ]
    
    # Default ordering descending (UX-09)
    ordering = [Symbol.symbol_id.desc()]
    
    # Search fields
    search_fields = [Symbol.symbol]
    
    # Disable bulk operations
    enable_bulk_create = False
    
    # Use optimized Pydantic schemas
    schema_create = SymbolCreateSchema
    schema_update = SymbolUpdateSchema
    
    # Enable field updates (CRITICAL: defaults to empty list = no updates!)
    update_fields = [
        Symbol.min_qty,
        Symbol.status,
        Symbol.symbol_flags,
        Symbol.base_maker_fee,
        Symbol.base_taker_fee,
    ]
