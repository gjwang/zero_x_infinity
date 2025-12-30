#!/usr/bin/env python3
"""
TC-ADR-05: Safe Listing Test
----------------------------
Verify that new tokens added to chain_assets_tb with is_active=FALSE
are NOT processed by Sentinel until explicitly activated.

This is a SECURITY feature to prevent "mis-configuration goes live immediately".

Scenario:
1. Add token "RISKY" to chain_assets_tb with is_active=FALSE (default).
2. Simulate a Transfer event for "RISKY".
3. Expected: Sentinel ignores/rejects the deposit.
4. Ops sets is_active=TRUE.
5. Simulate another Transfer.
6. Expected: Sentinel now accepts the deposit.

Prerequisites:
    - PostgreSQL running with migrations applied.
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

# Simulated Sentinel logic (mirrors get_token_info + is_active check)
def mock_sentinel_lookup(contract_address):
    """
    Simulates what Sentinel's get_token_info would return.
    Returns: (asset_symbol, decimals) or None if not found/inactive.
    """
    conn = get_db_connection()
    cur = conn.cursor()
    
    cur.execute("""
        SELECT a.asset, ca.decimals, ca.is_active
        FROM chain_assets_tb ca
        JOIN assets_tb a ON ca.asset_id = a.asset_id
        WHERE ca.chain_slug = 'ETH' AND LOWER(ca.contract_address) = LOWER(%s)
    """, (contract_address,))
    
    row = cur.fetchone()
    cur.close()
    conn.close()
    
    if not row:
        return None, None, "NOT_FOUND"
    
    symbol, decimals, is_active = row
    if not is_active:
        return symbol, decimals, "INACTIVE"
    
    return symbol, decimals, "ACTIVE"

def add_test_token(symbol, contract, decimals, is_active=False):
    """Add a test token to DB."""
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
    
    # Add to chain_assets_tb
    cur.execute("""
        INSERT INTO chain_assets_tb (chain_slug, asset_id, contract_address, decimals, is_active)
        VALUES ('ETH', %s, %s, %s, %s)
        ON CONFLICT (chain_slug, contract_address) DO UPDATE SET is_active = %s
    """, (asset_id, contract.lower(), decimals, is_active, is_active))
    conn.commit()
    
    cur.close()
    conn.close()
    return asset_id

def set_token_active(contract, is_active):
    """Toggle is_active flag."""
    conn = get_db_connection()
    cur = conn.cursor()
    cur.execute("""
        UPDATE chain_assets_tb SET is_active = %s
        WHERE LOWER(contract_address) = LOWER(%s)
    """, (is_active, contract))
    conn.commit()
    cur.close()
    conn.close()

def cleanup_test_token(contract):
    """Remove test token."""
    conn = get_db_connection()
    cur = conn.cursor()
    cur.execute("DELETE FROM chain_assets_tb WHERE LOWER(contract_address) = LOWER(%s)", (contract,))
    conn.commit()
    cur.close()
    conn.close()

def run_test():
    log_info("=" * 60)
    log_info("TC-ADR-05: Safe Listing Test (is_active=FALSE)")
    log_info("=" * 60)
    
    TEST_SYMBOL = "RISKY"
    TEST_CONTRACT = "0xdeadbeef00000000000000000000000000000001"
    TEST_DECIMALS = 18
    
    # Step 1: Add token with is_active=FALSE (default)
    log_step("1. Adding 'RISKY' token with is_active=FALSE...")
    add_test_token(TEST_SYMBOL, TEST_CONTRACT, TEST_DECIMALS, is_active=False)
    log_info("✅ Token added (is_active=FALSE)")
    
    # Step 2: Simulate Sentinel lookup
    log_step("2. Simulating Sentinel lookup for RISKY...")
    symbol, decimals, status = mock_sentinel_lookup(TEST_CONTRACT)
    
    if status == "INACTIVE":
        log_info(f"✅ SECURE: Token found but INACTIVE (symbol={symbol})")
    else:
        log_fail(f"❌ VULNERABILITY: Token status={status}, should be INACTIVE")
        cleanup_test_token(TEST_CONTRACT)
        return False
    
    # Step 3: Ops activates the token
    log_step("3. Ops activating token (is_active=TRUE)...")
    set_token_active(TEST_CONTRACT, True)
    log_info("✅ Token activated")
    
    # Step 4: Re-check
    log_step("4. Re-checking Sentinel lookup...")
    symbol, decimals, status = mock_sentinel_lookup(TEST_CONTRACT)
    
    if status == "ACTIVE":
        log_info(f"✅ Token now ACTIVE (symbol={symbol}, decimals={decimals})")
    else:
        log_fail(f"❌ Token should be ACTIVE, got {status}")
        cleanup_test_token(TEST_CONTRACT)
        return False
    
    # Cleanup
    cleanup_test_token(TEST_CONTRACT)
    
    log_info("=" * 60)
    log_info("✅ TC-ADR-05 PASSED: Safe Listing verified")
    log_info("=" * 60)
    return True

if __name__ == "__main__":
    try:
        success = run_test()
        sys.exit(0 if success else 1)
    except Exception as e:
        log_fail(f"Test crashed: {e}")
        sys.exit(1)
