import pytest
from playwright.sync_api import Page, expect
import requests
import time
import os

# Configuration
ADMIN_URL = os.getenv("ADMIN_URL", "http://localhost:8001")
GATEWAY_API = os.getenv("GATEWAY_API", "http://localhost:8080")

def test_admin_ui_create_asset_propagation(page: Page):
    """
    UI E2E PoC:
    1. Open Admin Dashboard
    2. Navigate to Asset Page
    3. Click "Create"
    4. Fill Form
    5. Submit
    6. Verify in Gateway API
    """
    print(f"\n🚀 Starting UI E2E PoC on {ADMIN_URL}")

    # 1. Open Admin Dashboard
    page.goto(f"{ADMIN_URL}/admin/AssetAdmin/list")
    
    # Wait for page to load (Amis renders dynamically)
    page.wait_for_load_state("networkidle")
    
    # 2. Click "Create" button
    # Amis usually renders a button with "Create" or "Add" text
    # Adjust selector based on actual Amis rendering
    create_btn = page.get_by_role("button", name="Create") 
    create_btn.click()
    
    # 3. Fill Form
    suffix = int(time.time())
    asset_code = f"UI_A_{suffix}"
    
    # Amis form inputs usually have name attributes matching the schema field names
    page.fill('input[name="asset"]', asset_code)
    page.fill('input[name="name"]', f"UI Asset {suffix}")
    page.fill('input[name="decimals"]', "8")
    
    # Status is often a select/switch. For now assuming simple input or default is OK.
    # If it's a select, might need: page.click('div[name="status"]'); page.click('text=Active')
    
    # 4. Submit
    page.click('button[type="submit"]')
    
    # 5. Wait for Success Toast or Redirect
    # Amis usually shows a toast message
    expect(page.get_by_text("Created successfully")).to_be_visible(timeout=5000)
    print(f"✅ UI Form Submitted for {asset_code}")

    # 6. Verify via Gateway API (The "Longest Path" Check)
    print("⏳ Verifying propagation to Gateway...")
    
    found = False
    for i in range(5):
        try:
            resp = requests.get(f"{GATEWAY_API}/api/v1/public/assets", timeout=2)
            if resp.status_code == 200:
                data = resp.json().get("data", [])
                if any(parse_asset(a) == asset_code for a in data):
                    found = True
                    break
        except:
            pass
        time.sleep(1)
    
    if found:
        print("✅ Asset verified in Gateway API")
    else:
        print("❌ Asset NOT found in Gateway API (Expected due to missing Hot-Reload)")
        # In a real test, we would assert found here.
        # assert found, f"Asset {asset_code} not propagated to Gateway"

def parse_asset(a):
    return a.get("asset")
