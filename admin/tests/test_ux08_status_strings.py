"""
Unit tests for UX-08: Status String Handling
Verifies that status fields accept string/int inputs and serialize to strings.
"""
import pytest
from pydantic import ValidationError
from schemas.asset import AssetCreateSchema, AssetStatus
from schemas.symbol import SymbolCreateSchema, SymbolStatus


class TestUX08StatusHandling:
    """Test suite for human-readable status strings (UX-08)"""

    def test_asset_status_string_input(self):
        """Test that Asset accepts string inputs (case-insensitive)"""
        # Upper case
        asset_upper = AssetCreateSchema(asset="BTC", name="Bitcoin", decimals=8, status="ACTIVE")
        assert asset_upper.status == AssetStatus.ACTIVE
        
        # Lower case (auto-conversion)
        asset_lower = AssetCreateSchema(asset="ETH", name="Ethereum", decimals=18, status="disabled")
        assert asset_lower.status == AssetStatus.DISABLED

    def test_asset_status_integer_input(self):
        """Test that Asset still accepts legacy integer inputs"""
        asset_int = AssetCreateSchema(asset="USDT", name="Tether", decimals=6, status=1)
        assert asset_int.status == AssetStatus.ACTIVE

    def test_asset_status_serialization(self):
        """Test that Asset status serializes to string"""
        asset = AssetCreateSchema(asset="BTC", name="Bitcoin", decimals=8, status=AssetStatus.ACTIVE)
        dump = asset.model_dump()
        # Crucial check: serializes to "ACTIVE", not 1
        assert dump["status"] == "ACTIVE"

    def test_symbol_status_string_input(self):
        """Test that Symbol accepts string inputs with dash/underscore handling"""
        # Underscore
        symbol_under = SymbolCreateSchema(
            symbol="BTC_USDT", base_asset_id=1, quote_asset_id=2, 
            price_decimals=2, qty_decimals=8, status="CLOSE_ONLY"
        )
        assert symbol_under.status == SymbolStatus.CLOSE_ONLY
        
        # Dash conversion (UX enhancement)
        symbol_dash = SymbolCreateSchema(
            symbol="ETH_BTC", base_asset_id=3, quote_asset_id=1, 
            price_decimals=6, qty_decimals=8, status="close-only"
        )
        assert symbol_dash.status == SymbolStatus.CLOSE_ONLY

    def test_symbol_status_serialization(self):
        """Test that Symbol status serializes to string"""
        symbol = SymbolCreateSchema(
            symbol="BTC_USDT", base_asset_id=1, quote_asset_id=2, 
            price_decimals=2, qty_decimals=8, status=SymbolStatus.ONLINE
        )
        dump = symbol.model_dump()
        assert dump["status"] == "ONLINE"

    def test_invalid_status_inputs(self):
        """Test that invalid values are rejected with clear messages"""
        # Invalid string
        with pytest.raises(ValidationError) as exc:
            AssetCreateSchema(asset="BTC", name="B", decimals=8, status="MAYBE")
        assert "Status must be ACTIVE or DISABLED" in str(exc.value)

        # Invalid integer
        with pytest.raises(ValidationError) as exc:
            SymbolCreateSchema(
                symbol="BTC_USDT", base_asset_id=1, quote_asset_id=2, 
                price_decimals=2, qty_decimals=8, status=99
            )
        assert "Status must be ONLINE, OFFLINE, or CLOSE_ONLY" in str(exc.value)
