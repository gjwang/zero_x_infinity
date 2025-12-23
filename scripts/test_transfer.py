#!/usr/bin/env python3
import sys
import os
import time
import requests
import json

# Add scripts directory to path to import lib
sys.path.append(os.path.join(os.path.dirname(__file__), 'lib'))
from api_auth import ApiClient, USER_KEYS

BASE_URL = "http://127.0.0.1:8080"
USER_ID = 1001

def run_test():
    print(f"🔄 Starting Transfer Integration Test for User {USER_ID}")
    
    api_key, priv_key = USER_KEYS.get(USER_ID) # Unpack tuple
    client = ApiClient(api_key=api_key, private_key_hex=priv_key, base_url=BASE_URL)
    
    # 1. Check Initial Balance (Spot)
    print("\n🔍 Checking Initial Spot Balance...")
    # ApiClient automatically signs requests
    resp = client.get("/api/v1/private/balances", params={"user_id": USER_ID, "asset_id": 2})
    
    if resp.status_code == 200:
        bal = resp.json()["data"]
        print(f"   Initial Balance: {bal['available']} (Account Type: {bal.get('account_type', 'unknown')})")
    else:
        print(f"⚠️ Failed to get balance: {resp.text}")

    # 2. Execute Transfer (Spot -> Funding)
    print("\n💸 Executing Transfer: 100 USDT from Spot to Funding...")
    payload = {
        "from": "spot",
        "to": "funding",
        "asset": "USDT",
        "amount": "100"
    }
    
    resp = client.post("/api/v1/private/transfer", json_body=payload)
    
    if resp.status_code != 200:
        print(f"❌ Transfer Failed: {resp.status_code} {resp.text}")
        sys.exit(1)
        
    print(f"✅ Transfer Success: {resp.json()}")
    
    # 3. Verify Postgres Balances directly (since API hits separate TDengine)
    print("\n🔍 Verifying Postgres Balances...")
    import subprocess
    
    def get_pg_balance(u_id, a_id, acc_type):
        cmd = [
            "psql", "-d", "exchange_info_db", "-t", "-c",
            f"SELECT available FROM balances_tb WHERE user_id={u_id} AND asset_id={a_id} AND account_type={acc_type}"
        ]
        try:
            res = subprocess.check_output(cmd).decode().strip()
            return int(res) if res else 0
        except Exception as e:
            print(f"   ⚠️ DB Query failed: {e}")
            return None

    # USDT (2) Spot (1)
    spot_bal = get_pg_balance(USER_ID, 2, 1)
    # USDT (2) Funding (2)
    fund_bal = get_pg_balance(USER_ID, 2, 2)
    
    print(f"   Spot Balance: {spot_bal}")
    print(f"   Funding Balance: {fund_bal}")

    # Initial Spot was 1,000,000. Transfer 100.
    # Spot should be 999,900. Funding 100.
    # Scale: USDT decimals 6. 100 -> 100000000? 
    # Wait, input "100". Logic: 100 * 10^decimals.
    # USDT decimals 6. 100 * 10^6 = 100,000,000.
    # Initial: 1000000.00000000 implied 1,000,000 * 10^0?
    # Seed data: `1000000.00000000`. Numeric literal.
    # Stored in `balances_tb` as DECIMAL/NUMERIC?
    # `migrations/003...` says `ALTER TABLE`.
    # `001_init_schema.sql` defined types.
    # Project uses `i64` for internal representation usually.
    # Let's check `balances_tb` definition. `available` is DECIMAL(30, 8)?
    
    # If DB column is DECIMAL, psql returns e.g. "999900.00000000".
    # And implementation `service.rs`: `rows.map(|r| r.available)` -> `available` is `BigDecimal`?
    # No, `available` in `service.rs`:
    # `sqlx::query!` infers type.
    # If column is DECIMAL, sqlx maps to `BigDecimal`.
    # But my code likely treats it as `i64`?
    # `Amount` in transfer is `i64`.
    
    # CRITICAL CHECK: `balances_tb` schema.
    # If `available` is `DECIMAL`, I need to cast or handle BigDecimal in Rust.
    # My `service.rs` uses `available < amount_scaled`.
    # `amount_scaled` is `i64`.
    # If `available` is `BigDecimal`, comparison fails or type error.
    
    # I MUST check `001_init_schema.sql` for `balances_tb`.
    pass

if __name__ == "__main__":
    try:
        run_test()
    except Exception as e:
        print(f"❌ Error: {e}")
        sys.exit(1)
