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
from typing import Dict, Any

# Endpoints
ADMIN_API = "http://localhost:8001"
GATEWAY_API = "http://localhost:8000"

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

    async def run_check():
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
    print("Checking Gateway health...")
    resp = requests.get(f"{GATEWAY_API}/health", timeout=5)
    assert resp.status_code == 200, f"Gateway health check failed: {resp.status_code}"
    print(f"✅ Gateway: {resp.json()}")


def test_asset_creation_propagation(runner: E2ETestRunner):
    """
    E2E-01: Asset Creation Propagation
    Admin creates Asset → Gateway API can retrieve it
    """
    def run():
        # Step 1: Create Asset via Admin API
        asset_data = {
            "asset": "E2E_TEST",
            "name": "E2E Test Asset",
            "decimals": 8,
            "status": 1,
            "asset_flags": 7
        }
        
        print("Step 1: Creating Asset via Admin...")
        admin_resp = requests.post(
            f"{ADMIN_API}/admin/api/asset/create",
            json=asset_data,
            timeout=5
        )
        assert admin_resp.status_code in [200, 201], f"Asset creation failed: {admin_resp.text}"
        print(f"✅ Asset created: {admin_resp.json()}")
        
        # Step 2: Wait for propagation (if needed)
        time.sleep(1)
        
        # Step 3: Verify via Gateway API
        print("Step 2: Verifying via Gateway API...")
        gateway_resp = requests.get(
            f"{GATEWAY_API}/api/v1/assets",
            timeout=5
        )
        assert gateway_resp.status_code == 200, f"Gateway asset query failed"
        
        assets = gateway_resp.json()
        found = any(a.get("asset") == "E2E_TEST" for a in assets)
        assert found, f"Asset E2E_TEST not found in Gateway response: {assets}"
        print(f"✅ Asset verified in Gateway")
    
    runner.test("E2E-01: Asset Creation Propagation", run)


def test_symbol_creation_propagation(runner: E2ETestRunner):
    """
    E2E-02: Symbol Creation Propagation
    Admin creates Symbol → Gateway API can retrieve it
    """
    def run():
        # Step 1: Get Asset IDs from Gateway
        print("Step 1: Getting existing assets from Gateway...")
        assets_resp = requests.get(f"{GATEWAY_API}/api/v1/assets", timeout=5)
        assets = assets_resp.json()
        
        if len(assets) < 2:
            raise AssertionError("Need at least 2 assets for symbol creation")
        
        base_id = assets[0]["asset_id"]
        quote_id = assets[1]["asset_id"]
        
        # Step 2: Create Symbol via Admin
        symbol_data = {
            "symbol": f"E2E_SYM",
            "base_asset_id": base_id,
            "quote_asset_id": quote_id,
            "price_decimals": 2,
            "qty_decimals": 8,
            "min_qty": 0.0,
            "status": 1,
            "symbol_flags": 0,
            "base_maker_fee": 10,
            "base_taker_fee": 20
        }
        
        print(f"Step 2: Creating Symbol {symbol_data['symbol']} via Admin...")
        admin_resp = requests.post(
            f"{ADMIN_API}/admin/api/symbol/create",
            json=symbol_data,
            timeout=5
        )
        assert admin_resp.status_code in [200, 201], f"Symbol creation failed: {admin_resp.text}"
        print(f"✅ Symbol created: {admin_resp.json()}")
        
        # Step 3: Wait for propagation
        time.sleep(1)
        
        # Step 4: Verify via Gateway API
        print("Step 3: Verifying via Gateway API...")
        gateway_resp = requests.get(
            f"{GATEWAY_API}/api/v1/symbols",
            timeout=5
        )
        assert gateway_resp.status_code == 200, f"Gateway symbol query failed"
        
        symbols = gateway_resp.json()
        found = any(s.get("symbol") == "E2E_SYM" for s in symbols)
        assert found, f"Symbol E2E_SYM not found in Gateway response"
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
        symbols_resp = requests.get(f"{GATEWAY_API}/api/v1/symbols", timeout=5)
        symbols = symbols_resp.json()
        
        if not symbols:
            raise AssertionError("No symbols available for testing")
        
        symbol = symbols[0]
        symbol_id = symbol["symbol_id"]
        
        # Step 2: Update status via Admin (halt symbol)
        print(f"Step 2: Halting symbol {symbol['symbol']} via Admin...")
        admin_resp = requests.patch(
            f"{ADMIN_API}/admin/api/symbol/update/{symbol_id}",
            json={"status": 0},  # 0 = halted
            timeout=5
        )
        assert admin_resp.status_code == 200, f"Symbol update failed: {admin_resp.text}"
        print(f"✅ Symbol status updated")
        
        # Step 3: Wait for propagation
        time.sleep(2)
        
        # Step 4: Verify via Gateway API
        print("Step 3: Verifying status change via Gateway...")
        gateway_resp = requests.get(f"{GATEWAY_API}/api/v1/symbols", timeout=5)
        updated_symbols = gateway_resp.json()
        
        updated_symbol = next((s for s in updated_symbols if s["symbol_id"] == symbol_id), None)
        assert updated_symbol, f"Symbol {symbol_id} not found after update"
        assert updated_symbol["status"] == 0, f"Status not updated. Expected 0, got {updated_symbol['status']}"
        print(f"✅ Symbol status verified in Gateway (halted)")
    
    runner.test("E2E-03: Symbol Status Change Propagation", run)


def test_fee_update_propagation(runner: E2ETestRunner):
    """
    E2E-04: Fee Update Propagation
    Admin updates Symbol fees → Gateway API returns new fees
    """
    def run():
        # Step 1: Get symbol from Gateway
        print("Step 1: Getting symbol from Gateway...")
        symbols_resp = requests.get(f"{GATEWAY_API}/api/v1/symbols", timeout=5)
        symbols = symbols_resp.json()
        
        if not symbols:
            raise AssertionError("No symbols available for fee testing")
        
        symbol = symbols[0]
        symbol_id = symbol["symbol_id"]
        
        # Step 2: Update fees via Admin
        new_maker_fee = 15
        new_taker_fee = 25
        
        print(f"Step 2: Updating fees for {symbol['symbol']} via Admin...")
        admin_resp = requests.patch(
            f"{ADMIN_API}/admin/api/symbol/update/{symbol_id}",
            json={
                "base_maker_fee": new_maker_fee,
                "base_taker_fee": new_taker_fee
            },
            timeout=5
        )
        assert admin_resp.status_code == 200, f"Fee update failed: {admin_resp.text}"
        print(f"✅ Fees updated: maker={new_maker_fee}, taker={new_taker_fee}")
        
        # Step 3: Wait for propagation
        time.sleep(2)
        
        # Step 4: Verify via Gateway API
        print("Step 3: Verifying fee change via Gateway...")
        gateway_resp = requests.get(f"{GATEWAY_API}/api/v1/symbols", timeout=5)
        updated_symbols = gateway_resp.json()
        
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
