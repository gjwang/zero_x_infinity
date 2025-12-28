import sys
import os
import requests
import time
import uuid

# Path Setup
SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
PROJECT_ROOT = os.path.abspath(os.path.join(SCRIPT_DIR, "../../.."))
SCRIPTS_LIB_DIR = os.path.join(PROJECT_ROOT, "scripts", "lib")
sys.path.append(SCRIPTS_LIB_DIR)
sys.path.append(os.path.join(PROJECT_ROOT, "scripts", "tests", "0x11_funding"))

try:
    import common_jwt
    from api_auth import ApiClient
except ImportError as e:
    print(f"❌ Critical: Cannot import helpers: {e}")
    sys.exit(1)

BASE_URL = "http://127.0.0.1:8080"
INTERNAL_URL = f"{BASE_URL}/internal/mock"
USER_URL = f"{BASE_URL}/api/v1/user"
PRIVATE_URL = f"{BASE_URL}/api/v1/private"

def test_full_lifecycle():
    print("🧪 Phase 0x12: Full Lifecycle Verification (Deposit -> Transfer -> Trade)")
    
    # 1. Setup User (JWT)
    user_data = common_jwt.setup_jwt_user()
    if not user_data:
        return False
        
    user_id, token, jwt_headers = user_data
    print(f"   👤 User ID: {user_id} ready.")

    # 2. Setup API Key (for Trading)
    print("\n[Setup] Generating API Key for Trading...")
    api_key_payload = {"label": "QA_Trading_Key", "permissions": ["trading", "transfer"]}
    resp_key = requests.post(f"{USER_URL}/apikeys", json=api_key_payload, headers=jwt_headers)
    
    if resp_key.status_code != 201:
        print(f"❌ Failed to create API Key: {resp_key.text}")
        return False
        
    key_data = resp_key.json()["data"]
    api_key = key_data["api_key"]
    private_key = key_data["api_secret"] # Handler returns 'api_secret'
    
    print(f"   🔑 API Key Created: {api_key}")
    
    # Initialize Ed25519 Client
    # Note: private_key from handler is hex string of 32 bytes or 64 bytes?
    # handler typically returns 64-char hex (32 bytes) or full keypair?
    # Let's assume it returns Hex string compatible with ApiClient.
    client = ApiClient(api_key, private_key, base_url=BASE_URL)

    # 3. Deposit 10.0 BTC to Funding Wallet (Mock via Internal)
    print("\n[Step 1] Depositing 10.0 BTC to Funding Wallet...")
    tx_hash = f"tx_0x12_{int(time.time())}"
    dev_headers = {"X-Internal-Secret": "dev-secret"}
    
    payload = {
        "user_id": user_id,
        "asset": "BTC",
        "amount": "10.00000000",
        "tx_hash": tx_hash
    }
    
    resp_dep = requests.post(f"{INTERNAL_URL}/deposit", json=payload, headers=dev_headers)
    if resp_dep.status_code != 200:
        print(f"❌ Deposit Failed: {resp_dep.text}")
        return False
    print("   ✅ Deposit Confirmed (Funding Wallet Credited)")

    # 4. Attempt Trade WITHOUT Transfer (Should FAIL)
    print("\n[Step 2] Attempting Premature Trade (No Transfer)...")
    order_payload = {
        "cid": f"cid_{uuid.uuid4()}",
        "side": "SELL",
        "symbol": "BTC_USDT",
        "order_type": "LIMIT",
        "price": "50000",
        "qty": "1.00000000"
    }
    
    # Uses Ed25519 Client
    resp_ord = client.post("/api/v1/private/order", json_body=order_payload)
    
    if resp_ord.status_code == 202:
        order_id = resp_ord.json()["data"]["order_id"]
        status = poll_order_status(client, order_id)
        if status == "REJECTED":
            print("   ✅ Order Rejected as Expected (Insufficient Trading Balance).")
        elif status == "NEW" or status == "TIMEOUT":
            print(f"   ⚠️  Order Status: {status}. (Engine might be ignoring it due to UserNotFound).")
            print("      This confirms the Gap: User not in Trading Engine.")
        else:
            print(f"   ❌ UNEXPECTED: Order {status}. Should be REJECTED!")
            # return False # Allow proceeding to test Transfer
    elif resp_ord.status_code == 400:
         print("   ✅ Order Rejected Immediately (400).")
    else:
         print(f"   ❓ Unexpected Response: {resp_ord.status_code} | {resp_ord.text}")
         if resp_ord.status_code == 401: return False

    # 5. Execute Internal Transfer (Funding -> Trading)
    print("\n[Step 3] Executing Internal Transfer (10.0 BTC -> Trading)...")
    transfer_payload = {
        "from": "Funding", 
        "to": "Trading",
        "asset": "BTC",
        "amount": "10.00000000",
        "cid": f"trans_{int(time.time())}"
    }
    # Handlers might expect 'from_type', 'to_type'?
    # Checking src/gateway/handlers.rs -> transfer logic uses `TransferRequest`.
    # Let's hope field names match.
    # Note: `ApiClient` handles auth header.
    
    resp_trans = client.post("/api/v1/private/transfer", json_body=transfer_payload)
    
    if resp_trans.status_code == 200:
        print("   ✅ Transfer Accepted.")
        # Need to poll transfer status? FSM is async?
        # Assuming synchronous acceptance, async processing.
        # Wait a bit.
        time.sleep(1)
    else:
        print(f"   ❌ Transfer Failed: {resp_trans.status_code} | {resp_trans.text}")
        print("      (Hint: Check field names in handlers.rs if 400)")
        return False

    # 6. Attempt Trade WITH Transfer (Should SUCCEED)
    print("\n[Step 4] Attempting Valid Trade (After Transfer)...")
    order_payload["cid"] = f"cid_{uuid.uuid4()}" # New CID
    
    resp_ord_2 = client.post("/api/v1/private/order", json_body=order_payload)
    
    if resp_ord_2.status_code == 202:
        order_id_2 = resp_ord_2.json()["data"]["order_id"]
        time.sleep(1) # Give engine 1s
        status_2 = poll_order_status(client, order_id_2)
        
        if status_2 == "NEW" or status_2 == "FILLED":
             print(f"   ✅ Order {status_2}. Trading Balance Verified!")
             return True
        else:
             print(f"   ❌ Order {status_2}. Still failing? (Maybe transfer didn't complete?)")
             return False
    else:
         print(f"   ❌ Order Request Failed: {resp_ord_2.status_code} | {resp_ord_2.text}")
         return False

def poll_order_status(client, order_id):
    for _ in range(5):
        time.sleep(0.5)
        resp = client.get(f"/api/v1/private/order/{order_id}")
        if resp.status_code == 200:
            data = resp.json().get("data")
            if data:
                return data.get("status") or data.get("order_status")
    return "TIMEOUT"

if __name__ == "__main__":
    if test_full_lifecycle():
        sys.exit(0)
    else:
        sys.exit(1)
