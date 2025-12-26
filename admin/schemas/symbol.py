"""
Symbol Schemas - FastAPI Best Practice: Declarative Pydantic Validation
UX-08: Status displayed as human-readable strings
"""
from pydantic import BaseModel, Field, field_validator, field_serializer, model_validator
from typing import Annotated
from enum import IntEnum


class SymbolStatus(IntEnum):
    """Symbol trading status"""
    OFFLINE = 0
    ONLINE = 1
    CLOSE_ONLY = 2


class SymbolCreateSchema(BaseModel):
    """
    Schema for creating Symbols
    Uses Pydantic Field() and IntEnum for declarative validation
    """
    
    symbol: Annotated[str, Field(
        max_length=32,
        pattern=r"^[A-Z0-9]+_[A-Z0-9]+$",
        description="Symbol in BASE_QUOTE format (e.g., BTC_USDT)",
        examples=["BTC_USDT", "ETH_BTC", "ETH2_USDT"],
    )]
    
    base_asset_id: Annotated[int, Field(
        gt=0,
        description="Base asset ID (must exist in assets_tb)"
    )]
    
    quote_asset_id: Annotated[int, Field(
        gt=0,
        description="Quote asset ID (must exist in assets_tb)"
    )]
    
    price_decimals: Annotated[int, Field(
        ge=0,
        le=18,
        description="Price precision decimals"
    )]
    
    qty_decimals: Annotated[int, Field(
        ge=0,
        le=18,
        description="Quantity precision decimals"
    )]
    
    min_qty: Annotated[int, Field(
        default=0,
        ge=0,
        description="Minimum order quantity"
    )]
    
    status: Annotated[SymbolStatus, Field(
        default=SymbolStatus.ONLINE,
        description="Trading status: ONLINE, OFFLINE, or CLOSE_ONLY"
    )]
    
    @field_validator('status', mode='before')
    @classmethod
    def validate_status(cls, v):
        """Accept string input ONLY (UX-08) - reject integers"""
        if not isinstance(v, str):
            raise ValueError(f"Status must be a string (ONLINE, OFFLINE, or CLOSE_ONLY), got: {type(v).__name__}")
        normalized = v.upper().replace('-', '_')
        try:
            return SymbolStatus[normalized]
        except KeyError:
            raise ValueError(f"Status must be ONLINE, OFFLINE, or CLOSE_ONLY, got: {v}")
    
    @field_serializer('status')
    def serialize_status(self, value: SymbolStatus) -> str:
        """Display status as string (UX-08)"""
        return value.name  # "ONLINE", "OFFLINE", or "CLOSE_ONLY"
    
    symbol_flags: Annotated[int, Field(
        default=15,
        ge=0,
        description="Feature flags bitmap"
    )]
    
    base_maker_fee: Annotated[int, Field(
        default=1000,
        ge=0,
        le=10000,
        description="Maker fee in basis points (1000 = 0.10%)"
    )]
    
    base_taker_fee: Annotated[int, Field(
        default=2000,
        ge=0,
        le=10000,
        description="Taker fee in basis points (2000 = 0.20%)"
    )]
    
    # Transformation logic only
    @field_validator("symbol", mode="before")
    @classmethod
    def uppercase_symbol(cls, v: str) -> str:
        """Convert to uppercase before validation"""
        return v.upper() if isinstance(v, str) else v
    
    # Business logic validation (not expressible in Field())
    @model_validator(mode='after')
    def validate_base_not_equal_quote(self):
        """Ensure base_asset_id != quote_asset_id (BUG-07 fix)"""
        if self.base_asset_id == self.quote_asset_id:
            raise ValueError("base_asset_id cannot equal quote_asset_id")
        return self


class SymbolUpdateSchema(BaseModel):
    """
    Schema for updating Symbols
    IMMUTABLE fields excluded: symbol, base_asset_id, quote_asset_id, decimals
    """
    
    min_qty: Annotated[int, Field(
        ge=0,
        description="Minimum order quantity"
    )]
    
    status: Annotated[SymbolStatus, Field(
        description="Trading status: ONLINE, OFFLINE, or CLOSE_ONLY"
    )]
    
    @field_validator('status', mode='before')
    @classmethod
    def validate_status(cls, v):
        """Accept string input ONLY (UX-08) - reject integers"""
        if not isinstance(v, str):
            raise ValueError(f"Status must be a string (ONLINE, OFFLINE, or CLOSE_ONLY), got: {type(v).__name__}")
        normalized = v.upper().replace('-', '_')
        try:
            return SymbolStatus[normalized]
        except KeyError:
            raise ValueError(f"Status must be ONLINE, OFFLINE, or CLOSE_ONLY, got: {v}")
    
    @field_serializer('status')
    def serialize_status(self, value: SymbolStatus) -> str:
        """Display status as string (UX-08)"""
        return value.name
    
    symbol_flags: Annotated[int, Field(
        ge=0,
        description="Feature flags bitmap"
    )]
    
    base_maker_fee: Annotated[int, Field(
        ge=0,
        le=10000,
        description="Maker fee in basis points"
    )]
    
    base_taker_fee: Annotated[int, Field(
        ge=0,
        le=10000,
        description="Taker fee in basis points"
    )]
