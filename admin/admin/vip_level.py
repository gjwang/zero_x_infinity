"""
VIP Level Admin CRUD
AC-08, AC-10: Create, Edit, Default Normal
"""

from decimal import Decimal
from typing import Optional

from fastapi_amis_admin.admin import admin
from pydantic import BaseModel, field_validator, field_serializer

from models import VIPLevel


class VIPLevelCreateSchema(BaseModel):
    """Schema for creating/updating VIP Levels with validation"""
    level: int
    discount_percent: int = 100  # 100 = no discount
    min_volume: Optional[Decimal] = Decimal("0")
    description: Optional[str] = None
    
    @field_validator("level")
    @classmethod
    def validate_level(cls, v: int) -> int:
        """Level must be 0+"""
        if v < 0:
            raise ValueError("Level must be 0 or greater")
        return v
    
    @field_validator("discount_percent")
    @classmethod
    def validate_discount(cls, v: int) -> int:
        """Discount percent: 0-100 (0 = free, 100 = full price)"""
        if not 0 <= v <= 100:
            raise ValueError("Discount percent must be between 0 and 100")
        return v
    
    @field_serializer("min_volume")
    def serialize_min_volume(self, v: Optional[Decimal], _info) -> Optional[str]:
        """Serialize Decimal as String to prevent precision loss"""
        return str(v) if v is not None else None


class VIPLevelAdmin(admin.ModelAdmin):
    """Admin interface for VIP Level management"""
    
    page_schema = admin.PageSchema(label="VIP Levels", icon="fa fa-star")
    pk_name = "level"  # Specify primary key name
    model = VIPLevel
    
    # List columns
    list_display = [
        VIPLevel.level,
        VIPLevel.discount_percent,
        VIPLevel.min_volume,
        VIPLevel.description,
    ]
    
    # No bulk operations for VIP
    enable_bulk_create = False
    
    # Custom schemas with validation
    schema_create = VIPLevelCreateSchema
    schema_update = VIPLevelCreateSchema
