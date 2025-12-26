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

    def error_execute_sql(self, request: object, error: Exception):
        """Handle SQL/validation errors with proper JSON-serializable messages"""
        import sys
        import traceback
        from fastapi import HTTPException
        
        # Log for debugging
        print(f"DEBUG: Error caught in SymbolAdmin: {type(error).__name__}: {error}")
        traceback.print_exc(file=sys.stdout)
        sys.stdout.flush()
        
        # Convert error to string to avoid "ValueError is not JSON serializable"
        error_msg = str(error) if error else "Unknown error"
        
        # Raise HTTPException with string message (JSON-serializable)
        raise HTTPException(status_code=422, detail=error_msg)
