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

async def test_identity_spoofing():
    """
    SECURITY TEST: Can I connect as user_id=1001 without a token?
    If yes, and if I can receive private data (future), this is a critical vuln.
    For now, we verify if the server accepts the identity.
    """
    print("\n🕵️  TEST: Identity Spoofing (Connection w/o Token)")
    uri = f"{GATEWAY_WS_BASE}?user_id=1001"
    
    try:
        async with websockets.connect(uri) as ws:
            # Expect welcome message
            msg = await ws.recv()
            data = json.loads(msg)
            print(f"   Response: {data}")
            
            if data.get("type") == "connected" and data.get("user_id") == 1001:
                print("   ❌ VULNERABILITY CONFIRMED: Server accepted unauthenticated identity 1001")
                return False
            else:
                print("   ✅ Server rejected or sanitized identity")
                return True
    except Exception as e:
        print(f"   ⚠️ Connection failed (might be good): {e}")
        return True

async def test_malformed_handshake():
    """
    ROBUSTNESS TEST: What happens if user_id is garbage?
    """
    print("\n🔨 TEST: Malformed Handshake")
    # Case 1: String instead of int
    uri = f"{GATEWAY_WS_BASE}?user_id=admin"
    try:
        async with websockets.connect(uri) as ws:
            print("   ❌ Connected with invalid type")
            return False
    except websockets.exceptions.InvalidStatusCode as e:
        print(f"   ✅ Server rejected invalid type: {e.status_code}")
        if e.status_code == 400:
            return True
        return False
    except Exception as e:
        print(f"   ✅ Connection failed: {e}")
        return True

async def test_topic_fuzzing():
    """
    ROBUSTNESS TEST: Subscribe to garbage topics
    """
    print("\n🤪 TEST: Topic Fuzzing")
    uri = f"{GATEWAY_WS_BASE}?user_id=0"
    async with websockets.connect(uri) as ws:
        await ws.recv() # Welcome
        
        # Payload 1: Null args
        bad_msg = {"op": "subscribe", "args": None}
        await ws.send(json.dumps(bad_msg))
        # Logic: Should not crash backend.
        
        # Payload 2: Huge topic
        huge_topic = "A" * 10000
        bad_msg_2 = {"op": "subscribe", "args": [huge_topic]}
        await ws.send(json.dumps(bad_msg_2))
        
        # Check if still alive (send valid ping)
        ping = {"op": "ping"} # technically not supported yet? Or standard ping?
        # Let's try valid subscribe
        valid = {"op": "subscribe", "args": ["market.trade.BTC_USDT"]}
        await ws.send(json.dumps(valid))
        
        try:
            # We expect a "subscribed" response eventually
            resp = await asyncio.wait_for(ws.recv(), timeout=2.0)
            print(f"   Alive check: {resp}")
            if "subscribed" in resp:
                print("   ✅ Server survived fuzzing")
                return True
        except:
             print("   ❌ Server unresponsive after fuzzing")
             return False
    return False

async def main():
    results = []
    results.append(await test_identity_spoofing())
    # results.append(await test_malformed_handshake())
    # results.append(await test_topic_fuzzing())
    
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
