"""
Test Input Validation
AC-09: Reject invalid input (decimals<0, fee>100%)
"""

import pytest
from pydantic import ValidationError

from schemas.asset import AssetCreateSchema, AssetUpdateSchema
from schemas.symbol import SymbolCreateSchema, SymbolUpdateSchema
from schemas.vip_level import VIPLevelCreateSchema


class TestAssetValidation:
    """Test Asset input validation"""
    
    def test_valid_asset(self):
        """Valid asset should pass"""
        schema = AssetCreateSchema(
            asset="BTC",
            name="Bitcoin",
            decimals=8,
            status=1,
        )
        assert schema.asset == "BTC"
        assert schema.decimals == 8
    
    def test_asset_uppercase_conversion(self):
        """Asset should be converted to uppercase"""
        schema = AssetCreateSchema(
            asset="btc",
            name="Bitcoin",
            decimals=8,
        )
        assert schema.asset == "BTC"
    
    def test_invalid_asset_format(self):
        """Invalid asset format should be rejected"""
        with pytest.raises(ValidationError) as exc_info:
            AssetCreateSchema(
                asset="BTC-X",  # Hyphen not allowed
                name="Bitcoin",
                decimals=8,
            )
        # Pydantic Field(pattern=...) error message
        assert "pattern" in str(exc_info.value).lower() or "string_pattern_mismatch" in str(exc_info.value).lower()
    
    def test_invalid_decimals_negative(self):
        """Negative decimals should be rejected"""
        with pytest.raises(ValidationError) as exc_info:
            AssetCreateSchema(
                asset="BTC",
                name="Bitcoin",
                decimals=-1,
            )
        # Pydantic Field(ge=0) error message
        assert "greater than or equal" in str(exc_info.value).lower() or "ge=0" in str(exc_info.value).lower()
    
    def test_invalid_decimals_too_large(self):
        """Decimals > 18 should be rejected"""
        with pytest.raises(ValidationError) as exc_info:
            AssetCreateSchema(
                asset="BTC",
                name="Bitcoin",
                decimals=19,
            )
        # Pydantic Field(le=18) error message
        assert "less than or equal" in str(exc_info.value).lower() or "le=18" in str(exc_info.value).lower()
    
    def test_invalid_status(self):
        """Invalid status should be rejected"""
        with pytest.raises(ValidationError) as exc_info:
            AssetCreateSchema(
                asset="BTC",
                name="Bitcoin",
                decimals=8,
                status=3,  # Only 0 or 1 allowed
            )
        # Pydantic IntEnum error message (UX-08)
        assert ("input should be 0 or 1" in str(exc_info.value).lower() 
                or "type=enum" in str(exc_info.value).lower())


class TestSymbolValidation:
    """Test Symbol input validation"""
    
    def test_valid_symbol(self):
        """Valid symbol should pass"""
        schema = SymbolCreateSchema(
            symbol="BTC_USDT",
            base_asset_id=1,
            quote_asset_id=2,
            price_decimals=2,
            qty_decimals=8,
        )
        assert schema.symbol == "BTC_USDT"
    
    def test_symbol_uppercase_conversion(self):
        """Symbol should be converted to uppercase"""
        schema = SymbolCreateSchema(
            symbol="btc_usdt",
            base_asset_id=1,
            quote_asset_id=2,
            price_decimals=2,
            qty_decimals=8,
        )
        assert schema.symbol == "BTC_USDT"
    
    def test_invalid_symbol_format(self):
        """Invalid symbol format should be rejected"""
        with pytest.raises(ValidationError) as exc_info:
            SymbolCreateSchema(
                symbol="BTCUSDT",  # Missing underscore
                base_asset_id=1,
                quote_asset_id=2,
                price_decimals=2,
                qty_decimals=8,
            )
        # Pydantic Field(pattern=...) error message
        assert "pattern" in str(exc_info.value).lower() or "string_pattern_mismatch" in str(exc_info.value).lower()
    
    def test_invalid_fee_too_high(self):
        """Fee > 10000 bps should be rejected"""
        with pytest.raises(ValidationError) as exc_info:
            SymbolCreateSchema(
                symbol="BTC_USDT",
                base_asset_id=1,
                quote_asset_id=2,
                price_decimals=2,
                qty_decimals=8,
                base_maker_fee=10001,  # > 100%
            )
        # Pydantic Field(le=10000) error message
        assert "less than or equal" in str(exc_info.value).lower() or "le=10000" in str(exc_info.value).lower()
    
    def test_invalid_fee_negative(self):
        """Negative fee should be rejected"""
        with pytest.raises(ValidationError) as exc_info:
            SymbolCreateSchema(
                symbol="BTC_USDT",
                base_asset_id=1,
                quote_asset_id=2,
                price_decimals=2,
                qty_decimals=8,
                base_taker_fee=-1,
            )
        # Pydantic Field(ge=0) error message
        assert "greater than or equal" in str(exc_info.value).lower() or "ge=0" in str(exc_info.value).lower()


class TestVIPLevelValidation:
    """Test VIP Level input validation"""
    
    def test_valid_vip_level(self):
        """Valid VIP level should pass"""
        schema = VIPLevelCreateSchema(
            level=0,
            discount_percent=100,
            description="Normal",
        )
        assert schema.level == 0
        assert schema.discount_percent == 100
    
    def test_invalid_level_negative(self):
        """Negative level should be rejected"""
        with pytest.raises(ValidationError) as exc_info:
            VIPLevelCreateSchema(
                level=-1,
                discount_percent=100,
            )
        # Pydantic Field(ge=0) error message
        assert "greater than or equal" in str(exc_info.value).lower() or "ge=0" in str(exc_info.value).lower()
    
    def test_invalid_discount_too_high(self):
        """Discount > 100 should be rejected"""
        with pytest.raises(ValidationError) as exc_info:
            VIPLevelCreateSchema(
                level=1,
                discount_percent=101,
            )
        # Pydantic Field(le=100) error message
        assert "less than or equal" in str(exc_info.value).lower() or "le=100" in str(exc_info.value).lower()
    
    def test_invalid_discount_negative(self):
        """Negative discount should be rejected"""
        with pytest.raises(ValidationError) as exc_info:
            VIPLevelCreateSchema(
                level=1,
                discount_percent=-1,
            )
        # Pydantic Field(ge=0) error message
        assert "greater than or equal" in str(exc_info.value).lower() or "ge=0" in str(exc_info.value).lower()


class TestAssetImmutability:
    """Test Asset field immutability (per id-specification.md)"""
    
    def test_update_schema_excludes_asset(self):
        """UpdateSchema should NOT have 'asset' field (immutable)"""
        fields = AssetUpdateSchema.model_fields.keys()
        assert "asset" not in fields, "asset field should be immutable (not in UpdateSchema)"
    
    def test_update_schema_excludes_decimals(self):
        """UpdateSchema should NOT have 'decimals' field (immutable)"""
        fields = AssetUpdateSchema.model_fields.keys()
        assert "decimals" not in fields, "decimals field should be immutable (not in UpdateSchema)"
    
    def test_update_schema_has_mutable_fields(self):
        """UpdateSchema should have mutable fields"""
        fields = AssetUpdateSchema.model_fields.keys()
        assert "name" in fields, "name should be mutable"
        assert "status" in fields, "status should be mutable"
        assert "asset_flags" in fields, "asset_flags should be mutable"
    
    def test_update_schema_valid(self):
        """Valid update should pass"""
        schema = AssetUpdateSchema(
            name="Bitcoin Updated",
            status=0,  # Disable
            asset_flags=7,
        )
        assert schema.name == "Bitcoin Updated"
        assert schema.status == 0


class TestSymbolImmutability:
    """Test Symbol field immutability (per id-specification.md)"""
    
    def test_update_schema_excludes_symbol(self):
        """UpdateSchema should NOT have 'symbol' field (immutable)"""
        fields = SymbolUpdateSchema.model_fields.keys()
        assert "symbol" not in fields, "symbol field should be immutable"
    
    def test_update_schema_excludes_base_asset_id(self):
        """UpdateSchema should NOT have 'base_asset_id' field (immutable)"""
        fields = SymbolUpdateSchema.model_fields.keys()
        assert "base_asset_id" not in fields, "base_asset_id should be immutable"
    
    def test_update_schema_excludes_quote_asset_id(self):
        """UpdateSchema should NOT have 'quote_asset_id' field (immutable)"""
        fields = SymbolUpdateSchema.model_fields.keys()
        assert "quote_asset_id" not in fields, "quote_asset_id should be immutable"
    
    def test_update_schema_excludes_decimals(self):
        """UpdateSchema should NOT have decimal fields (immutable)"""
        fields = SymbolUpdateSchema.model_fields.keys()
        assert "price_decimals" not in fields, "price_decimals should be immutable"
        assert "qty_decimals" not in fields, "qty_decimals should be immutable"
    
    def test_update_schema_has_mutable_fields(self):
        """UpdateSchema should have mutable fields"""
        fields = SymbolUpdateSchema.model_fields.keys()
        assert "min_qty" in fields, "min_qty should be mutable"
        assert "status" in fields, "status should be mutable"
        assert "symbol_flags" in fields, "symbol_flags should be mutable"
        assert "base_maker_fee" in fields, "base_maker_fee should be mutable"
        assert "base_taker_fee" in fields, "base_taker_fee should be mutable"
    
    def test_update_schema_valid(self):
        """Valid update should pass"""
        schema = SymbolUpdateSchema(
            min_qty=100,
            status=2,  # Close-only
            symbol_flags=15,
            base_maker_fee=500,  # 0.05%
            base_taker_fee=1000,  # 0.10%
        )
        assert schema.status == 2
        assert schema.base_maker_fee == 500

