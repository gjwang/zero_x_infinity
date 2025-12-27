"""
E2E Test Configuration and Fixtures

Common configuration for all E2E tests.
"""

import pytest
import httpx
import asyncio
import os

# Environment configuration - ports default to dev values (db_env.sh is source of truth)
ADMIN_PORT = os.getenv("ADMIN_PORT", "8002")
GATEWAY_PORT = os.getenv("GATEWAY_PORT", "8080")
ADMIN_URL = os.getenv("ADMIN_URL", f"http://localhost:{ADMIN_PORT}")
GATEWAY_URL = os.getenv("GATEWAY_URL", f"http://localhost:{GATEWAY_PORT}")
DATABASE_URL = os.getenv("DATABASE_URL", "postgresql://postgres:postgres@localhost:5433/zero_x_infinity")

# SLA Configuration
HOT_RELOAD_SLA_SECONDS = 5

# Test admin credentials
ADMIN_USERNAME = os.getenv("ADMIN_USERNAME", "admin")
ADMIN_PASSWORD = os.getenv("ADMIN_PASSWORD", "admin123")


@pytest.fixture(scope="session")
def event_loop():
    """Create an instance of the default event loop for the test session."""
    loop = asyncio.get_event_loop_policy().new_event_loop()
    yield loop
    loop.close()


@pytest.fixture(scope="session")
async def admin_session():
    """
    Authenticated admin session for the test run.
    
    Logs in once and reuses the session.
    """
    async with httpx.AsyncClient(base_url=ADMIN_URL) as client:
        # Try to login
        login_resp = await client.post("/admin/auth/login", json={
            "username": ADMIN_USERNAME,
            "password": ADMIN_PASSWORD,
        })
        
        if login_resp.status_code != 200:
            pytest.skip(f"Cannot login to Admin: {login_resp.status_code}")
        
        yield client


@pytest.fixture
async def admin_client():
    """Fresh admin client for each test"""
    async with httpx.AsyncClient(base_url=ADMIN_URL) as client:
        yield client


@pytest.fixture
async def gateway_client():
    """Gateway API client"""
    async with httpx.AsyncClient(base_url=GATEWAY_URL) as client:
        yield client


async def wait_for_service(url: str, timeout: int = 30) -> bool:
    """Wait for a service to become available"""
    async with httpx.AsyncClient() as client:
        start = asyncio.get_event_loop().time()
        while asyncio.get_event_loop().time() - start < timeout:
            try:
                resp = await client.get(url)
                if resp.status_code < 500:
                    return True
            except httpx.ConnectError:
                pass
            await asyncio.sleep(1)
    return False


@pytest.fixture(scope="session", autouse=True)
async def ensure_services_running():
    """Check that required services are running before tests"""
    services = [
        (ADMIN_URL + "/admin/", "Admin Dashboard"),
        (GATEWAY_URL + "/health", "Gateway"),
    ]
    
    for url, name in services:
        if not await wait_for_service(url, timeout=5):
            pytest.skip(f"{name} is not running at {url}")


# Test data helpers

async def create_test_asset(client: httpx.AsyncClient, code: str) -> dict:
    """Create a test asset, return asset data"""
    resp = await client.post("/admin/AssetAdmin/item", json={
        "asset": code.upper(),
        "name": f"Test {code}",
        "decimals": 8,
        "status": "ACTIVE",
    })
    if resp.status_code in (200, 201):
        return resp.json()
    return {}


async def create_test_symbol(
    client: httpx.AsyncClient,
    symbol: str,
    base_asset_id: int,
    quote_asset_id: int
) -> dict:
    """Create a test symbol, return symbol data"""
    resp = await client.post("/admin/SymbolAdmin/item", json={
        "symbol": symbol.upper(),
        "base_asset_id": base_asset_id,
        "quote_asset_id": quote_asset_id,
        "price_decimals": 2,
        "qty_decimals": 8,
        "status": "ACTIVE",
        "base_maker_fee": 10,
        "base_taker_fee": 20,
    })
    if resp.status_code in (200, 201):
        return resp.json()
    return {}


async def cleanup_test_data(client: httpx.AsyncClient):
    """Clean up test data created during tests"""
    # Get all assets starting with TEST or E2E
    assets_resp = await client.get("/admin/AssetAdmin/item")
    if assets_resp.status_code == 200:
        for asset in assets_resp.json().get("items", []):
            code = asset.get("asset", "")
            if code.startswith("TEST") or code.startswith("E2E"):
                await client.delete(f"/admin/AssetAdmin/item/{asset['asset_id']}")
    
    # Get all symbols with test assets
    symbols_resp = await client.get("/admin/SymbolAdmin/item")
    if symbols_resp.status_code == 200:
        for symbol in symbols_resp.json().get("items", []):
            name = symbol.get("symbol", "")
            if "TEST" in name or "E2E" in name:
                await client.delete(f"/admin/SymbolAdmin/item/{symbol['symbol_id']}")
