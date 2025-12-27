"""
Multi-Agent QA Edge Case Tests

Tests discovered during 4-Agent QA review:
- Agent A (激进派): Edge cases and boundary conditions
- Agent C (安全专家): Security tests

Run: pytest tests/test_multi_agent_qa.py -v
"""
import pytest
from httpx import AsyncClient, ASGITransport
from main import app


class TestAgentAEdgeCases:
    """Agent A: 激进派 - Edge Case Testing"""

    @pytest.fixture
    async def client(self):
        """Create async test client"""
        transport = ASGITransport(app=app)
        async with AsyncClient(transport=transport, base_url="http://test") as ac:
            yield ac

    @pytest.mark.asyncio
    async def test_ec01_empty_asset_code_rejected(self, client):
        """EC-01-01: Empty asset code should return 422"""
        response = await client.post(
            "/admin/AssetAdmin/item",
            json={"asset": "", "name": "Empty", "decimals": 8, "status": 1, "asset_flags": 7}
        )
        assert response.status_code == 422
        assert "string_too_short" in response.text or "at least 1" in response.text

    @pytest.mark.asyncio
    async def test_ec01_too_long_asset_code_rejected(self, client):
        """EC-01-02: Asset code exceeding 16 chars should return 422"""
        response = await client.post(
            "/admin/AssetAdmin/item",
            json={"asset": "ABCDEFGHIJKLMNOPQ", "name": "TooLong", "decimals": 8, "status": 1, "asset_flags": 7}
        )
        assert response.status_code == 422
        assert "string_too_long" in response.text or "at most 16" in response.text

    @pytest.mark.asyncio
    async def test_ec01_special_chars_rejected(self, client):
        """EC-01-04: Special characters should return 422"""
        response = await client.post(
            "/admin/AssetAdmin/item",
            json={"asset": "BTC@#$", "name": "Special", "decimals": 8, "status": 1, "asset_flags": 7}
        )
        assert response.status_code == 422
        assert "pattern" in response.text.lower()

    @pytest.mark.asyncio
    async def test_ec01_unicode_rejected(self, client):
        """EC-01-05: Unicode characters should return 422"""
        response = await client.post(
            "/admin/AssetAdmin/item",
            json={"asset": "比特币", "name": "Unicode", "decimals": 8, "status": 1, "asset_flags": 7}
        )
        assert response.status_code == 422

    @pytest.mark.asyncio
    async def test_ec02_symbol_base_equals_quote_returns_422_not_500(self, client):
        """EC-02-01: base_asset_id == quote_asset_id should return 422, NOT 500
        
        This was discovered during Multi-Agent QA - API returned 500 instead of 422.
        """
        response = await client.post(
            "/admin/SymbolAdmin/item",
            json={
                "symbol": "SAME_SAME",
                "base_asset_id": 1,
                "quote_asset_id": 1,  # Same as base!
                "price_decimals": 2,
                "qty_decimals": 4,
                "min_qty": 1,
                "status": 1,
                "symbol_flags": 0,
                "base_maker_fee": 10,
                "base_taker_fee": 20
            }
        )
        # Should be 422 (validation error), NOT 500 (server error)
        assert response.status_code == 422, f"Expected 422, got {response.status_code}. BUG: Server should validate base!=quote"


class TestAgentCSecurity:
    """Agent C: 安全专家 - Security Testing"""

    @pytest.fixture
    async def client(self):
        transport = ASGITransport(app=app)
        async with AsyncClient(transport=transport, base_url="http://test") as ac:
            yield ac

    @pytest.mark.asyncio
    async def test_sec03_sql_injection_safely_rejected(self, client):
        """SEC-03: SQL injection attempt should be safely rejected (422)"""
        response = await client.post(
            "/admin/AssetAdmin/item",
            json={"asset": "'; DROP TABLE--", "name": "Inject", "decimals": 8, "status": 1, "asset_flags": 7}
        )
        # Should be rejected by validation, not execute SQL
        assert response.status_code == 422
        # Should not crash (500) or succeed (200)
        assert response.status_code not in [200, 201, 500]

    @pytest.mark.asyncio
    async def test_sec03_xss_in_name_escaped(self, client):
        """SEC-03: XSS in asset name should be handled safely"""
        response = await client.post(
            "/admin/AssetAdmin/item",
            json={
                "asset": "XSSTEST",
                "name": "<script>alert('xss')</script>",
                "decimals": 8,
                "status": 1,
                "asset_flags": 7
            }
        )
        # Either rejected or stored escaped - should not crash
        assert response.status_code in [200, 201, 422]

    @pytest.mark.asyncio
    async def test_sec04_health_no_password_leak(self, client):
        """SEC-04: Health endpoint should not leak sensitive data"""
        response = await client.get("/health")
        assert response.status_code == 200
        text = response.text.lower()
        assert "password" not in text
        assert "secret" not in text
        assert "token" not in text

    @pytest.mark.asyncio
    async def test_sec04_trace_id_header_present(self, client):
        """SEC-04: X-Trace-ID should be present in response headers"""
        response = await client.get("/health")
        assert response.status_code == 200
        trace_id = response.headers.get("x-trace-id")
        assert trace_id is not None, "X-Trace-ID header missing"
        assert len(trace_id) == 26, f"Trace ID should be 26-char ULID, got {len(trace_id)}"
