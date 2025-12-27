"""
Test Core Flow - Agent B Tests

Per 0x0F-admin-test-plan.md:
- TC-CORE-13: Symbol CloseOnly flow
- TC-CORE-14: CloseOnly → Trading recovery
- TC-CORE-15: VIP Level skip creation

Agent B focuses on core flow stability and regression testing.
"""

import pytest
from pydantic import ValidationError

from schemas.symbol import SymbolCreateSchema, SymbolUpdateSchema
from schemas.vip_level import VIPLevelCreateSchema


class TestSymbolCloseOnlyFlowAgentB:
    """Agent B (保守派): Core flow tests for CloseOnly state"""
    
    def test_symbol_create_default_trading(self):
        """TC-CORE: Symbol should default to Trading status"""
        schema = SymbolCreateSchema(
            symbol="BTC_USDT",
            base_asset_id=1,
            quote_asset_id=2,
            price_decimals=2,
            qty_decimals=8,
        )
        # Default status should be Trading (1)
        assert schema.status == 1
    
    def test_symbol_transition_trading_to_close_only(self):
        """TC-CORE-13: Transition from Trading to CloseOnly"""
        # Simulate: current status is Trading (1), update to CloseOnly (2)
        update_schema = SymbolUpdateSchema(
            min_qty=100,
            status="CLOSE_ONLY",  # CloseOnly
            symbol_flags=0,
            base_maker_fee=100,
            base_taker_fee=200,
        )
        assert update_schema.status == 2  # CloseOnly
    
    def test_symbol_transition_close_only_to_trading(self):
        """TC-CORE-14: Transition from CloseOnly back to Trading"""
        # Simulate: current status is CloseOnly (2), update to Trading (1)
        update_schema = SymbolUpdateSchema(
            min_qty=100,
            status="ONLINE",  # Back to Trading
            symbol_flags=0,
            base_maker_fee=100,
            base_taker_fee=200,
        )
        assert update_schema.status == 1  # Trading
        assert update_schema.status == 1  # Trading
    
    def test_symbol_transition_trading_to_halt(self):
        """Symbol transition from Trading to Halt"""
        update_schema = SymbolUpdateSchema(
            min_qty=100,
            status="OFFLINE",  # Halt
            symbol_flags=0,
            base_maker_fee=100,
            base_taker_fee=200,
        )
        assert update_schema.status == 0  # Halt
    
    def test_symbol_transition_halt_to_trading(self):
        """Symbol transition from Halt back to Trading"""
        update_schema = SymbolUpdateSchema(
            min_qty=100,
            status="ONLINE",  # Trading
            symbol_flags=0,
            base_maker_fee=100,
            base_taker_fee=200,
        )
        assert update_schema.status == 1  # Trading
        assert update_schema.status == 1


class TestVIPLevelFlowAgentB:
    """Agent B: VIP Level CRUD flow tests"""
    
    def test_vip_level_0_default(self):
        """TC-CORE-12: VIP Level 0 should be Normal (100% fee)"""
        schema = VIPLevelCreateSchema(
            level=0,
            discount_percent=100,  # 100% of original fee
            description="Normal",
        )
        assert schema.level == 0
        assert schema.discount_percent == 100
    
    def test_vip_level_1_with_discount(self):
        """VIP Level 1 with 90% fee (10% discount)"""
        schema = VIPLevelCreateSchema(
            level=1,
            discount_percent=90,
            description="Silver",
        )
        assert schema.level == 1
        assert schema.discount_percent == 90
    
    def test_vip_level_skip_create(self):
        """TC-CORE-15: Create Level 5 skipping 2,3,4
        
        Business rule question: Should this be allowed or rejected?
        Per current implementation: ALLOWED (no consecutive check)
        """
        # This SHOULD work - levels don't need to be consecutive
        schema = VIPLevelCreateSchema(
            level=5,
            discount_percent=50,
            description="Diamond",
        )
        assert schema.level == 5
    
    def test_vip_level_sequential_creation(self):
        """Create VIP levels in sequence"""
        levels = [
            VIPLevelCreateSchema(level=0, discount_percent=100, description="Normal"),
            VIPLevelCreateSchema(level=1, discount_percent=90, description="Silver"),
            VIPLevelCreateSchema(level=2, discount_percent=80, description="Gold"),
            VIPLevelCreateSchema(level=3, discount_percent=70, description="Platinum"),
        ]
        for i, schema in enumerate(levels):
            assert schema.level == i


class TestCRUDFlowAgentB:
    """Agent B: Basic CRUD flow validation"""
    
    def test_asset_create_flow(self):
        """Complete asset creation flow"""
        from schemas.asset import AssetCreateSchema
        
        schema = AssetCreateSchema(
            asset="ETH",
            name="Ethereum",
            decimals=18,
            status="ACTIVE",
        )
        assert schema.asset == "ETH"
        assert schema.name == "Ethereum"
        assert schema.decimals == 18
        assert schema.status == 1
    
    def test_asset_update_flow(self):
        """Complete asset update flow (mutable fields only)"""
        from schemas.asset import AssetUpdateSchema
        
        schema = AssetUpdateSchema(
            name="Ethereum Updated",
            status="DISABLED",  # Disable
            asset_flags=0,
        )
        assert schema.name == "Ethereum Updated"
        assert schema.status == 0
    
    def test_symbol_create_flow(self):
        """Complete symbol creation flow"""
        schema = SymbolCreateSchema(
            symbol="ETH_USDT",
            base_asset_id=2,
            quote_asset_id=3,
            price_decimals=2,
            qty_decimals=4,
            min_qty=1000,
            base_maker_fee=50,
            base_taker_fee=100,
        )
        assert schema.symbol == "ETH_USDT"
        assert schema.base_asset_id == 2
        assert schema.quote_asset_id == 3
    
    def test_symbol_update_flow(self):
        """Complete symbol update flow (mutable fields only)"""
        schema = SymbolUpdateSchema(
            min_qty=500,
            status="ONLINE",
            symbol_flags=7,
            base_maker_fee=30,
            base_taker_fee=80,
        )
        assert schema.min_qty == 500
        assert schema.base_maker_fee == 30


class TestFeeUpdateFlowAgentB:
    """Agent B: Fee update flow tests"""
    
    def test_fee_increase(self):
        """Fee can be increased"""
        schema = SymbolUpdateSchema(
            min_qty=100,
            status="ONLINE",
            symbol_flags=0,
            base_maker_fee=200,  # Increased from 100
            base_taker_fee=300,  # Increased from 200
        )
        assert schema.base_maker_fee == 200
        assert schema.base_taker_fee == 300
    
    def test_fee_decrease(self):
        """Fee can be decreased"""
        schema = SymbolUpdateSchema(
            min_qty=100,
            status="ONLINE",
            symbol_flags=0,
            base_maker_fee=0,  # Reduced to 0
            base_taker_fee=0,  # Reduced to 0
        )
        assert schema.base_maker_fee == 0
        assert schema.base_taker_fee == 0
    
    def test_fee_to_max(self):
        """Fee can be set to maximum"""
        schema = SymbolUpdateSchema(
            min_qty=100,
            status="ONLINE",
            symbol_flags=0,
            base_maker_fee=10000,  # 100%
            base_taker_fee=10000,  # 100%
        )
        assert schema.base_maker_fee == 10000
        assert schema.base_taker_fee == 10000
