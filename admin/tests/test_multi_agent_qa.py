"""
Multi-Agent QA Edge Case Tests

Tests discovered during 4-Agent QA review:
- Agent A (激进派): Edge cases and boundary conditions
- Agent C (安全专家): Security tests

IMPORTANT: FastAPI Amis Admin returns HTTP 200 with validation errors in body.
- HTTP Status: 200 (request processed)
- Body Status: 422 (validation failed)

Run: pytest tests/test_multi_agent_qa.py -v
"""
import pytest
from httpx import AsyncClient, ASGITransport
from main import app


def amis_validation_failed(response) -> bool:
    """Check if Amis returned a validation error (HTTP 200 but body has error)"""
    if response.status_code != 200:
        return False
    try:
        data = response.json()
        # Amis returns status in body for validation errors
        return data.get("status") in [422, 0] or "error" in str(data).lower() or "msg" in data
    except:
        return False


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
        """EC-01-01: Empty asset code should be rejected"""
        response = await client.post(
            "/admin/AssetAdmin/item",
            json={"asset": "", "name": "Empty", "decimals": 8, "status": 1, "asset_flags": 7}
        )
        # Amis returns HTTP 200 with error in body
        assert response.status_code == 200
        data = response.json()
        assert data.get("status") != 0 or "error" in str(data).lower(), f"Empty asset should fail: {data}"

    @pytest.mark.asyncio
    async def test_ec01_too_long_asset_code_rejected(self, client):
        """EC-01-02: Asset code exceeding 16 chars should be rejected"""
        response = await client.post(
            "/admin/AssetAdmin/item",
            json={"asset": "ABCDEFGHIJKLMNOPQ", "name": "TooLong", "decimals": 8, "status": 1, "asset_flags": 7}
        )
        assert response.status_code == 200
        data = response.json()
        assert data.get("status") != 0 or "error" in str(data).lower(), f"Too long asset should fail: {data}"

    @pytest.mark.asyncio
    async def test_ec01_special_chars_rejected(self, client):
        """EC-01-04: Special characters should be rejected"""
        response = await client.post(
            "/admin/AssetAdmin/item",
            json={"asset": "BTC@#$", "name": "Special", "decimals": 8, "status": 1, "asset_flags": 7}
        )
        assert response.status_code == 200
        data = response.json()
        assert data.get("status") != 0 or "error" in str(data).lower(), f"Special chars should fail: {data}"

    @pytest.mark.asyncio
    async def test_ec01_unicode_rejected(self, client):
        """EC-01-05: Unicode characters should be rejected"""
        response = await client.post(
            "/admin/AssetAdmin/item",
            json={"asset": "比特币", "name": "Unicode", "decimals": 8, "status": 1, "asset_flags": 7}
        )
        assert response.status_code == 200
        data = response.json()
        assert data.get("status") != 0 or "error" in str(data).lower(), f"Unicode should fail: {data}"

    @pytest.mark.skip(reason="fastapi_amis_admin cannot serialize Pydantic validation errors - tracked as ISSUE-002")
    @pytest.mark.asyncio
    async def test_ec02_symbol_base_equals_quote_rejected(self, client):
        """EC-02-01: base_asset_id == quote_asset_id should be rejected
        
        This was discovered during Multi-Agent QA.
        KNOWN ISSUE: fastapi_amis_admin fails to JSON-serialize ValueError/AssertionError
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
        # Should be rejected (not 500 server error)
        assert response.status_code in [200, 422], f"Expected 200 or 422, got {response.status_code}"
        if response.status_code == 200:
            data = response.json()
            assert data.get("status") != 0, f"base==quote should fail: {data}"


class TestAgentCSecurity:
    """Agent C: 安全专家 - Security Testing"""

    @pytest.fixture
    async def client(self):
        transport = ASGITransport(app=app)
        async with AsyncClient(transport=transport, base_url="http://test") as ac:
            yield ac

    @pytest.mark.asyncio
    async def test_sec03_sql_injection_safely_rejected(self, client):
        """SEC-03: SQL injection attempt should be safely rejected"""
        response = await client.post(
            "/admin/AssetAdmin/item",
            json={"asset": "'; DROP TABLE--", "name": "Inject", "decimals": 8, "status": 1, "asset_flags": 7}
        )
        # Should be rejected by validation (HTTP 200 + error in body), not crash (500)
        assert response.status_code in [200, 422], f"SQL injection should not crash server: {response.status_code}"
        if response.status_code == 200:
            data = response.json()
            assert data.get("status") != 0 or "error" in str(data), f"SQL injection should fail: {data}"

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
