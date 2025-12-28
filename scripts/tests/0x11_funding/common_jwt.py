
import requests
import random
import string

GATEWAY_URL = "http://localhost:8080"

def get_random_string(length):
    letters = string.ascii_lowercase
    return ''.join(random.choice(letters) for i in range(length))

def setup_jwt_user():
    """
    Registers a new user and returns (user_id, token, headers)
    """
    username = f"qa_user_{get_random_string(6)}"
    email = f"{username}@example.com"
    password = "password123"
    
    # 1. Register
    resp = requests.post(f"{GATEWAY_URL}/api/v1/auth/register", json={
        "username": username, "email": email, "password": password
    })
    if resp.status_code != 201:
        raise Exception(f"Registration failed: {resp.text}")
    
    user_id = resp.json()['data']
    
    # 2. Login
    resp = requests.post(f"{GATEWAY_URL}/api/v1/auth/login", json={
        "email": email, "password": password
    })
    if resp.status_code != 200:
        raise Exception(f"Login failed: {resp.text}")
        
    token = resp.json()['data']['token']
    headers = {"Authorization": f"Bearer {token}"}
    
    print(f"🔑 [Setup] Valid JWT acquired for user {username} (ID: {user_id})")
    return user_id, token, headers
