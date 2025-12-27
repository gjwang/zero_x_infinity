import requests
import sys
import time
import random
import string
import json

GATEWAY_URL = "http://localhost:8080"

def get_random_string(length):
    letters = string.ascii_lowercase
    return ''.join(random.choice(letters) for i in range(length))

def verify_api_keys():
    print(f"Checking Gateway health at {GATEWAY_URL}...")
    try:
        resp = requests.get(f"{GATEWAY_URL}/api/v1/health")
        if resp.status_code != 200:
            print(f"Gateway not healthy: {resp.status_code}")
            sys.exit(1)
    except Exception as e:
        print(f"Failed to connect to Gateway: {e}")
        sys.exit(1)

    # 1. Register & Login
    username = f"trader_{get_random_string(6)}"
    email = f"{username}@test.com"
    password = "strongpassword123"

    print(f"\n[1] Registering {username}...")
    resp = requests.post(f"{GATEWAY_URL}/api/v1/auth/register", json={
        "username": username, "email": email, "password": password
    })
    if resp.status_code != 201:
        print(f"Registration failed: {resp.text}")
        sys.exit(1)

    print(f"[2] Logging in...")
    resp = requests.post(f"{GATEWAY_URL}/api/v1/auth/login", json={
        "email": email, "password": password
    })
    token = resp.json()['data']['token']
    headers = {"Authorization": f"Bearer {token}"}
    print(f"✅ Logged in. Token set.")

    # 2. Create API Key
    label = "My Trading Bot"
    print(f"\n[3] Creating API Key (Label: {label})...")
    resp = requests.post(f"{GATEWAY_URL}/api/v1/user/apikeys", headers=headers, json={"label": label})
    
    if resp.status_code == 201:
        data = resp.json()['data']
        api_key = data['api_key']
        api_secret = data['api_secret']
        print(f"✅ Generated Key: {api_key}")
        print(f"   Secret length: {len(api_secret)}")
    else:
        print(f"❌ Failed to create API Key: {resp.text}")
        sys.exit(1)

    # 3. List API Keys
    print(f"\n[4] Listing API Keys...")
    resp = requests.get(f"{GATEWAY_URL}/api/v1/user/apikeys", headers=headers)
    if resp.status_code == 200:
        keys = resp.json()['data']
        print(f"✅ Found {len(keys)} keys.")
        found = False
        for k in keys:
            print(f"   - {k['api_key']} ({k['label']})")
            if k['api_key'] == api_key:
                found = True
        if not found:
            print(f"❌ Newly created key not found in list!")
            sys.exit(1)
    else:
        print(f"❌ Failed to list keys: {resp.text}")
        sys.exit(1)

    # 4. Delete API Key
    print(f"\n[5] Deleting API Key {api_key}...")
    resp = requests.delete(f"{GATEWAY_URL}/api/v1/user/apikeys/{api_key}", headers=headers)
    if resp.status_code == 200:
        print(f"✅ Deleted successfully.")
    else:
        print(f"❌ Failed to delete key: {resp.text}")
        sys.exit(1)

    # 5. Verify Deletion
    print(f"\n[6] Verifying Deletion...")
    resp = requests.get(f"{GATEWAY_URL}/api/v1/user/apikeys", headers=headers)
    keys = resp.json()['data']
    for k in keys:
        if k['api_key'] == api_key:
            print(f"❌ Key still exists after deletion!")
            sys.exit(1)
    print(f"✅ Key gone from list.")

    print("\n🎉 API Key Management Verification Passed!")

if __name__ == "__main__":
    verify_api_keys()
