#!/usr/bin/env python3
"""
Query balances via API with Ed25519 authentication.

Usage:
    python3 scripts/query_balances.py --user 1001
    python3 scripts/query_balances.py --user 1001 --raw  # Output raw JSON
"""

import argparse
import json
import os
import sys

# Add scripts directory to path for lib imports
SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, SCRIPT_DIR)

try:
    from lib.api_auth import get_test_client
except ImportError:
    print("ERROR: lib/auth.py not found", file=sys.stderr)
    sys.exit(1)

GATEWAY_URL = os.environ.get("GATEWAY_URL", "http://localhost:8080")

# Default assets to query
DEFAULT_ASSETS = [1, 2]  # BTC=1, USDT=2


def query_balance(user_id: int, asset_id: int, client) -> dict:
    """Query single balance for a user/asset pair."""
    path = f"/api/v1/private/balances?user_id={user_id}&asset_id={asset_id}"
    try:
        resp = client.get(path)
        if resp.status_code != 200:
            return {"error": f"HTTP {resp.status_code}", "asset_id": asset_id}
        return resp.json()
    except Exception as e:
        return {"error": str(e), "asset_id": asset_id}


def query_balances(user_id: int, assets: list = None, raw: bool = False) -> dict:
    """Query balances for a user via authenticated API."""
    client = get_test_client(GATEWAY_URL)
    assets = assets or DEFAULT_ASSETS
    
    results = []
    for asset_id in assets:
        data = query_balance(user_id, asset_id, client)
        if data.get("code") == 0 and data.get("data"):
            results.append(data["data"])
        elif "error" in data:
            results.append({"asset_id": asset_id, "error": data["error"]})
    
    output = {"user_id": user_id, "balances": results}
    
    if raw:
        print(json.dumps(output))
    else:
        print(f"Balances for user {user_id}:")
        for b in results:
            if "error" in b:
                print(f"  Asset {b.get('asset_id', '?')}: ERROR - {b['error']}")
            else:
                asset = b.get("asset", f"Asset_{b.get('asset_id', '?')}")
                balance = b.get("balance", "?")
                print(f"  {asset}: {balance}")
    
    return output


def main():
    parser = argparse.ArgumentParser(description='Query balances via API')
    parser.add_argument('--user', '-u', type=int, required=True, help='User ID')
    parser.add_argument('--raw', '-r', action='store_true', help='Output raw JSON')
    args = parser.parse_args()
    
    result = query_balances(args.user, None, args.raw)
    return 0 if result else 1


if __name__ == "__main__":
    sys.exit(main())
