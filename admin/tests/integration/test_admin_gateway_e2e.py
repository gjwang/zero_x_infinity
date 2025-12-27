#!/usr/bin/env python3
"""
Real E2E Test: Admin Dashboard → Gateway API
Tests the complete chain: Admin modifies config → Gateway API reflects changes

Usage:
    ./test_admin_gateway_e2e.py
"""

import requests
import time
import sys
import os
from typing import Dict, Any

# Endpoints - use environment variables to support different environments
ADMIN_PORT = os.environ.get("ADMIN_PORT", "8002")
GATEWAY_PORT = os.environ.get("GATEWAY_PORT", "8081")
ADMIN_API = f"http://localhost:{ADMIN_PORT}"
GATEWAY_API = f"http://localhost:{GATEWAY_PORT}"

class E2ETestRunner:
    def __init__(self):
        self.passed = 0
        self.failed = 0
        self.errors = []
    
    def test(self, name: str, func):
        """Run a single test"""
        print(f"\n{'='*60}")
        print(f"TEST: {name}")
        print(f"{'='*60}")
        try:
            func()
            print(f"✅ PASS: {name}")
            self.passed += 1
        except AssertionError as e:
            print(f"❌ FAIL: {name}")
            print(f"   Error: {e}")
            self.errors.append((name, str(e)))
            self.failed += 1
        except Exception as e:
            print(f"💥 ERROR: {name}")
            print(f"   Exception: {e}")
            self.errors.append((name, str(e)))
            self.failed += 1
    
    def report(self):
        """Print final report"""
        print(f"\n{'='*60}")
        print(f"FINAL REPORT")
        print(f"{'='*60}")
        print(f"✅ Passed: {self.passed}")
        print(f"❌ Failed: {self.failed}")
        print(f"Total: {self.passed + self.failed}")
        
        if self.errors:
            print(f"\n{'='*60}")
            print(f"FAILURES:")
            print(f"{'='*60}")
            for name, error in self.errors:
                print(f"\n{name}:")
                print(f"  {error}")
        
        return self.failed == 0


def check_db_integrity():
    """Verify actual PostgreSQL database matches expectations"""
    import asyncio
    from sqlalchemy import text
    from database import AsyncSessionLocal
    import sys

    print("Checking Database Schema Integrity...")
    from database import init_db
    from settings import settings
    
    async def run_check():
        await init_db(settings.database_url)
        async with AsyncSessionLocal() as session:
            # Check symbols_tb columns
            try:
                await session.execute(text("SELECT base_maker_fee, base_taker_fee FROM symbols_tb LIMIT 1"))
                print("  ✅ symbols_tb schema matches models")
            except Exception as e:
                print(f"  ❌ Database schema mismatch in symbols_tb: {e}")
                return False

            # Check admin_audit_log
            try:
                await session.execute(text("SELECT id, action, entity_type FROM admin_audit_log LIMIT 1"))
                print("  ✅ admin_audit_log table exists and matches models")
            except Exception as e:
                print(f"  ❌ Database schema mismatch in admin_audit_log: {e}")
                return False
            
            return True

    # Run the async check in the existing loop or create one
    try:
        loop = asyncio.get_event_loop()
    except RuntimeError:
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
    
    success = loop.run_until_complete(run_check())
    if not success:
        raise AssertionError("Database integrity check failed")


def check_admin_health():
    """Verify Admin Dashboard is running"""
    print("Checking Admin Dashboard health...")
    resp = requests.get(f"{ADMIN_API}/health", timeout=5)
    assert resp.status_code == 200, f"Admin health check failed: {resp.status_code}"
    print(f"✅ Admin Dashboard: {resp.json()}")


def check_gateway_health():
    """Verify Gateway is running"""
    print(f"Checking Gateway health at {GATEWAY_API}/api/v1/health...")
    resp = requests.get(f"{GATEWAY_API}/api/v1/health", timeout=5)
    assert resp.status_code == 200, f"Gateway health check failed: {resp.status_code} {resp.text}"
    print(f"✅ Gateway: {resp.json()}")


def test_asset_creation_propagation(runner: E2ETestRunner):
    """
    E2E-01: Asset Creation Propagation
    Admin creates 2 Assets (needed for Symbol creation) → Gateway API can retrieve them
    """
    def run():
        suffix = int(time.time())
        
        # Create 2 Assets (needed for Symbol creation in E2E-02)
        for i, code_prefix in enumerate(["BASE", "QUOTE"]):
            asset_code = f"{code_prefix}_{suffix}"
            asset_data = {
                "asset": asset_code,
                "name": f"E2E {code_prefix} Asset {suffix}",
                "decimals": 8,
                "status": 1,  # 1 = ACTIVE
                "asset_flags": 7
            }
            
            print(f"Step {i+1}: Creating {code_prefix} Asset via Admin...")
            admin_resp = requests.post(
                f"{ADMIN_API}/admin/AssetAdmin/item",
                json=asset_data,
                timeout=5
            )
            assert admin_resp.status_code in [200, 201], f"Asset creation failed: {admin_resp.text}"
            print(f"✅ {code_prefix} Asset created: {admin_resp.json()}")
        
        # Wait for TTL cache refresh
        print("Waiting for TTL cache refresh (5s + buffer)...")
        time.sleep(10)
        
        # Verify via Gateway API
        print(f"Step 3: Verifying via Gateway API at {GATEWAY_API}/api/v1/public/assets...")
        try:
            gateway_resp = requests.get(
                f"{GATEWAY_API}/api/v1/public/assets",
                timeout=5
            )
            assert gateway_resp.status_code == 200, f"Gateway asset query failed: {gateway_resp.status_code} {gateway_resp.text}"
        except Exception as e:
            raise AssertionError(f"Gateway connection failed: {e}")
        
        assets = gateway_resp.json()["data"]
        base_found = any(a.get("asset") == f"BASE_{suffix}" for a in assets)
        quote_found = any(a.get("asset") == f"QUOTE_{suffix}" for a in assets)
        assert base_found, f"BASE_{suffix} not found in Gateway response"
        assert quote_found, f"QUOTE_{suffix} not found in Gateway response"
        print(f"✅ Both Assets verified in Gateway (total: {len(assets)})")
    
    runner.test("E2E-01: Asset Creation Propagation", run)


def test_symbol_creation_propagation(runner: E2ETestRunner):
    """
    E2E-02: Symbol Creation Propagation
    Admin creates Symbol → Gateway API can retrieve it
    """
    def run():
        # Step 1: Get Asset IDs from Gateway
        print("Step 1: Getting existing assets from Gateway...")
        assets_resp = requests.get(f"{GATEWAY_API}/api/v1/public/assets", timeout=5)
        assets = assets_resp.json()["data"]
        
        if len(assets) < 2:
            raise AssertionError("Need at least 2 assets for symbol creation")
        
        base_id = assets[0]["asset_id"]
        quote_id = assets[1]["asset_id"]
        
        # Step 2: Create Symbol via Admin
        suffix = int(time.time())
        # Regex requires ^[A-Z0-9]+_[A-Z0-9]+$ so we need strict BASE_QUOTE
        # Assuming Base=Asset0, Quote=Asset1
        symbol_code = f"TEST_{suffix}"
        symbol_data = {
            "symbol": symbol_code,
            "base_asset_id": base_id,
            "quote_asset_id": quote_id,
            "price_decimals": 2,
            "qty_decimals": 8,
            "min_qty": 0.0,
            "status": 1,  # 1 = ONLINE
            "symbol_flags": 0,
            "base_maker_fee": 10,
            "base_taker_fee": 20
        }
        
        print(f"Step 2: Creating Symbol {symbol_data['symbol']} via Admin...")
        admin_resp = requests.post(
            f"{ADMIN_API}/admin/SymbolAdmin/item",
            json=symbol_data,
            timeout=5
        )
        assert admin_resp.status_code in [200, 201], f"Symbol creation failed: {admin_resp.text}"
        print(f"✅ Symbol created: {admin_resp.json()}")
        
        # Step 3: Wait for propagation
        time.sleep(10)  # Wait for TTL cache refresh
        
        # Step 4: Verify via Gateway API
        print("Step 3: Verifying via Gateway API...")
        gateway_resp = requests.get(
            f"{GATEWAY_API}/api/v1/public/symbols",
            timeout=5
        )
        assert gateway_resp.status_code == 200, f"Gateway symbol query failed: {gateway_resp.status_code}"
        
        symbols = gateway_resp.json()["data"]
        found = any(s.get("symbol") == symbol_code for s in symbols)
        assert found, f"Symbol {symbol_code} not found in Gateway response"
        print(f"✅ Symbol verified in Gateway")
    
    runner.test("E2E-02: Symbol Creation Propagation", run)


def test_symbol_status_change_propagation(runner: E2ETestRunner):
    """
    E2E-03: Symbol Status Change Propagation
    Admin halts Symbol → Gateway API reflects status change
    """
    def run():
        # Step 1: Get symbol from Gateway
        print("Step 1: Getting symbol from Gateway...")
        symbols_resp = requests.get(f"{GATEWAY_API}/api/v1/public/symbols", timeout=5)
        resp_data = symbols_resp.json()
        symbols = resp_data.get("data", [])
        
        if not symbols:
            raise AssertionError("No symbols available for testing")
        
        symbol = symbols[0]
        symbol_id = symbol["symbol_id"]
        
        # Step 2: Update status via Admin (halt symbol)
        print(f"Step 2: Halting symbol {symbol['symbol']} via Admin...")
        admin_resp = requests.put(
            f"{ADMIN_API}/admin/SymbolAdmin/item/{symbol_id}",
            json={"status": 0},  # 0 = OFFLINE (halted)
            timeout=5
        )
        print(f"PUT Response: status={admin_resp.status_code}, body={admin_resp.text[:200]}")
        assert admin_resp.status_code == 200, f"Symbol update failed: {admin_resp.text}"
        print(f"✅ Symbol status updated")
        
        # Step 3: Wait for propagation
        time.sleep(10)  # Wait for TTL cache refresh
        
        # Step 4: Verify via Gateway API - OFFLINE symbol should NOT be returned
        print("Step 3: Verifying status change via Gateway...")
        gateway_resp = requests.get(f"{GATEWAY_API}/api/v1/public/symbols", timeout=5)
        updated_symbols = gateway_resp.json()["data"]
        
        # OFFLINE (status=0) symbols should NOT appear in Gateway's public API
        # Gateway uses WHERE status=1 to filter only ONLINE symbols
        updated_symbol = next((s for s in updated_symbols if s["symbol_id"] == symbol_id), None)
        assert updated_symbol is None, f"OFFLINE symbol {symbol_id} should NOT appear in Gateway API, but was found"
        print(f"✅ Symbol correctly hidden from Gateway (OFFLINE)")
        
        # Step 5: Restore symbol status so E2E-04 can use it
        print("Step 4: Restoring symbol status...")
        admin_resp = requests.put(
            f"{ADMIN_API}/admin/SymbolAdmin/item/{symbol_id}",
            json={"status": 1},  # 1 = ONLINE
            timeout=5
        )
        assert admin_resp.status_code == 200, f"Symbol restore failed: {admin_resp.text}"
        print(f"✅ Symbol status restored to ONLINE")
        time.sleep(10)  # Wait for TTL cache refresh
    
    runner.test("E2E-03: Symbol Status Change Propagation", run)


def test_fee_update_propagation(runner: E2ETestRunner):
    """
    E2E-04: Fee Update Propagation
    Admin updates Symbol fees → Gateway API returns new fees
    """
    def run():
        # Step 1: Get symbol from Gateway
        print("Step 1: Getting symbol from Gateway...")
        symbols_resp = requests.get(f"{GATEWAY_API}/api/v1/public/symbols", timeout=5)
        resp_data = symbols_resp.json()
        symbols = resp_data.get("data", [])
        
        if not symbols:
            raise AssertionError("No symbols available for fee testing")
        
        symbol = symbols[0]
        symbol_id = symbol["symbol_id"]
        
        # Step 2: Update fees via Admin
        new_maker_fee = 15
        new_taker_fee = 25
        
        print(f"Step 2: Updating fees for {symbol['symbol']} via Admin...")
        admin_resp = requests.put(
            f"{ADMIN_API}/admin/SymbolAdmin/item/{symbol_id}",
            json={
                "base_maker_fee": new_maker_fee,
                "base_taker_fee": new_taker_fee
            },
            timeout=5
        )
        assert admin_resp.status_code == 200, f"Fee update failed: {admin_resp.text}"
        print(f"✅ Fees updated: maker={new_maker_fee}, taker={new_taker_fee}")
        
        # Step 3: Wait for propagation
        # Cache TTL is 3s - wait for cache to expire, then trigger refresh
        print("Waiting for TTL cache expiry (5s)...")
        time.sleep(5)  # Wait for cache to expire
        
        # Step 4: Trigger cache refresh with a dummy request
        print("Triggering cache refresh...")
        _ = requests.get(f"{GATEWAY_API}/api/v1/public/symbols", timeout=5)
        time.sleep(1)  # Let the refresh complete
        
        # Step 5: Verify via Gateway API
        print("Step 4: Verifying fee change via Gateway...")
        gateway_resp = requests.get(f"{GATEWAY_API}/api/v1/public/symbols", timeout=5)
        updated_symbols = gateway_resp.json()["data"]
        
        updated_symbol = next((s for s in updated_symbols if s["symbol_id"] == symbol_id), None)
        assert updated_symbol, f"Symbol {symbol_id} not found after fee update"
        assert updated_symbol["base_maker_fee"] == new_maker_fee, \
            f"Maker fee not updated. Expected {new_maker_fee}, got {updated_symbol['base_maker_fee']}"
        assert updated_symbol["base_taker_fee"] == new_taker_fee, \
            f"Taker fee not updated. Expected {new_taker_fee}, got {updated_symbol['base_taker_fee']}"
        print(f"✅ Fees verified in Gateway")
    
    runner.test("E2E-04: Fee Update Propagation", run)


def main():
    """Run all E2E tests"""
    print("""
╔════════════════════════════════════════════════════════════╗
║  REAL E2E TEST: Admin Dashboard → Gateway API             ║
║  Testing complete chain propagation                        ║
╚════════════════════════════════════════════════════════════╝
""")
    
    # Pre-flight checks
    try:
        check_db_integrity()
        check_admin_health()
        check_gateway_health()
    except Exception as e:
        print(f"\n💥 Pre-flight check failed: {e}")
        # Continuing despite DB integrity failure for full report
    
    # Run E2E tests
    runner = E2ETestRunner()
    
    test_asset_creation_propagation(runner)
    test_symbol_creation_propagation(runner)
    test_symbol_status_change_propagation(runner)
    test_fee_update_propagation(runner)
    
    # Report
    success = runner.report()
    
    if success:
        print("\n🎉 All E2E tests PASSED!")
        return 0
    else:
        print("\n❌ Some E2E tests FAILED")
        return 1


if __name__ == "__main__":
    sys.exit(main())
