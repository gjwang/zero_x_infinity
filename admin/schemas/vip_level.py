"""
VIP Level Schemas - FastAPI Best Practice
"""
from pydantic import BaseModel, Field
from typing import Annotated
from decimal import Decimal


class VIPLevelCreateSchema(BaseModel):
    """Schema for creating VIP Levels"""
    
    level: Annotated[int, Field(
        ge=0,
        description="VIP level (0=normal, higher=better)"
    )]
    
    discount_percent: Annotated[int, Field(
        default=100,
        ge=0,
        le=100,
        description="Fee discount percentage (100=no discount, 0=free)"
    )]
    
    min_volume: Annotated[Decimal, Field(
        default=Decimal("0"),
        ge=0,
        max_digits=30,
        decimal_places=8,
        description="Minimum 30-day volume required"
    )]
    
    description: Annotated[str | None, Field(
        default=None,
        max_length=64,
        description="VIP level description"
    )] = None


class VIPLevelUpdateSchema(BaseModel):
    """Schema for updating VIP Levels"""
    
    discount_percent: Annotated[int, Field(
        ge=0,
        le=100,
        description="Fee discount percentage"
    )]
    
    min_volume: Annotated[Decimal, Field(
        ge=0,
        max_digits=30,
        decimal_places=8,
        description="Minimum volume threshold"
    )]
    
    description: Annotated[str | None, Field(
        max_length=64,
        description="Description"
    )] = None
