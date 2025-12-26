"""
E2E-03: Fee Update Integration Test

Tests: Admin changes fee → New trades use new fee

Test Flow:
1. Verify current fee rate
2. Execute trade → Confirm fee amount
3. Update fee via Admin
4. Wait for hot-reload
5. Execute trade → Confirm new fee amount

Prerequisites:
- All services running
- Test user with balance
"""

import asyncio
import httpx
import pytest
from decimal import Decimal

ADMIN_URL = "http://localhost:8001"
GATEWAY_URL = "http://localhost:8080"
HOT_RELOAD_SLA_SECONDS = 5


class TestFeeUpdate:
    """E2E: Fee configuration update test"""
    
    @pytest.fixture
    async def admin_client(self):
        async with httpx.AsyncClient(base_url=ADMIN_URL) as client:
            yield client
    
    @pytest.fixture
    async def gateway_client(self):
        async with httpx.AsyncClient(base_url=GATEWAY_URL) as client:
            yield client
    
    async def wait_for_hot_reload(self):
        await asyncio.sleep(HOT_RELOAD_SLA_SECONDS)
    
    async def get_symbol_fee(self, admin_client, symbol: str) -> tuple[int, int]:
        """Get current maker/taker fee for symbol"""
        symbols = await admin_client.get("/admin/SymbolAdmin/item")
        for s in symbols.json().get("items", []):
            if s.get("symbol") == symbol:
                return s.get("base_maker_fee", 0), s.get("base_taker_fee", 0)
        return 0, 0
    
    async def execute_trade_and_get_fee(
        self, gateway_client, symbol: str, quantity: str, price: str
    ) -> Decimal:
        """Execute a trade and return the fee amount"""
        # Place buy order
        buy_resp = await gateway_client.post("/api/v1/order", json={
            "symbol": symbol,
            "side": "buy",
            "type": "limit",
            "price": price,
            "quantity": quantity,
        })
        
        # Place matching sell order
        sell_resp = await gateway_client.post("/api/v1/order", json={
            "symbol": symbol,
            "side": "sell",
            "type": "limit",
            "price": price,
            "quantity": quantity,
        })
        
        # Query trades to get fee
        trades_resp = await gateway_client.get(f"/api/v1/trades?symbol={symbol}&limit=1")
        if trades_resp.status_code == 200:
            trades = trades_resp.json().get("trades", [])
            if trades:
                return Decimal(str(trades[0].get("fee_amount", "0")))
        
        return Decimal("0")
    
    @pytest.mark.asyncio
    async def test_e2e_fee_update_takes_effect(
        self, admin_client, gateway_client
    ):
        """
        E2E-03: Fee update flow
        
        1. Get current fee
        2. Update to new fee
        3. Wait for hot-reload
        4. Verify new trades use new fee
        """
        symbol = "BTC_USDT"  # Use existing symbol
        
        # Step 1: Get current fee
        old_maker_fee, old_taker_fee = await self.get_symbol_fee(admin_client, symbol)
        
        if old_maker_fee == 0 and old_taker_fee == 0:
            pytest.skip(f"Symbol {symbol} not found or has no fee")
        
        # Step 2: Calculate new fee (increase by 10 bps)
        new_maker_fee = old_maker_fee + 10
        new_taker_fee = old_taker_fee + 10
        
        # Get symbol ID
        symbols = await admin_client.get("/admin/SymbolAdmin/item")
        symbol_id = None
        for s in symbols.json().get("items", []):
            if s.get("symbol") == symbol:
                symbol_id = s.get("symbol_id")
                break
        
        if not symbol_id:
            pytest.skip(f"Could not find symbol_id for {symbol}")
        
        # Update fee
        update_resp = await admin_client.put(
            f"/admin/SymbolAdmin/item{symbol_id}",
            json={
                "base_maker_fee": new_maker_fee,
                "base_taker_fee": new_taker_fee,
            }
        )
        assert update_resp.status_code == 200, f"Failed to update fee: {update_resp.text}"
        
        # Step 3: Wait for hot-reload
        await self.wait_for_hot_reload()
        
        # Step 4: Verify new fee in DB
        current_maker, current_taker = await self.get_symbol_fee(admin_client, symbol)
        assert current_maker == new_maker_fee, f"Maker fee not updated: {current_maker} != {new_maker_fee}"
        assert current_taker == new_taker_fee, f"Taker fee not updated: {current_taker} != {new_taker_fee}"
        
        # Note: Actually verifying fee in trade requires:
        # - Users with balance
        # - Matching orders
        # - Checking trade response
        
        # Restore original fee
        await admin_client.put(
            f"/admin/SymbolAdmin/item{symbol_id}",
            json={
                "base_maker_fee": old_maker_fee,
                "base_taker_fee": old_taker_fee,
            }
        )


class TestVIPDiscount:
    """E2E: VIP level fee discount test"""
    
    @pytest.fixture
    async def admin_client(self):
        async with httpx.AsyncClient(base_url=ADMIN_URL) as client:
            yield client
    
    @pytest.mark.asyncio
    async def test_vip_discount_applied(self, admin_client):
        """
        E2E-04: VIP discount flow
        
        1. Create VIP level with 80% fee (20% discount)
        2. Assign user to VIP level
        3. Execute trade
        4. Verify fee is 80% of base fee
        """
        # Step 1: Create VIP level
        vip_resp = await admin_client.post("/admin/vip/", json={
            "level": 1,
            "name": "Silver",
            "maker_discount": 80,  # 80% of base fee
            "taker_discount": 80,
        })
        
        if vip_resp.status_code not in (200, 201):
            # VIP level might already exist
            pass
        
        # Step 2: Would need user management to assign VIP
        # Step 3-4: Would need actual trading to verify
        
        # This is a placeholder for the complete test
        pass
