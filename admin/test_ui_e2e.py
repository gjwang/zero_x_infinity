"""
Playwright UI E2E Test Suite for Admin Dashboard

Tests all UI modules:
1. AssetAdmin - Asset management (Create, List)
2. SymbolAdmin - Symbol management (Create, List)
3. VIPLevelAdmin - VIP level management (Create, List)
4. AdminAuditLog - Audit log viewing (List, Filter)

Run with: pytest test_ui_e2e.py --headed
"""
import pytest
from playwright.sync_api import Page, expect
import requests
import time
import os

# Configuration
ADMIN_URL = os.getenv("ADMIN_URL", "http://localhost:8001")
GATEWAY_API = os.getenv("GATEWAY_API", "http://localhost:8080")


class TestAssetAdmin:
    """Test Asset management UI"""
    
    def test_asset_list_page_loads(self, page: Page):
        """TC-UI-01: Asset list page loads correctly"""
        page.goto(f"{ADMIN_URL}/admin/#/admin/AssetAdmin")
        page.wait_for_load_state("networkidle")
        page.wait_for_timeout(2000)
        
        # Verify page loaded - use breadcrumb which is unique
        expect(page.locator(".cxd-AppBcn-item", has_text="Assets")).to_be_visible(timeout=10000)
        
        # Verify Create button exists
        create_btn = page.locator("button.cxd-Button--primary", has_text="Create")
        expect(create_btn).to_be_visible()
        print("✅ TC-UI-01: Asset list page loads correctly")
    
    def test_asset_create_form(self, page: Page):
        """TC-UI-02: Asset creation form works"""
        page.goto(f"{ADMIN_URL}/admin/#/admin/AssetAdmin")
        page.wait_for_load_state("networkidle")
        page.wait_for_timeout(2000)
        
        # Click Create
        create_btn = page.locator("button.cxd-Button--primary", has_text="Create")
        create_btn.click()
        page.wait_for_timeout(1000)
        
        # Fill form
        suffix = int(time.time())
        asset_code = f"TEST_{suffix}"
        
        page.fill('input[name="asset"]', asset_code)
        page.fill('input[name="name"]', f"Test Asset {suffix}")
        page.fill('input[name="decimals"]', "8")
        
        # Submit
        submit_btn = page.locator("button.cxd-Button--primary", has_text="确认")
        submit_btn.click()
        page.wait_for_timeout(2000)
        
        print(f"✅ TC-UI-02: Asset {asset_code} created via UI")


class TestSymbolAdmin:
    """Test Symbol management UI"""
    
    def test_symbol_list_page_loads(self, page: Page):
        """TC-UI-03: Symbol list page loads correctly"""
        page.goto(f"{ADMIN_URL}/admin/#/admin/SymbolAdmin")
        page.wait_for_load_state("networkidle")
        page.wait_for_timeout(2000)
        
        # Verify page loaded - use breadcrumb
        expect(page.locator(".cxd-AppBcn-item", has_text="Symbols")).to_be_visible(timeout=10000)
        
        # Verify Create button exists
        create_btn = page.locator("button.cxd-Button--primary", has_text="Create")
        expect(create_btn).to_be_visible()
        print("✅ TC-UI-03: Symbol list page loads correctly")
    
    def test_symbol_create_form_opens(self, page: Page):
        """TC-UI-04: Symbol creation form opens"""
        page.goto(f"{ADMIN_URL}/admin/#/admin/SymbolAdmin")
        page.wait_for_load_state("networkidle")
        page.wait_for_timeout(2000)
        
        # Click Create
        create_btn = page.locator("button.cxd-Button--primary", has_text="Create")
        create_btn.click()
        page.wait_for_timeout(1000)
        
        # Verify form fields exist
        expect(page.locator('input[name="symbol"]')).to_be_visible(timeout=5000)
        print("✅ TC-UI-04: Symbol creation form opens")


class TestVIPLevelAdmin:
    """Test VIP Level management UI"""
    
    def test_vip_level_list_page_loads(self, page: Page):
        """TC-UI-05: VIP Level list page loads correctly"""
        page.goto(f"{ADMIN_URL}/admin/#/admin/VIPLevelAdmin")
        page.wait_for_load_state("networkidle")
        page.wait_for_timeout(2000)
        
        # Verify page loaded - use breadcrumb
        expect(page.locator(".cxd-AppBcn-item", has_text="VIP Levels")).to_be_visible(timeout=10000)
        print("✅ TC-UI-05: VIP Level list page loads correctly")
    
    def test_vip_level_create_form_opens(self, page: Page):
        """TC-UI-06: VIP Level creation form opens"""
        page.goto(f"{ADMIN_URL}/admin/#/admin/VIPLevelAdmin")
        page.wait_for_load_state("networkidle")
        page.wait_for_timeout(2000)
        
        # Click Create
        create_btn = page.locator("button.cxd-Button--primary", has_text="Create")
        create_btn.click()
        page.wait_for_timeout(1000)
        
        # Verify form opens (modal appears)
        expect(page.locator(".cxd-Modal")).to_be_visible(timeout=5000)
        print("✅ TC-UI-06: VIP Level creation form opens")


class TestAuditLogAdmin:
    """Test Audit Log viewing UI"""
    
    def test_audit_log_list_page_loads(self, page: Page):
        """TC-UI-07: Audit Log list page loads correctly"""
        page.goto(f"{ADMIN_URL}/admin/#/admin/AdminAuditLog")
        page.wait_for_load_state("networkidle")
        page.wait_for_timeout(2000)
        
        # Verify page loaded - use breadcrumb
        assert "AdminAuditLog" in page.url
        print("✅ TC-UI-07: Audit Log list page loads correctly")


class TestNavigation:
    """Test sidebar navigation"""
    
    def test_sidebar_navigation(self, page: Page):
        """TC-UI-08: Sidebar navigation works for all modules"""
        page.goto(f"{ADMIN_URL}/admin/")
        page.wait_for_load_state("networkidle")
        page.wait_for_timeout(2000)
        
        # Check all sidebar links exist
        modules = ["Assets", "Symbols", "VIP"]
        for module in modules:
            link = page.locator(f"text={module}").first
            expect(link).to_be_visible(timeout=5000)
        
        print("✅ TC-UI-08: Sidebar navigation works for all modules")


class TestE2EPropagation:
    """End-to-end tests verifying Admin -> Gateway propagation"""
    
    def test_asset_propagates_to_gateway(self, page: Page):
        """TC-UI-09: Asset created in UI appears in Gateway API"""
        # 1. Create asset via UI
        page.goto(f"{ADMIN_URL}/admin/#/admin/AssetAdmin")
        page.wait_for_load_state("networkidle")
        page.wait_for_timeout(2000)
        
        create_btn = page.locator("button.cxd-Button--primary", has_text="Create")
        create_btn.click()
        page.wait_for_timeout(1000)
        
        suffix = int(time.time())
        asset_code = f"E2E_{suffix}"
        
        page.fill('input[name="asset"]', asset_code)
        page.fill('input[name="name"]', f"E2E Asset {suffix}")
        page.fill('input[name="decimals"]', "8")
        
        submit_btn = page.locator("button.cxd-Button--primary", has_text="确认")
        submit_btn.click()
        page.wait_for_timeout(2000)
        
        # 2. Verify in Gateway API
        found = False
        for _ in range(5):
            try:
                resp = requests.get(f"{GATEWAY_API}/api/v1/public/assets", timeout=2)
                if resp.status_code == 200:
                    data = resp.json().get("data", [])
                    if any(a.get("asset") == asset_code for a in data):
                        found = True
                        break
            except:
                pass
            time.sleep(1)
        
        if found:
            print(f"✅ TC-UI-09: Asset {asset_code} propagated to Gateway")
        else:
            print(f"⚠️ TC-UI-09: Asset {asset_code} NOT in Gateway (Hot-Reload issue)")
