"""
Test Input Validation
AC-09: Reject invalid input (decimals<0, fee>100%)
"""

import pytest
from pydantic import ValidationError

from admin.asset import AssetCreateSchema
from admin.symbol import SymbolCreateSchema
from admin.vip_level import VIPLevelCreateSchema


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
        assert "uppercase letters" in str(exc_info.value).lower()
    
    def test_invalid_decimals_negative(self):
        """Negative decimals should be rejected"""
        with pytest.raises(ValidationError) as exc_info:
            AssetCreateSchema(
                asset="BTC",
                name="Bitcoin",
                decimals=-1,
            )
        assert "between 0 and 18" in str(exc_info.value)
    
    def test_invalid_decimals_too_large(self):
        """Decimals > 18 should be rejected"""
        with pytest.raises(ValidationError) as exc_info:
            AssetCreateSchema(
                asset="BTC",
                name="Bitcoin",
                decimals=19,
            )
        assert "between 0 and 18" in str(exc_info.value)
    
    def test_invalid_status(self):
        """Invalid status should be rejected"""
        with pytest.raises(ValidationError) as exc_info:
            AssetCreateSchema(
                asset="BTC",
                name="Bitcoin",
                decimals=8,
                status=3,  # Only 0 or 1 allowed
            )
        assert "must be 0" in str(exc_info.value)


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
        assert "BASE_QUOTE" in str(exc_info.value)
    
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
        assert "between 0 and 10000" in str(exc_info.value)
    
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
        assert "between 0 and 10000" in str(exc_info.value)


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
        assert "0 or greater" in str(exc_info.value)
    
    def test_invalid_discount_too_high(self):
        """Discount > 100 should be rejected"""
        with pytest.raises(ValidationError) as exc_info:
            VIPLevelCreateSchema(
                level=1,
                discount_percent=101,
            )
        assert "between 0 and 100" in str(exc_info.value)
    
    def test_invalid_discount_negative(self):
        """Negative discount should be rejected"""
        with pytest.raises(ValidationError) as exc_info:
            VIPLevelCreateSchema(
                level=1,
                discount_percent=-1,
            )
        assert "between 0 and 100" in str(exc_info.value)
