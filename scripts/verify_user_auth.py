import requests
import sys
import time
import random
import string

GATEWAY_URL = "http://localhost:8080"

def get_random_string(length):
    letters = string.ascii_lowercase
    return ''.join(random.choice(letters) for i in range(length))

def verify_user_auth():
    print(f"Checking Gateway health at {GATEWAY_URL}...")
    try:
        resp = requests.get(f"{GATEWAY_URL}/api/v1/health")
        if resp.status_code != 200:
            print(f"Gateway not healthy: {resp.status_code}")
            sys.exit(1)
    except Exception as e:
        print(f"Failed to connect to Gateway: {e}")
        sys.exit(1)

    # 1. Register
    username = f"user_{get_random_string(8)}"
    email = f"{username}@example.com"
    password = "password123"

    print(f"\n[1] Registering user: {username} ({email})...")
    resp = requests.post(f"{GATEWAY_URL}/api/v1/auth/register", json={
        "username": username,
        "email": email,
        "password": password
    })
    
    if resp.status_code == 201:
        data = resp.json()
        user_id = data['data']
        print(f"✅ Registered successfully. User ID: {user_id}")
    else:
        print(f"❌ Registration failed: {resp.text}")
        sys.exit(1)

    # 2. Login
    print(f"\n[2] Logging in...")
    resp = requests.post(f"{GATEWAY_URL}/api/v1/auth/login", json={
        "email": email,
        "password": password
    })

    if resp.status_code == 200:
        data = resp.json()
        token = data['data']['token']
        print(f"✅ Login successful.")
        print(f"   Token: {token[:20]}...")
        print(f"   User ID: {data['data']['user_id']}")
    else:
        print(f"❌ Login failed: {resp.text}")
        sys.exit(1)

    print("\n✅ Verification Passed!")

if __name__ == "__main__":
    verify_user_auth()
