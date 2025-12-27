"""
Unit tests for UX-08: Status String Handling
Verifies that status fields accept string/int inputs.
Note: Status is stored as integer in DB, not serialized to string.
"""
import pytest
from pydantic import ValidationError
from schemas.asset import AssetCreateSchema, AssetStatus
from schemas.symbol import SymbolCreateSchema, SymbolStatus


class TestUX08StatusHandling:
    """Test suite for status input handling (UX-08)"""

    def test_asset_status_string_input(self):
        """Test that Asset accepts string inputs (case-insensitive)"""
        # Upper case
        asset_upper = AssetCreateSchema(asset="BTC", name="Bitcoin", decimals=8, status="ACTIVE")
        assert asset_upper.status == 1  # Internal representation
        
        # Lower case (auto-conversion)
        asset_lower = AssetCreateSchema(asset="ETH", name="Ethereum", decimals=18, status="disabled")
        assert asset_lower.status == 0  # Internal representation

    def test_asset_status_integer_input(self):
        """Test that Asset accepts integer inputs directly"""
        asset = AssetCreateSchema(asset="BTC", name="Bitcoin", decimals=8, status=1)
        assert asset.status == 1
        
        asset_disabled = AssetCreateSchema(asset="ETH", name="Ethereum", decimals=18, status=0)
        assert asset_disabled.status == 0

    def test_asset_status_serialization(self):
        """Test that Asset status serializes to integer (DB compatible)"""
        asset = AssetCreateSchema(asset="BTC", name="Bitcoin", decimals=8, status="ACTIVE")
        dump = asset.model_dump(mode='json')
        # Status stored as integer for DB compatibility
        assert dump["status"] == 1

    def test_symbol_status_string_input(self):
        """Test that Symbol accepts string inputs with dash/underscore handling"""
        # Underscore
        symbol_under = SymbolCreateSchema(
            symbol="BTC_USDT", base_asset_id=1, quote_asset_id=2, 
            price_decimals=2, qty_decimals=8, status="CLOSE_ONLY"
        )
        assert symbol_under.status == 2  # Internal representation
        
        # Dash conversion (UX enhancement)
        symbol_dash = SymbolCreateSchema(
            symbol="ETH_BTC", base_asset_id=3, quote_asset_id=1, 
            price_decimals=6, qty_decimals=8, status="close-only"
        )
        assert symbol_dash.status == 2  # Internal representation

    def test_symbol_status_serialization(self):
        """Test that Symbol status serializes to integer (DB compatible)"""
        symbol = SymbolCreateSchema(
            symbol="BTC_USDT", base_asset_id=1, quote_asset_id=2, 
            price_decimals=2, qty_decimals=8, status="ONLINE"
        )
        dump = symbol.model_dump(mode='json')
        # Status stored as integer for DB compatibility
        assert dump["status"] == 1

    def test_invalid_status_inputs(self):
        """Test that invalid values are rejected with clear messages"""
        # Invalid string
        with pytest.raises(ValidationError) as exc:
            AssetCreateSchema(asset="BTC", name="B", decimals=8, status="MAYBE")
        assert "ACTIVE" in str(exc.value) or "status" in str(exc.value).lower()

        # Invalid integer (out of range)
        with pytest.raises(ValidationError) as exc:
            SymbolCreateSchema(
                symbol="BTC_USDT", base_asset_id=1, quote_asset_id=2, 
                price_decimals=2, qty_decimals=8, status=99
            )
        assert "status" in str(exc.value).lower()
