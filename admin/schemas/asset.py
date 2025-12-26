"""
Asset Schemas - FastAPI Best Practice: Declarative Pydantic Validation
"""
from pydantic import BaseModel, Field, field_validator
from typing import Annotated


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
        ge=0,
        le=1,
        default=1,
        description="Status: 0=disabled, 1=active"
    )]
    
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
        description="Status: 0=disabled, 1=active"
    )]
    
    asset_flags: Annotated[int, Field(
        ge=0,
        description="Feature flags bitmap"
    )]
