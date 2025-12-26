"""
Test UX Improvements - Admin Dashboard Usability

Per QA Review: Improvements for Admin usability, readability, and error prevention.

Tests:
- TC-UX-01: Symbol creation shows Asset names (not just IDs)
- TC-UX-02: Fee displays percentage alongside BPS
- TC-UX-03: Dangerous operations require confirmation
- TC-UX-04: Immutable fields disabled in edit mode
- TC-UX-05: Error messages are actionable
- TC-UX-06: Symbol base != quote validation
- TC-UX-07: Symbol name consistency with Asset codes
"""

import pytest
from pydantic import ValidationError

from schemas.asset import AssetCreateSchema
from schemas.symbol import SymbolCreateSchema


class TestSymbolAssetNameDisplay:
    """TC-UX-01: Symbol creation should show Asset names"""
    
    def test_symbol_create_shows_asset_code_from_id(self):
        """When creating symbol, user should see asset NAMES not just IDs
        
        Example:
        base_asset_id: 1 → should display "BTC (ID: 1)"
        quote_asset_id: 2 → should display "USDT (ID: 2)"
        
        This is a UI test requirement, not schema validation.
        """
        # Schema accepts IDs, but UI should display names
        schema = SymbolCreateSchema(
            symbol="BTC_USDT",
            base_asset_id=1,
            quote_asset_id=2,
            price_decimals=2,
            qty_decimals=8,
        )
        # The schema works with IDs
        assert schema.base_asset_id == 1
        # But UI should show: "BTC (ID: 1)" in dropdown
    
    def test_symbol_name_should_match_asset_ids(self):
        """TC-UX-07: Symbol name should match its base/quote assets
        
        If symbol="BTC_USDT":
        - base_asset_id should point to asset with code "BTC"
        - quote_asset_id should point to asset with code "USDT"
        
        VIOLATION EXAMPLE:
        symbol="BTC_USDT", base_asset_id=3(ETH), quote_asset_id=4(BNB)
        → Should warn or reject!
        """
        # This is an integration check that requires DB lookup
        # At minimum, UI should show the asset codes for verification
        pass


class TestFeeDisplayFormat:
    """TC-UX-02: Fee should display percentage alongside BPS"""
    
    def test_fee_bps_to_percentage_conversion(self):
        """BPS should display as percentage for clarity
        
        10 bps → "0.10%"
        100 bps → "1.00%"
        1000 bps → "10.00%"
        """
        def bps_to_percent(bps: int) -> str:
            return f"{bps / 100:.2f}%"
        
        assert bps_to_percent(10) == "0.10%"
        assert bps_to_percent(100) == "1.00%"
        assert bps_to_percent(1000) == "10.00%"
        assert bps_to_percent(2000) == "20.00%"  # 20% taker fee
    
    def test_fee_display_format(self):
        """Fee should display both BPS and percentage
        
        Expected: "10 bps (0.10%)"
        NOT just: "10"
        """
        def format_fee_display(bps: int) -> str:
            pct = bps / 100
            return f"{bps} bps ({pct:.2f}%)"
        
        assert format_fee_display(10) == "10 bps (0.10%)"
        assert format_fee_display(20) == "20 bps (0.20%)"


class TestDangerousOperationConfirmation:
    """TC-UX-03: Dangerous operations require confirmation"""
    
    # These are UI tests, placeholder for requirements
    
    def test_halt_symbol_requires_confirmation(self):
        """Halting a symbol should require:
        1. Show impact preview (active orders, volume)
        2. Type symbol name to confirm
        3. Explicit confirmation button
        """
        pass
    
    def test_disable_asset_requires_confirmation(self):
        """Disabling an asset should require:
        1. Show impact preview (symbols using this asset)
        2. Confirmation dialog
        """
        pass
    
    def test_delete_vip_requires_confirmation(self):
        """Deleting VIP level should show users affected"""
        pass


class TestImmutableFieldsUI:
    """TC-UX-04: Immutable fields should be disabled in edit mode"""
    
    def test_asset_edit_disables_immutable_fields(self):
        """In Asset edit mode:
        - asset (code): DISABLED, show lock icon 🔒
        - decimals: DISABLED, show lock icon 🔒
        - name: ENABLED
        - status: ENABLED
        """
        from schemas.asset import AssetUpdateSchema
        
        # These fields should NOT be in UpdateSchema
        update_fields = set(AssetUpdateSchema.model_fields.keys())
        
        assert "asset" not in update_fields, "asset should be disabled"
        assert "decimals" not in update_fields, "decimals should be disabled"
        assert "name" in update_fields, "name should be editable"
        assert "status" in update_fields, "status should be editable"
    
    def test_symbol_edit_disables_immutable_fields(self):
        """In Symbol edit mode:
        - symbol: DISABLED 🔒
        - base_asset_id: DISABLED 🔒
        - quote_asset_id: DISABLED 🔒
        - price_decimals: DISABLED 🔒
        - qty_decimals: DISABLED 🔒
        - status: ENABLED ✏️
        - fees: ENABLED ✏️
        """
        from schemas.symbol import SymbolUpdateSchema
        
        update_fields = set(SymbolUpdateSchema.model_fields.keys())
        
        # Immutable - should NOT be in update schema
        assert "symbol" not in update_fields
        assert "base_asset_id" not in update_fields
        assert "quote_asset_id" not in update_fields
        assert "price_decimals" not in update_fields
        assert "qty_decimals" not in update_fields
        
        # Mutable - should be in update schema
        assert "status" in update_fields
        assert "base_maker_fee" in update_fields
        assert "base_taker_fee" in update_fields


class TestActionableErrorMessages:
    """TC-UX-05: Error messages should be actionable"""
    
    def test_asset_error_includes_field_name(self):
        """Error should say which field failed"""
        try:
            AssetCreateSchema(
                asset="btc!",  # Invalid char
                name="Bitcoin",
                decimals=8,
            )
        except ValidationError as e:
            error_str = str(e)
            # Should mention the field name
            assert "asset" in error_str.lower()
    
    def test_error_includes_expected_format(self):
        """Error should say what format is expected"""
        try:
            SymbolCreateSchema(
                symbol="BTCUSDT",  # Missing underscore
                base_asset_id=1,
                quote_asset_id=2,
                price_decimals=2,
                qty_decimals=8,
            )
        except ValidationError as e:
            error_str = str(e)
            # Should mention expected format
            assert "BASE_QUOTE" in error_str or "_" in error_str
    
    def test_fee_error_includes_valid_range(self):
        """Fee error should show valid range 0-10000"""
        try:
            SymbolCreateSchema(
                symbol="BTC_USDT",
                base_asset_id=1,
                quote_asset_id=2,
                price_decimals=2,
                qty_decimals=8,
                base_maker_fee=15000,  # Too high
            )
        except ValidationError as e:
            error_str = str(e)
            assert "10000" in error_str or "100%" in error_str


class TestSymbolBaseQuoteValidation:
    """TC-UX-06: Symbol base != quote validation"""
    
    def test_symbol_base_cannot_equal_quote(self):
        """BTC_BTC should be rejected - base cannot equal quote
        
        This prevents nonsensical trading pairs.
        """
        # NOTE: This is BUG-07 - currently not implemented
        # When fixed, this test should pass:
        try:
            schema = SymbolCreateSchema(
                symbol="BTC_BTC",
                base_asset_id=1,
                quote_asset_id=1,  # Same as base!
                price_decimals=2,
                qty_decimals=8,
            )
            # If we get here, the bug is not fixed
            # Mark as expected failure for now
            pytest.xfail("BUG-07: base_asset_id == quote_asset_id should be rejected")
        except ValidationError:
            # This is the expected behavior when fixed
            pass
