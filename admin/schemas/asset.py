"""
Asset Schemas - FastAPI Best Practice: Declarative Pydantic Validation
UX-08: Status displayed as human-readable strings
"""
from pydantic import BaseModel, Field, field_validator, field_serializer
from typing import Annotated
from enum import IntEnum


class AssetStatus(IntEnum):
    """Asset operational status"""
    DISABLED = 0  # 🔴 Red
    ACTIVE = 1    # 🟢 Green


class AssetCreateSchema(BaseModel):
    """
    Schema for creating Assets
    Uses Pydantic Field() for declarative validation
    """
    
    asset: Annotated[str, Field(
        min_length=1,
        max_length=16,
        pattern=r"^[A-Z0-9_]+$",
        description="Asset code (uppercase letters, numbers, underscore)",
        examples=["BTC", "ETH", "USDT"],
    )]
    
    name: Annotated[str, Field(
        max_length=256,
        description="Asset display name"
    )]
    
    decimals: Annotated[int, Field(
        ge=0,
        le=18,
        description="Precision decimals (0-18)"
    )]
    
    status: Annotated[AssetStatus, Field(
        default=AssetStatus.ACTIVE,
        description="Status: ACTIVE or DISABLED"
    )]
    
    @field_validator('status', mode='before')
    @classmethod
    def validate_status(cls, v):
        """Accept string or int input (UX-08)"""
        if isinstance(v, str):
            try:
                return AssetStatus[v.upper()]
            except KeyError:
                raise ValueError(f"Status must be ACTIVE or DISABLED, got: {v}")
        try:
            return AssetStatus(v)
        except ValueError:
            raise ValueError(f"Status must be ACTIVE or DISABLED, got: {v}")
    
    @field_serializer('status')
    def serialize_status(self, value: AssetStatus) -> str:
        """Display status as string (UX-08)"""
        return value.name  # "ACTIVE" or "DISABLED"
    
    asset_flags: Annotated[int, Field(
        default=7,
        ge=0,
        description="Feature flags bitmap"
    )]
    
    # Only keep field_validator for transformations, not validation
    @field_validator("asset", mode="before")
    @classmethod
    def uppercase_asset(cls, v: str) -> str:
        """Convert to uppercase before validation"""
        return v.upper() if isinstance(v, str) else v


class AssetUpdateSchema(BaseModel):
    """
    Schema for updating Assets
    IMMUTABLE fields excluded: asset, decimals
    """
    
    name: Annotated[str, Field(
        max_length=256,
        description="Asset display name"
    )]
    
    status: Annotated[AssetStatus, Field(
        description="Status: ACTIVE or DISABLED"
    )]
    
    @field_validator('status', mode='before')
    @classmethod
    def validate_status(cls, v):
        """Accept string or int input (UX-08)"""
        if isinstance(v, str):
            try:
                return AssetStatus[v.upper()]
            except KeyError:
                raise ValueError(f"Status must be ACTIVE or DISABLED, got: {v}")
        try:
            return AssetStatus(v)
        except ValueError:
            raise ValueError(f"Status must be ACTIVE or DISABLED, got: {v}")
    
    @field_serializer('status')
    def serialize_status(self, value: AssetStatus) -> str:
        """Display status as string (UX-08)"""
        return value.name
    
    asset_flags: Annotated[int, Field(
        ge=0,
        description="Feature flags bitmap"
    )]
