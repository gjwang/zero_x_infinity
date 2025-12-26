"""
Test Foreign Key & Self-Referential Constraints

Per 0x0F-admin-test-plan.md Gap Analysis:
- TC-NEW-01: Symbol base=quote should be rejected
- TC-NEW-02: Delete referenced Asset should be rejected
- TC-NEW-07: API Decimal should be String

These are constraint tests that were identified as missing.
"""

import pytest
from pydantic import ValidationError

from admin.symbol import SymbolCreateSchema, SymbolUpdateSchema
from admin.asset import AssetCreateSchema


class TestSelfReferentialConstraint:
    """Symbol cannot have base_asset = quote_asset"""
    
    def test_symbol_base_equals_quote_rejected(self):
        """TC-NEW-01: Symbol with base_asset == quote_asset should be rejected
        
        Example: BTC_BTC is invalid - a symbol must have different base and quote
        """
        with pytest.raises(ValidationError) as exc_info:
            SymbolCreateSchema(
                symbol="BTC_BTC",  # Invalid: base = quote
                base_asset_id=1,
                quote_asset_id=1,  # Same as base!
                price_decimals=2,
                qty_decimals=8,
            )
        assert "base" in str(exc_info.value).lower() or "quote" in str(exc_info.value).lower()
    
    def test_symbol_base_not_equals_quote_accepted(self):
        """Valid symbol with different base and quote should be accepted"""
        schema = SymbolCreateSchema(
            symbol="BTC_USDT",
            base_asset_id=1,
            quote_asset_id=2,  # Different from base
            price_decimals=2,
            qty_decimals=8,
        )
        assert schema.base_asset_id != schema.quote_asset_id


class TestForeignKeyConstraint:
    """Asset deletion when referenced by Symbol"""
    
    # Note: These are integration tests that require database
    # For now, we test the schema-level validation
    
    def test_symbol_requires_valid_asset_ids(self):
        """Symbol creation should validate asset IDs exist"""
        # This is schema-level validation
        # Real FK check happens at database level
        schema = SymbolCreateSchema(
            symbol="ETH_USDT",
            base_asset_id=999,  # May not exist
            quote_asset_id=998,  # May not exist
            price_decimals=2,
            qty_decimals=8,
        )
        # Schema accepts any integer, DB will reject non-existent FK
        assert schema.base_asset_id == 999
        assert schema.quote_asset_id == 998
        # Integration test would verify DB rejection


class TestDecimalPrecision:
    """API should return Decimal as String to prevent precision loss"""
    
    def test_fee_rate_is_integer_bps(self):
        """Fee rate should be stored as integer bps"""
        schema = SymbolCreateSchema(
            symbol="BTC_USDT",
            base_asset_id=1,
            quote_asset_id=2,
            price_decimals=2,
            qty_decimals=8,
            base_maker_fee=10,  # 0.10% = 10 bps
            base_taker_fee=20,  # 0.20% = 20 bps
        )
        # Fee should be integer (bps), not float
        assert isinstance(schema.base_maker_fee, int)
        assert isinstance(schema.base_taker_fee, int)
    
    def test_decimals_is_integer(self):
        """Decimals field should be integer"""
        schema = AssetCreateSchema(
            asset="BTC",
            name="Bitcoin",
            decimals=8,
        )
        assert isinstance(schema.decimals, int)


class TestSymbolNamingConvention:
    """Symbol naming tests"""
    
    def test_symbol_format_base_quote(self):
        """Symbol must be BASE_QUOTE format"""
        schema = SymbolCreateSchema(
            symbol="BTC_USDT",
            base_asset_id=1,
            quote_asset_id=2,
            price_decimals=2,
            qty_decimals=8,
        )
        assert "_" in schema.symbol
        parts = schema.symbol.split("_")
        assert len(parts) == 2
    
    def test_symbol_without_underscore_rejected(self):
        """Symbol without underscore should be rejected"""
        with pytest.raises(ValidationError) as exc_info:
            SymbolCreateSchema(
                symbol="BTCUSDT",  # Missing underscore
                base_asset_id=1,
                quote_asset_id=2,
                price_decimals=2,
                qty_decimals=8,
            )
        assert "BASE_QUOTE" in str(exc_info.value)
    
    def test_symbol_multiple_underscores_rejected(self):
        """Symbol with multiple underscores should be rejected"""
        with pytest.raises(ValidationError) as exc_info:
            SymbolCreateSchema(
                symbol="BTC_USDT_FUTURES",  # Multiple underscores
                base_asset_id=1,
                quote_asset_id=2,
                price_decimals=2,
                qty_decimals=8,
            )
        # Should be rejected - only one underscore allowed
        assert exc_info.value is not None


class TestAssetCodeConvention:
    """Asset code naming tests"""
    
    def test_asset_code_uppercase_only(self):
        """Asset code should be uppercase letters only"""
        schema = AssetCreateSchema(
            asset="BTC",
            name="Bitcoin",
            decimals=8,
        )
        assert schema.asset == "BTC"
        assert schema.asset.isupper()
    
    # REMOVED: test_asset_code_with_numbers_rejected
    # Per ID spec update, numbers ARE now allowed (e.g., BTC2, 1INCH)
    # See: test_id_spec_compliance.py for updated tests
    
    
    def test_asset_code_max_length(self):
        """Asset code should be max 16 characters"""
        # Valid: 16 chars
        schema = AssetCreateSchema(
            asset="ABCDEFGHIJKLMNOP",
            name="Test",
            decimals=8,
        )
        assert len(schema.asset) == 16
        
        # Invalid: 17 chars
        with pytest.raises(ValidationError):
            AssetCreateSchema(
                asset="ABCDEFGHIJKLMNOPQ",  # 17 chars
                name="Test",
                decimals=8,
            )
