"""
Test Edge Cases - Agent A Tests

Per 0x0F-admin-test-plan.md:
- TC-EDGE-14: Unicode in symbol
- TC-EDGE-15: Overflow name
- TC-EDGE-16: VIP discount >100%
- TC-STATE-06: Symbol CloseOnly state
"""

import pytest
from pydantic import ValidationError

from schemas.asset import AssetCreateSchema
from schemas.symbol import SymbolCreateSchema, SymbolUpdateSchema
from schemas.vip_level import VIPLevelCreateSchema


class TestEdgeCasesAgentA:
    """Agent A (激进派): Edge case and boundary tests"""
    
    # === TC-EDGE-14: Unicode in symbol ===
    def test_unicode_in_symbol_rejected(self):
        """TC-EDGE-14: Unicode characters in symbol should be rejected"""
        with pytest.raises(ValidationError) as exc_info:
            SymbolCreateSchema(
                symbol="BTC_💎_USDT",  # Emoji not allowed
                base_asset_id=1,
                quote_asset_id=2,
                price_decimals=2,
                qty_decimals=8,
            )
        # Should fail validation
        assert exc_info.value is not None
    
    def test_chinese_in_symbol_rejected(self):
        """Chinese characters in symbol should be rejected"""
        with pytest.raises(ValidationError) as exc_info:
            SymbolCreateSchema(
                symbol="BTC_人民币",  # Chinese not allowed
                base_asset_id=1,
                quote_asset_id=2,
                price_decimals=2,
                qty_decimals=8,
            )
        assert exc_info.value is not None
    
    # === TC-EDGE-15: Overflow name ===
    def test_asset_name_overflow_rejected(self):
        """TC-EDGE-15: Extremely long asset name should be rejected or truncated"""
        long_name = "A" * 1000
        # This should either raise validation error or truncate
        try:
            schema = AssetCreateSchema(
                asset="BTC",
                name=long_name,
                decimals=8,
            )
            # If it doesn't raise, name should be truncated or limited
            assert len(schema.name) <= 256, "Name should be limited to 256 chars"
        except ValidationError:
            # This is also acceptable - validation should reject it
            pass
    
    def test_symbol_name_overflow_rejected(self):
        """Symbol with extremely long name should be rejected"""
        long_symbol = "A" * 256
        with pytest.raises(ValidationError):
            SymbolCreateSchema(
                symbol=long_symbol,
                base_asset_id=1,
                quote_asset_id=2,
                price_decimals=2,
                qty_decimals=8,
            )
    
    # === TC-EDGE-16: VIP discount >100% ===
    def test_vip_discount_over_100_rejected(self):
        """TC-EDGE-16: VIP discount >100% should be rejected"""
        with pytest.raises(ValidationError) as exc_info:
            VIPLevelCreateSchema(
                level=1,
                discount_percent=101,  # >100% not allowed
            )
        # Pydantic Field(le=100) error message
        assert "less than or equal" in str(exc_info.value).lower() or "100" in str(exc_info.value)
    
    def test_vip_discount_exactly_100_accepted(self):
        """VIP discount exactly 100% (no discount) should be accepted"""
        schema = VIPLevelCreateSchema(
            level=0,
            discount_percent=100,
            description="Normal",
        )
        assert schema.discount_percent == 100
    
    def test_vip_discount_zero_accepted(self):
        """VIP discount 0% (100% discount) should be accepted"""
        schema = VIPLevelCreateSchema(
            level=5,
            discount_percent=0,
            description="VIP Supreme - Free trades",
        )
        assert schema.discount_percent == 0


class TestSymbolCloseOnlyState:
    """TC-STATE-06: Symbol CloseOnly state tests"""
    
    def test_symbol_status_close_only_valid(self):
        """CloseOnly status (2) should be valid in update"""
        schema = SymbolUpdateSchema(
            min_qty=100,
            status=2,  # CloseOnly
            symbol_flags=0,
            base_maker_fee=100,
            base_taker_fee=200,
        )
        assert schema.status == 2
    
    def test_symbol_status_trading_valid(self):
        """Trading status (1) should be valid"""
        schema = SymbolUpdateSchema(
            min_qty=100,
            status=1,  # Trading
            symbol_flags=0,
            base_maker_fee=100,
            base_taker_fee=200,
        )
        assert schema.status == 1
    
    def test_symbol_status_halt_valid(self):
        """Halt status (0) should be valid"""
        schema = SymbolUpdateSchema(
            min_qty=100,
            status=0,  # Halt
            symbol_flags=0,
            base_maker_fee=100,
            base_taker_fee=200,
        )
        assert schema.status == 0
    
    def test_symbol_status_invalid_rejected(self):
        """Invalid status (e.g., 5) should be rejected"""
        with pytest.raises(ValidationError) as exc_info:
            SymbolUpdateSchema(
                min_qty=100,
                status=5,  # Invalid
                symbol_flags=0,
                base_maker_fee=100,
                base_taker_fee=200,
            )
        assert "status" in str(exc_info.value).lower()


class TestInjectionAgentA:
    """Injection attack tests"""
    
    def test_sql_injection_in_asset_name(self):
        """SQL injection in asset name should be escaped or rejected"""
        # The schema should accept it (will be escaped by ORM)
        # But if validation rejects special chars, that's also fine
        try:
            schema = AssetCreateSchema(
                asset="BTC",
                name="'; DROP TABLE assets_tb; --",
                decimals=8,
            )
            # If accepted, the ORM should escape this
            assert "DROP" in schema.name  # Raw value stored, will be escaped by SQLAlchemy
        except ValidationError:
            # Also acceptable - validation rejects dangerous input
            pass
    
    def test_sql_injection_in_asset_code_rejected(self):
        """SQL injection in asset code should be rejected (non-letter chars)"""
        with pytest.raises(ValidationError):
            AssetCreateSchema(
                asset="'; DROP TABLE --",
                name="Hacked",
                decimals=8,
            )
    
    def test_null_byte_in_symbol_rejected(self):
        """Null byte in symbol should be rejected"""
        with pytest.raises(ValidationError):
            SymbolCreateSchema(
                symbol="BTC\x00USDT",  # Null byte
                base_asset_id=1,
                quote_asset_id=2,
                price_decimals=2,
                qty_decimals=8,
            )


class TestPrecisionAgentA:
    """Precision and decimal handling tests"""
    
    def test_fee_stored_as_integer_bps(self):
        """Fee rate should be stored as integer bps, not float"""
        schema = SymbolCreateSchema(
            symbol="BTC_USDT",
            base_asset_id=1,
            quote_asset_id=2,
            price_decimals=2,
            qty_decimals=8,
            base_maker_fee=10,  # 0.10% = 10 bps
            base_taker_fee=20,  # 0.20% = 20 bps
        )
        assert isinstance(schema.base_maker_fee, int)
        assert isinstance(schema.base_taker_fee, int)
        assert schema.base_maker_fee == 10
        assert schema.base_taker_fee == 20
    
    def test_fractional_bps_rejected(self):
        """Fractional bps (non-integer) should be rejected or rounded"""
        # Per GAP-06: Integer bps only
        with pytest.raises((ValidationError, TypeError)):
            SymbolCreateSchema(
                symbol="BTC_USDT",
                base_asset_id=1,
                quote_asset_id=2,
                price_decimals=2,
                qty_decimals=8,
                base_maker_fee=10.5,  # Not an integer
            )
    
    def test_boundary_fee_10000_bps_accepted(self):
        """Maximum fee 10000 bps (100%) should be accepted"""
        schema = SymbolCreateSchema(
            symbol="BTC_USDT",
            base_asset_id=1,
            quote_asset_id=2,
            price_decimals=2,
            qty_decimals=8,
            base_maker_fee=10000,  # 100%
        )
        assert schema.base_maker_fee == 10000
    
    def test_boundary_fee_0_bps_accepted(self):
        """Zero fee (0 bps) should be accepted"""
        schema = SymbolCreateSchema(
            symbol="BTC_USDT",
            base_asset_id=1,
            quote_asset_id=2,
            price_decimals=2,
            qty_decimals=8,
            base_maker_fee=0,  # 0%
        )
        assert schema.base_maker_fee == 0
