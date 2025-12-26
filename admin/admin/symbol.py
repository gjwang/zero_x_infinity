"""
Symbol Admin CRUD
AC-05, AC-06, AC-12: Create, Edit, Trading/Halt

IMPORTANT: Per id-specification.md, certain fields are IMMUTABLE after creation:
- symbol (trading pair name)
- base_asset_id (base asset reference)
- quote_asset_id (quote asset reference)
"""

import re
from typing import Any

from fastapi_amis_admin.admin import admin
from pydantic import BaseModel, field_validator
from starlette.requests import Request

from models import Symbol


class SymbolCreateSchema(BaseModel):
    """Schema for creating Symbols - all fields allowed"""
    symbol: str
    base_asset_id: int
    quote_asset_id: int
    price_decimals: int
    qty_decimals: int
    min_qty: int = 0
    status: int = 1
    symbol_flags: int = 15
    base_maker_fee: int = 1000  # 0.10%
    base_taker_fee: int = 2000  # 0.20%
    
    @field_validator("symbol")
    @classmethod
    def validate_symbol(cls, v: str) -> str:
        """Symbol must be uppercase letters and underscores only (e.g., BTC_USDT)"""
        v = v.upper()
        if not re.match(r"^[A-Z]+_[A-Z]+$", v):
            raise ValueError("Symbol must be in format BASE_QUOTE (e.g., BTC_USDT)")
        if len(v) > 32:
            raise ValueError("Symbol must be 32 characters or less")
        return v
    
    @field_validator("price_decimals", "qty_decimals")
    @classmethod
    def validate_decimals(cls, v: int) -> int:
        """Decimals must be 0-18"""
        if not 0 <= v <= 18:
            raise ValueError("Decimals must be between 0 and 18")
        return v
    
    @field_validator("status")
    @classmethod
    def validate_status(cls, v: int) -> int:
        """Status: 0=offline, 1=online, 2=close_only (per GAP-01)"""
        if v not in (0, 1, 2):
            raise ValueError("Status must be 0 (offline), 1 (online), or 2 (close-only)")
        return v
    
    @field_validator("base_maker_fee", "base_taker_fee")
    @classmethod
    def validate_fee(cls, v: int) -> int:
        """Fee rate: 0-10000 bps (0-100%), integer only (per GAP-06)"""
        if not 0 <= v <= 10000:
            raise ValueError("Fee must be between 0 and 10000 bps (0-100%)")
        return v


class SymbolUpdateSchema(BaseModel):
    """Schema for updating Symbols - IMMUTABLE fields excluded
    
    Per id-specification.md:
    - symbol: IMMUTABLE (cannot change trading pair name)
    - base_asset_id: IMMUTABLE (cannot change base asset)
    - quote_asset_id: IMMUTABLE (cannot change quote asset)
    
    Only mutable fields:
    - min_qty: minimum quantity can be adjusted
    - status: trading status (online/offline/close-only)
    - symbol_flags: feature flags
    - base_maker_fee: maker fee rate
    - base_taker_fee: taker fee rate
    
    NOTE: price_decimals and qty_decimals are also immutable
    (changing precision would break existing orders)
    """
    min_qty: int
    status: int
    symbol_flags: int
    base_maker_fee: int
    base_taker_fee: int
    
    @field_validator("status")
    @classmethod
    def validate_status(cls, v: int) -> int:
        """Status: 0=offline, 1=online, 2=close_only (per GAP-01)"""
        if v not in (0, 1, 2):
            raise ValueError("Status must be 0 (offline), 1 (online), or 2 (close-only)")
        return v
    
    @field_validator("base_maker_fee", "base_taker_fee")
    @classmethod
    def validate_fee(cls, v: int) -> int:
        """Fee rate: 0-10000 bps (0-100%), integer only (per GAP-06)"""
        if not 0 <= v <= 10000:
            raise ValueError("Fee must be between 0 and 10000 bps (0-100%)")
        return v


class SymbolAdmin(admin.ModelAdmin):
    """Admin interface for Symbol management"""
    
    page_schema = admin.PageSchema(label="Symbols", icon="fa fa-exchange-alt")
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
    
    # Search fields
    search_fields = [Symbol.symbol]
    
    # Enable actions
    enable_bulk_create = False
    
    # Custom schemas with validation
    # IMPORTANT: Different schemas for create vs update!
    schema_create = SymbolCreateSchema
    schema_update = SymbolUpdateSchema  # Only mutable fields

