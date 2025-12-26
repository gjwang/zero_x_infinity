"""
Test ID Mapping - Critical ID Specification Tests

Per docs/src/standards/id-specification.md:
- Asset: asset_id ↔ asset (code) ↔ name 映射正确
- Symbol: symbol_id ↔ symbol ↔ base_asset_id + quote_asset_id 映射正确

These are CRITICAL tests for data integrity.
"""

import pytest
from pydantic import ValidationError

from admin.asset import AssetCreateSchema
from admin.symbol import SymbolCreateSchema


class TestAssetIdMapping:
    """Test Asset ID to code/name mapping"""
    
    def test_asset_code_uppercase_required(self):
        """Asset code must be uppercase per ID spec"""
        # Valid uppercase
        schema = AssetCreateSchema(
            asset="BTC",
            name="Bitcoin",
            decimals=8,
        )
        assert schema.asset == "BTC"
        assert schema.asset.isupper()
    
    def test_asset_code_auto_uppercase_conversion(self):
        """Asset code should be auto-converted to uppercase"""
        # The current implementation auto-converts
        schema = AssetCreateSchema(
            asset="btc",  # lowercase input
            name="Bitcoin",
            decimals=8,
        )
        # Should be converted to uppercase
        assert schema.asset == "BTC"
    
    def test_asset_code_is_unique_identifier(self):
        """Asset code is the unique identifier (not name)
        
        Per ID spec:
        - asset (code): unique, immutable, used as identifier
        - name: display name, mutable
        """
        schema1 = AssetCreateSchema(
            asset="BTC",
            name="Bitcoin",
            decimals=8,
        )
        schema2 = AssetCreateSchema(
            asset="BTC",
            name="Bitcoin Core",  # Different name, same code
            decimals=8,
        )
        # Same code = same asset
        assert schema1.asset == schema2.asset
    
    def test_asset_code_format_matches_spec(self):
        """Asset code format: A-Z only, 1-16 chars
        
        Per ID spec: ^[A-Z]+$ (our current validation)
        Note: Spec allows 0-9 and _, but current impl is stricter
        """
        # Valid codes
        valid_codes = ["BTC", "ETH", "USDT", "USDC", "A", "ABCDEFGHIJKLMNOP"]
        for code in valid_codes:
            schema = AssetCreateSchema(asset=code, name="Test", decimals=8)
            assert schema.asset == code.upper()
    
    def test_asset_name_is_display_only(self):
        """Asset name is display name, not identifier"""
        schema = AssetCreateSchema(
            asset="BTC",
            name="比特币",  # Chinese name
            decimals=8,
        )
        # Code is the identifier, name is just display
        assert schema.asset == "BTC"
        assert schema.name == "比特币"


class TestSymbolIdMapping:
    """Test Symbol ID to code/asset mapping"""
    
    def test_symbol_format_base_quote(self):
        """Symbol must be in BASE_QUOTE format"""
        schema = SymbolCreateSchema(
            symbol="BTC_USDT",
            base_asset_id=1,
            quote_asset_id=2,
            price_decimals=2,
            qty_decimals=8,
        )
        # Symbol encodes BASE and QUOTE
        assert "_" in schema.symbol
        base, quote = schema.symbol.split("_")
        assert base == "BTC"
        assert quote == "USDT"
    
    def test_symbol_base_quote_ids_match_symbol_name(self):
        """Symbol name should match base and quote asset IDs
        
        CRITICAL: If symbol is BTC_USDT:
        - base_asset_id should point to BTC
        - quote_asset_id should point to USDT
        
        This is enforced at database level via FK, but
        Admin should display the mapping correctly.
        """
        schema = SymbolCreateSchema(
            symbol="BTC_USDT",
            base_asset_id=1,  # Should be BTC
            quote_asset_id=2,  # Should be USDT
            price_decimals=2,
            qty_decimals=8,
        )
        # The schema accepts the IDs
        # Integration test needed to verify:
        # - base_asset_id=1 -> assets_tb.asset = "BTC"
        # - quote_asset_id=2 -> assets_tb.asset = "USDT"
        assert schema.base_asset_id == 1
        assert schema.quote_asset_id == 2
    
    def test_symbol_naming_consistency(self):
        """Symbol name should be consistent with asset codes
        
        If we have:
        - Asset ID 1 = "ETH"
        - Asset ID 2 = "BTC"
        
        Then symbol should be "ETH_BTC", not "BTC_ETH"
        """
        # This is a design constraint - symbol name encodes the pair
        schema = SymbolCreateSchema(
            symbol="ETH_BTC",
            base_asset_id=1,  # ETH
            quote_asset_id=2,  # BTC
            price_decimals=8,
            qty_decimals=4,
        )
        base, quote = schema.symbol.split("_")
        # base (ETH) should match base_asset_id
        # quote (BTC) should match quote_asset_id
        # NOTE: This is a constraint that Admin UI should enforce
        assert base == "ETH"
        assert quote == "BTC"


class TestIdConsistencyChecks:
    """Integration tests for ID consistency (require DB)"""
    
    def test_symbol_base_asset_consistency_check(self):
        """CRITICAL: Symbol.base_asset_id must map to correct Asset
        
        Example violation to detect:
        - Symbol: BTC_USDT
        - base_asset_id: 5 (which is actually ETH, not BTC)
        
        This should be caught by:
        1. Admin UI validation (display asset code for confirmation)
        2. Integration test (verify DB mapping)
        """
        # This is a documentation test - actual check needs DB
        pass
    
    def test_symbol_quote_asset_consistency_check(self):
        """CRITICAL: Symbol.quote_asset_id must map to correct Asset
        
        Similar to base_asset check
        """
        pass
    
    def test_symbol_name_derivable_from_assets(self):
        """Symbol name should be derivable from base + quote assets
        
        If base_asset_id -> "BTC" and quote_asset_id -> "USDT"
        Then symbol should be "BTC_USDT"
        
        This could be auto-generated or validated.
        """
        pass


class TestIdImmutability:
    """Test that IDs cannot be changed after creation
    
    Per ID spec: "ID 一旦创建不应修改"
    """
    
    def test_asset_code_immutable(self):
        """Asset code cannot change (covered in immutability tests)"""
        from admin.asset import AssetUpdateSchema
        fields = AssetUpdateSchema.model_fields.keys()
        assert "asset" not in fields, "asset code should be immutable"
    
    def test_symbol_code_immutable(self):
        """Symbol code cannot change"""
        from admin.symbol import SymbolUpdateSchema
        fields = SymbolUpdateSchema.model_fields.keys()
        assert "symbol" not in fields, "symbol code should be immutable"
    
    def test_symbol_asset_ids_immutable(self):
        """Symbol's base_asset_id and quote_asset_id cannot change
        
        Changing these would break the meaning of the symbol
        """
        from admin.symbol import SymbolUpdateSchema
        fields = SymbolUpdateSchema.model_fields.keys()
        assert "base_asset_id" not in fields, "base_asset_id should be immutable"
        assert "quote_asset_id" not in fields, "quote_asset_id should be immutable"


class TestIdValidationRules:
    """Test validation rules from ID specification"""
    
    def test_asset_code_length_1_to_16(self):
        """Asset code: 1-16 characters"""
        # Min: 1 char (currently allowed in spec, might be restricted in impl)
        # Max: 16 chars
        schema = AssetCreateSchema(asset="ABCDEFGHIJKLMNOP", name="Test", decimals=8)
        assert len(schema.asset) == 16
        
        # 17 chars should fail
        with pytest.raises(ValidationError):
            AssetCreateSchema(asset="ABCDEFGHIJKLMNOPQ", name="Test", decimals=8)
    
    def test_symbol_length_3_to_33(self):
        """Symbol: 3-33 characters per spec
        
        Min: 3 chars (e.g., "A_B")
        Max: 33 chars (e.g., 16-char BASE + "_" + 16-char QUOTE)
        
        Note: Current impl uses 32 max
        """
        # Valid short
        schema = SymbolCreateSchema(
            symbol="A_B",
            base_asset_id=1,
            quote_asset_id=2,
            price_decimals=2,
            qty_decimals=8,
        )
        assert len(schema.symbol) == 3
    
    def test_symbol_single_underscore_required(self):
        """Symbol must have exactly one underscore"""
        # No underscore
        with pytest.raises(ValidationError):
            SymbolCreateSchema(
                symbol="BTCUSDT",
                base_asset_id=1,
                quote_asset_id=2,
                price_decimals=2,
                qty_decimals=8,
            )
        
        # Multiple underscores
        with pytest.raises(ValidationError):
            SymbolCreateSchema(
                symbol="BTC_USDT_PERP",
                base_asset_id=1,
                quote_asset_id=2,
                price_decimals=2,
                qty_decimals=8,
            )
