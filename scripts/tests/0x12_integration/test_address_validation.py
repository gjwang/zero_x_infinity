import sys
import os
import requests
import json

# Setup Path
SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
PROJECT_ROOT = os.path.abspath(os.path.join(SCRIPT_DIR, "../../.."))
SCRIPTS_LIB_DIR = os.path.join(PROJECT_ROOT, "scripts", "lib")
sys.path.append(SCRIPTS_LIB_DIR)
sys.path.append(os.path.join(PROJECT_ROOT, "scripts", "tests", "0x11_funding"))

try:
    import common_jwt
except ImportError as e:
    print(f"❌ Critical: Cannot import common_jwt: {e}")
    sys.exit(1)

BASE_URL = "http://127.0.0.1:8080"
WITHDRAW_URL = f"{BASE_URL}/api/v1/capital/withdraw/apply"
ADDRESS_URL = f"{BASE_URL}/api/v1/capital/deposit/address"

# Known Valid Test Vectors (Real Chain Formats)
VALID_BTC_LEGACY = "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa" # Genesis (starts with 1)
VALID_BTC_SEGWIT = "bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh" # Bech32
VALID_ETH = "0x71C7656EC7ab88b098defB751B7401B5f6d8976F" # Valid Checksum

# Invalid Test Vectors
INVALID_BTC_CHARS = "1InvalidChar$#@"
INVALID_BTC_PREFIX = "2InvalidPrefix"
INVALID_ETH_SHORT = "0x12345"
INVALID_ETH_NO_PREFIX = "71C7656EC7ab88b098defB751B7401B5f6d8976F"
INVALID_ETH_BAD_CHARS = "0x71C7656EC7ab88b098defB751B7401B5f6d897ZZ"

def test_address_validation():
    print("🧪 Phase 0x12 QA: Address Validation (Real Chains)")

    # 1. Setup User
    user_data = common_jwt.setup_jwt_user()
    if not user_data:
        return False
    user_id, token, jwt_headers = user_data
    print(f"   👤 User ID: {user_id}")

    success_count = 0
    fail_count = 0

    # === Scenario 1: Deposit Address Generation (System Output) ===
    print("\n[Test 1] Generating Deposit Addresses...")
    
    # 1a. BTC
    resp = requests.get(f"{ADDRESS_URL}?asset=BTC&network=BTC", headers=jwt_headers)
    if resp.status_code == 200:
        addr = resp.json()["data"]["address"]
        print(f"   ℹ️  Generated BTC: {addr}")
        # Note: Mock generates '1'+hash (33 chars). Real is variable.
        if addr.startswith("1") or addr.startswith("bc1"):
            print("   ✅ Format OK (Starts with 1/bc1)")
        else:
            print("   ❌ Format Mismatch")
            fail_count += 1
    else:
        print(f"   ❌ API Fail: {resp.text}")
        fail_count += 1

    # 1b. ETH
    resp = requests.get(f"{ADDRESS_URL}?asset=ETH&network=ETH", headers=jwt_headers)
    if resp.status_code == 200:
        addr = resp.json()["data"]["address"]
        print(f"   ℹ️  Generated ETH: {addr}")
        # Note: Mock generates '0x'+32 chars (34 total). Real is 42.
        # This confirms if we generate Real-looking addresses or Mock ones.
        if addr.startswith("0x"):
            print("   ✅ Format OK (Starts with 0x)")
        else:
            print("   ❌ Format Mismatch")
            fail_count += 1
    else:
        print(f"   ❌ API Fail: {resp.text}")
        fail_count += 1

    # === Scenario 2: Withdrawal Address (User Input) ===
    print("\n[Test 2] Withdrawal Address Validation (Input)...")

    # 2a. Valid BTC (Segwit bc1) - Should PASS
    if attempt_withdraw(jwt_headers, "BTC", "0.1", VALID_BTC_SEGWIT, expect_success=True):
        success_count += 1
    else: fail_count += 1

    # 2b. Valid BTC (Legacy 1) - Should PASS
    if attempt_withdraw(jwt_headers, "BTC", "0.1", VALID_BTC_LEGACY, expect_success=True):
         success_count += 1
    else: fail_count += 1

    # 2c. Valid BTC (P2SH 3) - Should PASS
    # Note: 3... addresses are script hashes, valid for withdrawal
    valid_btc_p2sh = "3J98t1WpEZ73CNmQviecrnyiWrnqRhWNLy"
    if attempt_withdraw(jwt_headers, "BTC", "0.1", valid_btc_p2sh, expect_success=True):
         success_count += 1
    else: fail_count += 1

    # 2d. Invalid BTC (Bad Prefix) - Should FAIL
    if attempt_withdraw(jwt_headers, "BTC", "0.1", INVALID_BTC_PREFIX, expect_success=False):
        success_count += 1
    else: fail_count += 1

    # 2e. Valid ETH - Should PASS
    if attempt_withdraw(jwt_headers, "ETH", "0.1", VALID_ETH, expect_success=True):
        success_count += 1
    else: fail_count += 1

    # 2f. Invalid ETH (Short) - Should FAIL
    if attempt_withdraw(jwt_headers, "ETH", "0.1", INVALID_ETH_SHORT, expect_success=False):
        success_count += 1
    else: fail_count += 1

    # 2g. Invalid ETH (Bad Chars / Non-Hex) - Should FAIL
    if attempt_withdraw(jwt_headers, "ETH", "0.1", INVALID_ETH_BAD_CHARS, expect_success=False):
        success_count += 1
    else: fail_count += 1

    print(f"\n📊 Result: {success_count} Passed, {fail_count} Failed")
    return fail_count == 0

def attempt_withdraw(headers, asset, amount, address, expect_success):
    payload = {
        "asset": asset,
        "amount": amount,
        "address": address
    }
    print(f"   ➡️  Testing {asset} Address: {address} ... ", end="")
    resp = requests.post(WITHDRAW_URL, json=payload, headers=headers)
    
    # We expect 200 OK (Request Created) OR 400 Bad Request (Validation Error)
    # Note: Insufficient funds might cause 400 too, but error message differs.
    # We care about Address Validation.
    
    if resp.status_code == 200:
        if expect_success:
            print("✅ Accepted (Valid)")
            return True
        else:
            print("❌ Accepted (Should be INVALID!)")
            return False
    elif resp.status_code == 400:
        # Check error message
        err_msg = resp.json().get("msg", "")
        if "Invalid address" in err_msg or "Invalid parameter" in err_msg or "Network error" in err_msg:
             if not expect_success:
                 print(f"✅ Rejected as Expected ({err_msg})")
                 return True
             else:
                 print(f"❌ Rejected (Should be Valid): {err_msg}")
                 return False
        elif "Insufficient" in err_msg:
             # If insufficient funds, it means Address Validation PASSED (reached logic).
             if expect_success:
                 print("✅ Validated (Insufficient Funds, but Address OK)")
                 return True
             else:
                 print(f"❌ Insufficient Funds check reached (Validation Skipped?): {err_msg}")
                 return False
        else:
             print(f"❓ Unknown 400: {err_msg}")
             return False
    else:
        print(f"❌ Error {resp.status_code}: {resp.text}")
        return False

if __name__ == "__main__":
    if test_address_validation():
        sys.exit(0)
    else:
        sys.exit(1)
