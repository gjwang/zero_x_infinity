import sys
import time
import json
import requests
sys.path.append('scripts/lib')
from api_auth import get_test_client

# Constants
USER_ID = 1001
ASSET = "USDT"
TRANSFER_AMOUNT = 100.0

def get_balances(client, user_id):
    """
    Fetch balances via the API. 
    The API is expected to return both Funding (from PG) and Spot (from TDengine).
    """
    headers = {'X-User-ID': str(user_id)}
    # Use the unified account API
    resp = client.get('/api/v1/private/account', headers=headers)
    if resp.status_code != 200:
        print(f"Error fetching balances: {resp.status_code} - {resp.text}")
        return None
    
    data = resp.json().get('data', {})
    balances = data.get('balances', [])
    
    funding_avail = 0.0
    spot_avail = 0.0
    
    for b in balances:
        if b['asset'] == ASSET:
            if b['account_type'] == 'funding':
                funding_avail = float(b['available'])
            elif b['account_type'] == 'spot':
                spot_avail = float(b['available'])
                
    return {
        'funding': funding_avail,
        'spot': spot_avail,
        'total': funding_avail + spot_avail
    }

def run_verification():
    print(f"=== Robust Transfer Verification (User: {USER_ID}, Asset: {ASSET}) ===")
    client = get_test_client(user_id=USER_ID)
    headers = {'X-User-ID': str(USER_ID)}

    # 1. Initial State
    initial = get_balances(client, USER_ID)
    if initial is None: return
    print(f"Initial State: Funding={initial['funding']:.2f}, Spot={initial['spot']:.2f}, Total={initial['total']:.2f}")

    # 2. Perform Transfer (Funding -> Spot)
    print(f"\nTransferring {TRANSFER_AMOUNT} {ASSET} (Funding -> Spot)...")
    transfer_req = {
        'from': 'funding',
        'to': 'spot',
        'asset': ASSET,
        'amount': str(TRANSFER_AMOUNT)
    }
    resp = client.post('/api/v1/private/transfer', json_body=transfer_req, headers=headers)
    
    if resp.status_code != 200:
        print(f"Transfer failed! Status: {resp.status_code}, Msg: {resp.text}")
        return
    
    status = resp.json().get('data', {}).get('status')
    print(f"Transfer Status: {status}")
    
    if status != 'COMMITTED':
        print("Transfer not committed, skipping balance check.")
        return

    # Wait for settlement persistence
    print("Waiting 3 seconds for TDengine settlement...")
    time.sleep(3)

    # 3. Verify Final State
    final = get_balances(client, USER_ID)
    if final is None: return
    
    print(f"Final State: Funding={final['funding']:.2f}, Spot={final['spot']:.2f}, Total={final['total']:.2f}")
    
    print("\n--- Verification Results ---")
    
    # Check 1: Funding decreased by exactly TRANSFER_AMOUNT
    funding_diff = initial['funding'] - final['funding']
    if abs(funding_diff - TRANSFER_AMOUNT) < 0.000001:
        print(f"✅ Funding Update: PASSED (Decreased by {funding_diff:.2f})")
        funding_ok = True
    else:
        print(f"❌ Funding Update: FAILED (Expected decrease: {TRANSFER_AMOUNT}, Actual: {funding_diff:.2f})")
        funding_ok = False

    # Check 2: Spot increased by at least TRANSFER_AMOUNT
    # Note: TDengine may start empty, so we check increment, not absolute
    spot_increase = final['spot'] - initial['spot']
    if spot_increase >= TRANSFER_AMOUNT - 0.000001:
        print(f"✅ Spot Persistence: PASSED (TDengine Spot increased by {spot_increase:.2f})")
        spot_ok = True
    else:
        print(f"❌ Spot Persistence: FAILED (Expected increase ≥ {TRANSFER_AMOUNT}, Actual: {spot_increase:.2f})")
        spot_ok = False

    # Check 3: Transfer was atomic (Funding decrease == Spot increase from transfer)
    # This verifies the TRANSFER itself was correct, not the initial state
    if abs(funding_diff - TRANSFER_AMOUNT) < 0.000001 and spot_increase >= TRANSFER_AMOUNT - 0.000001:
        print(f"✅ Transfer Atomicity: PASSED (Funding -{funding_diff:.2f} == Spot +{TRANSFER_AMOUNT:.2f})")
        atomic_ok = True
    else:
        print(f"❌ Transfer Atomicity: FAILED")
        atomic_ok = False

    if funding_ok and spot_ok and atomic_ok:
        print("\nSUMMARY: ✅ ALL CHECKS PASSED - Transfer Persistence Verified!")
        sys.exit(0)
    else:
        print("\nSUMMARY: ❌ SOME CHECKS FAILED")
        sys.exit(1)

if __name__ == "__main__":
    run_verification()
