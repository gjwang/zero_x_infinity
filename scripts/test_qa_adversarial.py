#!/usr/bin/env python3
"""
Adversarial QA Test Script
Phase 0x10.5 Backend Gaps

Focus: Security, Boundaries, and Robustness
"""

import asyncio
import websockets
import json
import sys
import time

GATEWAY_WS_BASE = "ws://localhost:8080/ws"

async def register_and_login():
    """Helper to get a valid JWT"""
    import aiohttp
    import random
    
    suffix = random.randint(1000, 9999)
    email = f"user{suffix}@example.com"
    password = "password123"
    username = f"user{suffix}"
    
    async with aiohttp.ClientSession() as session:
        # Register
        reg_url = "http://localhost:8080/api/v1/auth/register"
        async with session.post(reg_url, json={"username": username, "email": email, "password": password}) as resp:
            if resp.status not in [200, 201]:
                print(f"   ⚠️ Register failed: {resp.status} {await resp.text()}")
                return None
            
        # Login
        login_url = "http://localhost:8080/api/v1/auth/login"
        async with session.post(login_url, json={"email": email, "password": password}) as resp:
            if resp.status != 200:
                print(f"   ⚠️ Login failed: {resp.status}")
                return None
            data = await resp.json()
        if "data" in data and "token" in data["data"]:
             return data["data"]["token"]
        return data.get("token") # Fallback just in case

async def test_identity_spoofing():
    """
    SECURITY TEST: Can I connect as verify token logic?
    Expect: Anonymous (old user_id param ignored) or 401 if we send bad token.
    Authentication logic now strictly uses ?token=JWT.
    Any other param like ?user_id=... should be ignored and treated as Anonymous key.
    """
    print("\n🕵️  TEST: Legacy user_id param (Spoofing)")
    uri = f"{GATEWAY_WS_BASE}?user_id=1001"
    
    try:
        async with websockets.connect(uri) as ws:
            # Expect welcome message with user_id=null (Anonymous)
            msg = await ws.recv()
            data = json.loads(msg)
            print(f"   Response: {data}")
            
            # WsMessage::Connected { user_id: Option<u64> }
            # If user_id is null, it's anonymous.
            if data.get("type") == "connected" and data.get("user_id") is None:
                print("   ✅ Server correctly treated as Anonymous (ignored user_id param)")
                return True
            elif data.get("user_id") == 1001:
                print("   ❌ VULNERABILITY: Server accepted user_id param!")
                return False
            else:
                 print(f"   ⚠️ Unexpected behavior: {data}")
                 return False
    except Exception as e:
        print(f"   ⚠️ Connection failed (unexpected): {e}")
        return False

async def test_jwt_auth():
    print("\n🔐 TEST: JWT Authentication")
    token = await register_and_login()
    if not token:
        print("   ⚠️ Skipping JWT test (auth service unreachable?)")
        return False

    # 1. Valid Token
    uri = f"{GATEWAY_WS_BASE}?token={token}"
    try:
        async with websockets.connect(uri) as ws:
            msg = await ws.recv()
            data = json.loads(msg)
            if data.get("user_id") is not None:
                print(f"   ✅ Authenticated as user {data['user_id']}")
            else:
                print("   ❌ Failed to authenticate with valid token")
                return False
            
            # Subscribe Private
            await ws.send(json.dumps({"op": "subscribe", "args": ["order.update"]}))
            resp = await ws.recv()
            if "subscribed" in resp:
                 print("   ✅ Private subscription allowed")
            else:
                 print(f"   ❌ Private subscription failed: {resp}")
                 return False

    except Exception as e:
        print(f"   ❌ Valid token connection failed: {e}")
        return False

    # 2. Invalid Token
    print("   Testing Invalid Token...")
    uri_bad = f"{GATEWAY_WS_BASE}?token=bad.token.123"
    try:
        async with websockets.connect(uri_bad) as ws:
             print("   ❌ Should have rejected invalid token")
             return False
    except getattr(websockets.exceptions, "InvalidStatus", websockets.exceptions.InvalidStatusCode) as e:
        # Check status code (attribute might differ slightly or be same)
        status = getattr(e, "status_code", getattr(e, "response", None).status_code if hasattr(e, "response") else 0)
        # Actually InvalidStatus has .response which has .status_code? No, usually .status_code property exists.
        # Let's inspect e.
        code = 0
        if hasattr(e, "status_code"):
            code = e.status_code
        elif hasattr(e, "response") and hasattr(e.response, "status_code"):
             code = e.response.status_code
        
        if code == 401:
            print("   ✅ Rejected invalid token (401)")
        else:
            print(f"   ❌ Rejected with wrong code: {code} (Error: {e})")
            return False
            
    return True

async def test_private_channel_permissions():
    print("\n🚫 TEST: Private Channel Permissions (Anonymous)")
    uri = f"{GATEWAY_WS_BASE}" # No token
    try:
        async with websockets.connect(uri) as ws:
            # Welcome
            await ws.recv()
            
            # Try to subscribe to private channel
            await ws.send(json.dumps({"op": "subscribe", "args": ["order.update"]}))
            
            # Expect generic error or silence or no 'subscribed'
            # Implementation sends WsMessage::Error
            resp = await ws.recv()
            print(f"   Response to private sub: {resp}")
            if "error" in resp.lower() and "login required" in resp.lower():
                print("   ✅ Private subscription denied (Correct Error)")
                return True
            elif "subscribed" in resp:
                print("   ❌ VULNERABILITY: Anonymous user subscribed to private channel!")
                return False
            else:
                print("   ⚠️ Unexpected response")
                return False
    except Exception as e:
        print(f"   Connection failed: {e}")
        return False

async def main():
    results = []
    # Ensure aiohttp installed
    try:
        import aiohttp
    except ImportError:
        print("Please install aiohttp: pip install aiohttp")
        sys.exit(1)

    results.append(await test_identity_spoofing())
    results.append(await test_jwt_auth())
    results.append(await test_private_channel_permissions())
    
    if all(results):
        print("\n✅ ALL SYSTEM CHECKS PASSED (Secure/Robust)")
        sys.exit(0)
    else:
        print("\n❌ SECURITY/ROBUSTNESS ISSUES DETECTED")
        sys.exit(1)

if __name__ == "__main__":
    try:
        asyncio.run(main())
    except KeyboardInterrupt:
        pass
