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
    
    status: Annotated[int, Field(
        default=1,  # 1 = ACTIVE
        ge=0,
        le=1,
        description="Status: ACTIVE (1) or DISABLED (0)"
    )]
    
    @field_validator('status', mode='before')
    @classmethod
    def validate_status(cls, v):
        """Accept string or int input and convert to integer for DB storage
        Note: Field(ge=0, le=1) handles range validation - no ValueError needed
        """
        if isinstance(v, str):
            mapping = {"ACTIVE": 1, "DISABLED": 0}
            return mapping.get(v.upper(), v)  # Return as-is if invalid, let Field validate
        return v  # Let Field constraints validate the value
    
    # Note: No serializer - keep status as int for DB compatibility
    
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
    
    status: Annotated[int, Field(
        ge=0,
        le=1,
        description="Status: ACTIVE (1) or DISABLED (0)"
    )]
    
    @field_validator('status', mode='before')
    @classmethod
    def validate_status(cls, v):
        """Accept string or int input and convert to integer for DB storage
        Note: Field(ge=0, le=1) handles range validation - no ValueError needed
        """
        if isinstance(v, str):
            mapping = {"ACTIVE": 1, "DISABLED": 0}
            return mapping.get(v.upper(), v)  # Return as-is if invalid, let Field validate
        return v  # Let Field constraints validate the value
    
    # Note: No serializer - keep status as int for DB compatibility
    
    asset_flags: Annotated[int, Field(
        ge=0,
        description="Feature flags bitmap"
    )]
