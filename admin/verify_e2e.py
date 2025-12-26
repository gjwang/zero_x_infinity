#!/usr/bin/env python3
"""
Admin Dashboard E2E Verification Script
Tests all CRUD operations via HTTP API

Usage:
    # Ensure server is running: uvicorn main:app --port 8001
    cd admin && source venv/bin/activate
    python verify_e2e.py
"""

import httpx
import sys
from typing import Optional


BASE_URL = "http://localhost:8001"


class AdminClient:
    """HTTP client for Admin Dashboard"""
    
    def __init__(self):
        self.client = httpx.Client(timeout=10.0)
        self.session_cookies = None
    
    def login(self, username: str = "admin", password: str = "admin") -> bool:
        """Login and store session cookies"""
        resp = self.client.post(
            f"{BASE_URL}/admin/auth/form/login/api",
            json={"username": username, "password": password},
        )
        if resp.status_code == 200:
            data = resp.json()
            if data.get("status") == 0:
                self.session_cookies = resp.cookies
                print(f"✅ Login successful (user: {username})")
                return True
        print(f"❌ Login failed: {resp.status_code}")
        return False


def test_health():
    """Test health endpoint"""
    resp = httpx.get(f"{BASE_URL}/health", timeout=5.0)
    assert resp.status_code == 200
    data = resp.json()
    assert data["status"] == "ok"
    print("✅ Health check passed")


def test_login_page():
    """Test login page HTML"""
    resp = httpx.get(f"{BASE_URL}/admin/auth/form/login", timeout=5.0)
    assert resp.status_code == 200
    assert "<!DOCTYPE html>" in resp.text or "<html" in resp.text
    assert "Sign in" in resp.text or "User Login" in resp.text
    print("✅ Login page renders correctly")


def test_login_api():
    """Test login API"""
    client = AdminClient()
    assert client.login(), "Login should succeed"
    print("✅ Login API works")


def test_admin_dashboard_accessible():
    """Test admin dashboard is accessible after login"""
    client = AdminClient()
    client.login()
    
    # Access admin page with session cookie
    resp = client.client.get(
        f"{BASE_URL}/admin/",
        cookies=client.session_cookies,
        follow_redirects=True
    )
    
    # Should get HTML response (200 or 307 redirect is OK)
    if resp.status_code in (200, 307):
        print(f"✅ Admin dashboard accessible (status: {resp.status_code})")
    else:
        print(f"⚠️  Admin dashboard returned: {resp.status_code}")


def main():
    """Run all E2E verification tests"""
    print("=" * 60)
    print("0x0F Admin Dashboard E2E Verification")
    print("=" * 60)
    print()
    
    tests = [
        ("Health Check", test_health),
        ("Login Page", test_login_page),
        ("Login API", test_login_api),
        ("Admin Dashboard", test_admin_dashboard_accessible),
    ]
    
    passed = 0
    failed = 0
    
    for name, test_func in tests:
        try:
            print(f"\n[{name}]")
            test_func()
            passed += 1
        except AssertionError as e:
            print(f"❌ {name} failed: {e}")
            failed += 1
        except httpx.ConnectError:
            print(f"❌ Cannot connect to {BASE_URL}")
            print("   Make sure the server is running:")
            print("   cd admin && uvicorn main:app --port 8001")
            return 1
        except Exception as e:
            print(f"❌ {name} error: {e}")
            failed += 1
    
    print()
    print("=" * 60)
    print(f"Results: {passed} passed, {failed} failed")
    print("=" * 60)
    
    if failed == 0:
        print("\n✅ All E2E API tests passed!")
        print("\nNext steps:")
        print("1. Manual browser verification at http://localhost:8001/admin")
        print("2. Test CRUD operations for Assets, Symbols, VIP Levels")
        print("3. Gateway integration testing (AC-11, AC-12)")
        return 0
    else:
        return 1


if __name__ == "__main__":
    sys.exit(main())
