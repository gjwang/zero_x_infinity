"""
Test Immutability - CRITICAL Tests

Per arch-to-qa-0x0F-immutability-critical.md:
- TC-IMMUTABLE-01: Asset code cannot be changed
- TC-IMMUTABLE-02: Asset decimals cannot be changed
- TC-IMMUTABLE-03: Symbol name cannot be changed
- TC-IMMUTABLE-04: Symbol base_asset_id cannot be changed
- TC-IMMUTABLE-05: Symbol quote_asset_id cannot be changed
- TC-IMMUTABLE-06: Symbol decimals cannot be changed

These fields are IMMUTABLE after creation per id-specification.md
"""

import pytest
from pydantic import ValidationError

from admin.asset import AssetCreateSchema, AssetUpdateSchema
from admin.symbol import SymbolCreateSchema, SymbolUpdateSchema


class TestAssetImmutability:
    """TC-IMMUTABLE-01, TC-IMMUTABLE-02: Asset immutable fields"""
    
    def test_create_schema_has_asset(self):
        """CreateSchema should have 'asset' field"""
        fields = AssetCreateSchema.model_fields.keys()
        assert "asset" in fields
    
    def test_create_schema_has_decimals(self):
        """CreateSchema should have 'decimals' field"""
        fields = AssetCreateSchema.model_fields.keys()
        assert "decimals" in fields
    
    def test_update_schema_no_asset(self):
        """TC-IMMUTABLE-01: UpdateSchema should NOT have 'asset' field"""
        fields = AssetUpdateSchema.model_fields.keys()
        assert "asset" not in fields, \
            "CRITICAL: 'asset' field must NOT be in UpdateSchema (immutable)"
    
    def test_update_schema_no_decimals(self):
        """TC-IMMUTABLE-02: UpdateSchema should NOT have 'decimals' field"""
        fields = AssetUpdateSchema.model_fields.keys()
        assert "decimals" not in fields, \
            "CRITICAL: 'decimals' field must NOT be in UpdateSchema (immutable)"
    
    def test_update_schema_has_mutable_fields(self):
        """UpdateSchema should have mutable fields only"""
        fields = set(AssetUpdateSchema.model_fields.keys())
        expected = {"name", "status", "asset_flags"}
        assert fields == expected, f"Expected {expected}, got {fields}"
    
    def test_cannot_update_asset_via_schema(self):
        """Attempting to pass 'asset' to UpdateSchema should have no effect"""
        # UpdateSchema doesn't accept 'asset' field
        schema = AssetUpdateSchema(
            name="Updated Name",
            status=1,
            asset_flags=7,
        )
        # Verify 'asset' is not present
        assert not hasattr(schema, 'asset') or schema.model_fields.get('asset') is None
    
    def test_cannot_update_decimals_via_schema(self):
        """Attempting to pass 'decimals' to UpdateSchema should have no effect"""
        schema = AssetUpdateSchema(
            name="Updated Name",
            status=1,
            asset_flags=7,
        )
        # Verify 'decimals' is not present
        assert not hasattr(schema, 'decimals') or schema.model_fields.get('decimals') is None


class TestSymbolImmutability:
    """TC-IMMUTABLE-03 to TC-IMMUTABLE-06: Symbol immutable fields"""
    
    def test_create_schema_has_symbol(self):
        """CreateSchema should have 'symbol' field"""
        fields = SymbolCreateSchema.model_fields.keys()
        assert "symbol" in fields
    
    def test_create_schema_has_base_asset_id(self):
        """CreateSchema should have 'base_asset_id' field"""
        fields = SymbolCreateSchema.model_fields.keys()
        assert "base_asset_id" in fields
    
    def test_create_schema_has_quote_asset_id(self):
        """CreateSchema should have 'quote_asset_id' field"""
        fields = SymbolCreateSchema.model_fields.keys()
        assert "quote_asset_id" in fields
    
    def test_create_schema_has_decimals(self):
        """CreateSchema should have decimal fields"""
        fields = SymbolCreateSchema.model_fields.keys()
        assert "price_decimals" in fields
        assert "qty_decimals" in fields
    
    def test_update_schema_no_symbol(self):
        """TC-IMMUTABLE-03: UpdateSchema should NOT have 'symbol' field"""
        fields = SymbolUpdateSchema.model_fields.keys()
        assert "symbol" not in fields, \
            "CRITICAL: 'symbol' field must NOT be in UpdateSchema (immutable)"
    
    def test_update_schema_no_base_asset_id(self):
        """TC-IMMUTABLE-04: UpdateSchema should NOT have 'base_asset_id' field"""
        fields = SymbolUpdateSchema.model_fields.keys()
        assert "base_asset_id" not in fields, \
            "CRITICAL: 'base_asset_id' must NOT be in UpdateSchema (immutable)"
    
    def test_update_schema_no_quote_asset_id(self):
        """TC-IMMUTABLE-05: UpdateSchema should NOT have 'quote_asset_id' field"""
        fields = SymbolUpdateSchema.model_fields.keys()
        assert "quote_asset_id" not in fields, \
            "CRITICAL: 'quote_asset_id' must NOT be in UpdateSchema (immutable)"
    
    def test_update_schema_no_price_decimals(self):
        """TC-IMMUTABLE-06: UpdateSchema should NOT have 'price_decimals' field"""
        fields = SymbolUpdateSchema.model_fields.keys()
        assert "price_decimals" not in fields, \
            "CRITICAL: 'price_decimals' must NOT be in UpdateSchema (immutable)"
    
    def test_update_schema_no_qty_decimals(self):
        """TC-IMMUTABLE-06: UpdateSchema should NOT have 'qty_decimals' field"""
        fields = SymbolUpdateSchema.model_fields.keys()
        assert "qty_decimals" not in fields, \
            "CRITICAL: 'qty_decimals' must NOT be in UpdateSchema (immutable)"
    
    def test_update_schema_has_mutable_fields(self):
        """UpdateSchema should have mutable fields only"""
        fields = set(SymbolUpdateSchema.model_fields.keys())
        expected = {"min_qty", "status", "symbol_flags", "base_maker_fee", "base_taker_fee"}
        assert fields == expected, f"Expected {expected}, got {fields}"
    
    def test_cannot_update_symbol_via_schema(self):
        """Attempting to pass 'symbol' to UpdateSchema should have no effect"""
        schema = SymbolUpdateSchema(
            min_qty=100,
            status=1,
            symbol_flags=0,
            base_maker_fee=50,
            base_taker_fee=100,
        )
        assert "symbol" not in schema.model_fields
    
    def test_cannot_update_asset_ids_via_schema(self):
        """Attempting to pass asset IDs to UpdateSchema should have no effect"""
        schema = SymbolUpdateSchema(
            min_qty=100,
            status=1,
            symbol_flags=0,
            base_maker_fee=50,
            base_taker_fee=100,
        )
        assert "base_asset_id" not in schema.model_fields
        assert "quote_asset_id" not in schema.model_fields


class TestImmutabilityIntegration:
    """Integration tests for immutability enforcement"""
    
    def test_asset_immutable_fields_count(self):
        """Verify exactly 2 immutable fields in Asset"""
        create_fields = set(AssetCreateSchema.model_fields.keys())
        update_fields = set(AssetUpdateSchema.model_fields.keys())
        immutable = create_fields - update_fields
        
        expected_immutable = {"asset", "decimals"}
        assert immutable == expected_immutable, \
            f"Asset immutable fields should be {expected_immutable}, got {immutable}"
    
    def test_symbol_immutable_fields_count(self):
        """Verify exactly 5 immutable fields in Symbol"""
        create_fields = set(SymbolCreateSchema.model_fields.keys())
        update_fields = set(SymbolUpdateSchema.model_fields.keys())
        immutable = create_fields - update_fields
        
        expected_immutable = {"symbol", "base_asset_id", "quote_asset_id", 
                              "price_decimals", "qty_decimals"}
        assert immutable == expected_immutable, \
            f"Symbol immutable fields should be {expected_immutable}, got {immutable}"
    
    def test_all_immutable_fields_documented(self):
        """All immutable fields should be documented in docstrings"""
        # Check AssetUpdateSchema docstring
        assert "IMMUTABLE" in AssetUpdateSchema.__doc__
        assert "asset" in AssetUpdateSchema.__doc__
        assert "decimals" in AssetUpdateSchema.__doc__
        
        # Check SymbolUpdateSchema docstring
        assert "IMMUTABLE" in SymbolUpdateSchema.__doc__ or \
               "immutable" in SymbolUpdateSchema.__doc__.lower()
