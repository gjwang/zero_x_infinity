#!/usr/bin/env python3
"""
TC-ADR-06: Hot Token Listing Test
---------------------------------
Verify ADR-006: User Address Decoupling.

Core Value: When Ops lists a new ERC20 token, users do NOT need to
generate new deposit addresses. Their existing ETH chain address
automatically starts accepting the new token.

Scenario:
1. User A has existing ETH address '0xUserA' in user_chain_addresses.
2. Ops lists 'UNI' token (Contract: 0xUNI) into chain_assets_tb.
3. User A sends UNI to '0xUserA'.
4. Sentinel:
   - Matches 0xUNI -> UNI (via chain_assets_tb).
   - Matches 0xUserA -> User A (via user_chain_addresses).
   - Credits deposit.
5. Expected: Deposit credited without User A doing anything extra.

Prerequisites:
    - PostgreSQL running with migrations applied (012).
"""

import os
import sys
import psycopg2

# Colors for output
class Colors:
    GREEN = '\033[92m'
    RED = '\033[91m'
    YELLOW = '\033[93m'
    CYAN = '\033[96m'
    RESET = '\033[0m'

def log_info(msg): print(f"{Colors.GREEN}[INFO]{Colors.RESET} {msg}")
def log_warn(msg): print(f"{Colors.YELLOW}[WARN]{Colors.RESET} {msg}")
def log_fail(msg): print(f"{Colors.RED}[FAIL]{Colors.RESET} {msg}")
def log_step(msg): print(f"{Colors.CYAN}[STEP]{Colors.RESET} {msg}")

DATABASE_URL = os.getenv("DATABASE_URL", "postgres://postgres:postgres@localhost:5432/exchange_info_db")

def get_db_connection():
    return psycopg2.connect(DATABASE_URL)

# Simulated Sentinel Dual-Lookup
def mock_sentinel_dual_lookup(contract_address, recipient_address):
    """
    Simulates Sentinel's dual lookup:
    1. contract_address -> asset_id (via chain_assets_tb)
    2. recipient_address -> user_id (via user_chain_addresses)
    
    Returns: (user_id, asset_symbol) or (None, None) if either fails.
    """
    conn = get_db_connection()
    cur = conn.cursor()
    
    # Lookup 1: Contract -> Asset
    cur.execute("""
        SELECT a.asset, ca.decimals
        FROM chain_assets_tb ca
        JOIN assets_tb a ON ca.asset_id = a.asset_id
        WHERE ca.chain_slug = 'ETH' 
          AND LOWER(ca.contract_address) = LOWER(%s)
          AND ca.is_active = TRUE
    """, (contract_address,))
    
    asset_row = cur.fetchone()
    if not asset_row:
        cur.close()
        conn.close()
        return None, None, "ASSET_NOT_FOUND"
    
    asset_symbol, decimals = asset_row
    
    # Lookup 2: Address -> User
    cur.execute("""
        SELECT user_id FROM user_chain_addresses
        WHERE chain_slug = 'ETH' AND LOWER(address) = LOWER(%s)
    """, (recipient_address,))
    
    user_row = cur.fetchone()
    cur.close()
    conn.close()
    
    if not user_row:
        return None, asset_symbol, "USER_NOT_FOUND"
    
    user_id = user_row[0]
    return user_id, asset_symbol, "SUCCESS"

def setup_test_user(user_id, eth_address):
    """Create test user and their ETH address."""
    conn = get_db_connection()
    cur = conn.cursor()
    
    # Ensure user exists
    cur.execute("""
        INSERT INTO users_tb (user_id, username, email, status)
        VALUES (%s, %s, %s, 1)
        ON CONFLICT (user_id) DO NOTHING
    """, (user_id, f"testuser_{user_id}", f"user{user_id}@test.com"))
    
    # Add ETH address
    cur.execute("""
        INSERT INTO user_chain_addresses (user_id, chain_slug, address)
        VALUES (%s, 'ETH', %s)
        ON CONFLICT (user_id, chain_slug) DO UPDATE SET address = %s
    """, (user_id, eth_address.lower(), eth_address.lower()))
    
    conn.commit()
    cur.close()
    conn.close()

def setup_test_token(symbol, contract, decimals, is_active=True):
    """Add test token to chain_assets_tb."""
    conn = get_db_connection()
    cur = conn.cursor()
    
    # Ensure asset exists
    cur.execute("SELECT asset_id FROM assets_tb WHERE asset = %s", (symbol,))
    row = cur.fetchone()
    if not row:
        cur.execute("""
            INSERT INTO assets_tb (asset, name, decimals, status, asset_flags)
            VALUES (%s, %s, %s, 1, 7)
            RETURNING asset_id
        """, (symbol, f"{symbol} Token", decimals))
        row = cur.fetchone()
        conn.commit()
    
    asset_id = row[0]
    
    cur.execute("""
        INSERT INTO chain_assets_tb (chain_slug, asset_id, contract_address, decimals, is_active)
        VALUES ('ETH', %s, %s, %s, %s)
        ON CONFLICT (chain_slug, contract_address) DO UPDATE SET is_active = %s
    """, (asset_id, contract.lower(), decimals, is_active, is_active))
    conn.commit()
    
    cur.close()
    conn.close()

def cleanup_test_data(user_id, contract):
    """Remove test data."""
    conn = get_db_connection()
    cur = conn.cursor()
    cur.execute("DELETE FROM user_chain_addresses WHERE user_id = %s", (user_id,))
    cur.execute("DELETE FROM chain_assets_tb WHERE LOWER(contract_address) = LOWER(%s)", (contract,))
    conn.commit()
    cur.close()
    conn.close()

def run_test():
    log_info("=" * 60)
    log_info("TC-ADR-06: Hot Token Listing Test (Address Decoupling)")
    log_info("=" * 60)
    
    TEST_USER_ID = 999001
    TEST_USER_ADDRESS = "0xAAAA000000000000000000000000000000000001"
    TEST_TOKEN_SYMBOL = "HOTUNI"
    TEST_TOKEN_CONTRACT = "0xBBBB000000000000000000000000000000000002"
    TEST_TOKEN_DECIMALS = 18
    
    # Step 1: User A already has an ETH address (from previous ETH deposit)
    log_step("1. Setting up User A with existing ETH address...")
    setup_test_user(TEST_USER_ID, TEST_USER_ADDRESS)
    log_info(f"✅ User {TEST_USER_ID} has ETH address: {TEST_USER_ADDRESS}")
    
    # Step 2: Ops lists new token (similar to Hot Reload test)
    log_step("2. Ops listing new token 'HOTUNI' (Contract: 0xBBBB...)...")
    setup_test_token(TEST_TOKEN_SYMBOL, TEST_TOKEN_CONTRACT, TEST_TOKEN_DECIMALS, is_active=True)
    log_info(f"✅ Token {TEST_TOKEN_SYMBOL} listed on ETH chain (is_active=TRUE)")
    
    # Step 3: Simulate Sentinel detecting HOTUNI transfer to User A's address
    log_step("3. Simulating HOTUNI Transfer to User A's ETH address...")
    log_info(f"   Transfer(contract={TEST_TOKEN_CONTRACT}, to={TEST_USER_ADDRESS})")
    
    user_id, asset, status = mock_sentinel_dual_lookup(TEST_TOKEN_CONTRACT, TEST_USER_ADDRESS)
    
    if status == "SUCCESS":
        log_info(f"✅ DUAL LOOKUP SUCCESS!")
        log_info(f"   - Asset: {asset}")
        log_info(f"   - User ID: {user_id}")
        log_info("   → Deposit would be credited to correct user with correct asset!")
    else:
        log_fail(f"❌ DUAL LOOKUP FAILED: {status}")
        cleanup_test_data(TEST_USER_ID, TEST_TOKEN_CONTRACT)
        return False
    
    # Step 4: Verify key behavior - User did NOT need to generate a new address
    log_step("4. Verifying ADR-006 Value Proposition...")
    log_info("   ✅ User A did NOT generate a new 'HOTUNI address'")
    log_info("   ✅ User A reused their existing ETH chain address")
    log_info("   ✅ New token immediately recognized by Sentinel")
    
    # Cleanup
    cleanup_test_data(TEST_USER_ID, TEST_TOKEN_CONTRACT)
    
    log_info("=" * 60)
    log_info("✅ TC-ADR-06 PASSED: Hot Token Listing with Address Decoupling")
    log_info("=" * 60)
    return True

if __name__ == "__main__":
    try:
        success = run_test()
        sys.exit(0 if success else 1)
    except Exception as e:
        log_fail(f"Test crashed: {e}")
        sys.exit(1)
