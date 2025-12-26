#!/usr/bin/env python3
"""
Admin Dashboard E2E Test Script
CI-Ready: Can be run in CI pipeline

Usage:
    cd admin
    source venv/bin/activate
    python tests/test_e2e_admin.py

Requirements:
    - PostgreSQL running (for app data)
    - Or use --unit-only for unit tests only
"""

import asyncio
import sys
import os

# Add parent directory to path
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

import pytest
from httpx import AsyncClient, ASGITransport


class TestHealthCheck:
    """Test basic health endpoints - no DB required"""
    
    @pytest.fixture
    async def client(self):
        from main import app
        transport = ASGITransport(app=app)
        async with AsyncClient(transport=transport, base_url="http://test") as client:
            yield client
    
    @pytest.mark.asyncio
    async def test_health_endpoint(self, client):
        """AC-01: Health check returns OK"""
        response = await client.get("/health")
        assert response.status_code == 200
        data = response.json()
        assert data["status"] == "ok"
        assert data["service"] == "admin-dashboard"


class TestInputValidationUnit:
    """Unit tests for input validation - no server required"""
    
    def test_asset_create_schema_valid(self):
        """Valid asset creation"""
        from admin.asset import AssetCreateSchema
        schema = AssetCreateSchema(
            asset="BTC",
            name="Bitcoin",
            decimals=8,
            status=1,
        )
        assert schema.asset == "BTC"
        assert schema.decimals == 8
    
    def test_asset_create_schema_invalid_decimals(self):
        """Invalid decimals rejected"""
        from admin.asset import AssetCreateSchema
        from pydantic import ValidationError
        
        with pytest.raises(ValidationError):
            AssetCreateSchema(
                asset="BTC",
                name="Bitcoin",
                decimals=-1,  # Invalid
            )
    
    def test_asset_update_schema_immutable_fields(self):
        """Asset update schema should not have immutable fields"""
        from admin.asset import AssetUpdateSchema
        fields = AssetUpdateSchema.model_fields.keys()
        assert "asset" not in fields, "asset is immutable"
        assert "decimals" not in fields, "decimals is immutable"
    
    def test_symbol_create_schema_valid(self):
        """Valid symbol creation"""
        from admin.symbol import SymbolCreateSchema
        schema = SymbolCreateSchema(
            symbol="BTC_USDT",
            base_asset_id=1,
            quote_asset_id=2,
            price_decimals=2,
            qty_decimals=8,
        )
        assert schema.symbol == "BTC_USDT"
    
    def test_symbol_create_schema_invalid_format(self):
        """Invalid symbol format rejected"""
        from admin.symbol import SymbolCreateSchema
        from pydantic import ValidationError
        
        with pytest.raises(ValidationError):
            SymbolCreateSchema(
                symbol="BTCUSDT",  # Missing underscore
                base_asset_id=1,
                quote_asset_id=2,
                price_decimals=2,
                qty_decimals=8,
            )
    
    def test_symbol_create_schema_invalid_fee(self):
        """Invalid fee rejected"""
        from admin.symbol import SymbolCreateSchema
        from pydantic import ValidationError
        
        with pytest.raises(ValidationError):
            SymbolCreateSchema(
                symbol="BTC_USDT",
                base_asset_id=1,
                quote_asset_id=2,
                price_decimals=2,
                qty_decimals=8,
                base_maker_fee=10001,  # > 100%
            )
    
    def test_symbol_update_schema_immutable_fields(self):
        """Symbol update schema should not have immutable fields"""
        from admin.symbol import SymbolUpdateSchema
        fields = SymbolUpdateSchema.model_fields.keys()
        assert "symbol" not in fields
        assert "base_asset_id" not in fields
        assert "quote_asset_id" not in fields
        assert "price_decimals" not in fields
        assert "qty_decimals" not in fields
    
    def test_vip_level_create_schema_valid(self):
        """Valid VIP level creation"""
        from admin.vip_level import VIPLevelCreateSchema
        schema = VIPLevelCreateSchema(
            level=0,
            discount_percent=100,
            description="Normal",
        )
        assert schema.level == 0
        assert schema.discount_percent == 100
    
    def test_vip_level_create_schema_invalid_discount(self):
        """Invalid discount rejected"""
        from admin.vip_level import VIPLevelCreateSchema
        from pydantic import ValidationError
        
        with pytest.raises(ValidationError):
            VIPLevelCreateSchema(
                level=1,
                discount_percent=101,  # > 100
            )


class TestModelIntegrity:
    """Test model definitions - no DB required"""
    
    def test_asset_model_has_correct_columns(self):
        """Asset model has expected columns"""
        from models import Asset
        columns = [c.name for c in Asset.__table__.columns]
        assert "asset_id" in columns
        assert "asset" in columns
        assert "name" in columns
        assert "decimals" in columns
        assert "status" in columns
    
    def test_symbol_model_has_correct_columns(self):
        """Symbol model has expected columns"""
        from models import Symbol
        columns = [c.name for c in Symbol.__table__.columns]
        assert "symbol_id" in columns
        assert "symbol" in columns
        assert "base_asset_id" in columns
        assert "quote_asset_id" in columns
        assert "base_maker_fee" in columns
        assert "base_taker_fee" in columns
    
    def test_vip_level_model_has_correct_columns(self):
        """VIP Level model has expected columns"""
        from models import VIPLevel
        columns = [c.name for c in VIPLevel.__table__.columns]
        assert "level" in columns
        assert "discount_percent" in columns
        assert "min_volume" in columns
    
    def test_audit_log_model_has_correct_columns(self):
        """Audit Log model has expected columns"""
        from models import AdminAuditLog
        columns = [c.name for c in AdminAuditLog.__table__.columns]
        assert "id" in columns
        assert "admin_id" in columns
        assert "action" in columns
        assert "entity_type" in columns
        assert "old_value" in columns
        assert "new_value" in columns


def run_tests():
    """Run all tests - suitable for CI"""
    import subprocess
    result = subprocess.run(
        ["python", "-m", "pytest", __file__, "-v", "--tb=short"],
        cwd=os.path.dirname(os.path.dirname(os.path.abspath(__file__))),
    )
    return result.returncode


if __name__ == "__main__":
    # When run directly, execute pytest
    sys.exit(run_tests())
