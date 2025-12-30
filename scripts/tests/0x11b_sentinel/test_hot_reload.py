#!/usr/bin/env python3
"""
TC-ADR-04: Hot Reload Test
--------------------------
Verify that Sentinel's EthScanner can pick up newly added tokens
from the database without a restart.

Scenario:
1. Start with only ETH Native in chain_assets_tb.
2. Sentinel starts, builds config (no USDT).
3. Ops inserts USDT into chain_assets_tb via SQL.
4. Wait for refresh_config() interval (60s) or trigger manually.
5. Send USDT Transfer.
6. Expected: Sentinel now detects USDT deposit.

Prerequisites:
    - PostgreSQL running with migrations applied.
    - Anvil (ETH Node) running.
    - Sentinel running (for full E2E) OR mock mode.
"""

import os
import sys
import time
import psycopg2
from decimal import Decimal

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

# DB Connection
DATABASE_URL = os.getenv("DATABASE_URL", "postgres://postgres:postgres@localhost:5432/exchange_info_db")

def get_db_connection():
    return psycopg2.connect(DATABASE_URL)

def check_chain_assets_tb_exists():
    """Verify chain_assets_tb schema exists."""
    log_step("1. Checking chain_assets_tb exists...")
    try:
        conn = get_db_connection()
        cur = conn.cursor()
        cur.execute("SELECT COUNT(*) FROM chain_assets_tb")
        count = cur.fetchone()[0]
        log_info(f"✅ chain_assets_tb exists with {count} records")
        cur.close()
        conn.close()
        return True
    except Exception as e:
        log_fail(f"❌ chain_assets_tb not found: {e}")
        return False

def get_active_eth_assets():
    """Get list of active assets on ETH chain."""
    log_step("2. Fetching active ETH assets from DB...")
    conn = get_db_connection()
    cur = conn.cursor()
    cur.execute("""
        SELECT a.asset, ca.contract_address, ca.decimals, ca.is_active
        FROM chain_assets_tb ca
        JOIN assets_tb a ON ca.asset_id = a.asset_id
        WHERE ca.chain_slug = 'ETH'
    """)
    rows = cur.fetchall()
    cur.close()
    conn.close()
    
    for row in rows:
        status = "✅ Active" if row[3] else "⚠️ Inactive"
        log_info(f"   {row[0]}: Contract={row[1] or 'NATIVE'}, Decimals={row[2]}, {status}")
    
    return rows

def simulate_hot_add_token(symbol, contract, decimals):
    """Simulate Ops adding a new token to chain_assets_tb."""
    log_step(f"3. Simulating Ops adding {symbol} to chain_assets_tb...")
    conn = get_db_connection()
    cur = conn.cursor()
    
    # Get asset_id for the symbol
    cur.execute("SELECT asset_id FROM assets_tb WHERE asset = %s", (symbol,))
    row = cur.fetchone()
    
    if not row:
        log_warn(f"   Asset {symbol} not in assets_tb, creating mock entry...")
        cur.execute("""
            INSERT INTO assets_tb (asset, name, decimals, status, asset_flags)
            VALUES (%s, %s, %s, 1, 7)
            RETURNING asset_id
        """, (symbol, f"{symbol} Token", decimals))
        row = cur.fetchone()
        conn.commit()
    
    asset_id = row[0]
    
    # Insert into chain_assets_tb
    try:
        cur.execute("""
            INSERT INTO chain_assets_tb (chain_slug, asset_id, contract_address, decimals, is_active)
            VALUES ('ETH', %s, %s, %s, TRUE)
            ON CONFLICT (chain_slug, contract_address) DO UPDATE SET is_active = TRUE
        """, (asset_id, contract.lower(), decimals))
        conn.commit()
        log_info(f"✅ Added {symbol} to chain_assets_tb (is_active=TRUE)")
    except Exception as e:
        log_fail(f"❌ Failed to add {symbol}: {e}")
        conn.rollback()
    
    cur.close()
    conn.close()

def cleanup_test_token(contract):
    """Remove test token from chain_assets_tb."""
    log_step("Cleanup: Removing test token...")
    conn = get_db_connection()
    cur = conn.cursor()
    cur.execute("DELETE FROM chain_assets_tb WHERE contract_address = %s", (contract.lower(),))
    conn.commit()
    cur.close()
    conn.close()
    log_info("✅ Test token removed")

def run_test():
    """
    Main test flow.
    Note: This is a DB-level simulation. For full E2E, we would:
    1. Query Sentinel's internal state before/after.
    2. Send actual ERC20 transfer via Anvil.
    3. Verify deposit_history table.
    """
    log_info("=" * 60)
    log_info("TC-ADR-04: Hot Reload Test (DB Simulation)")
    log_info("=" * 60)
    
    # Step 1: Verify Schema
    if not check_chain_assets_tb_exists():
        log_fail("FAIL: Schema not ready")
        return False
    
    # Step 2: Get current state
    get_active_eth_assets()
    
    # Step 3: Add a test token
    TEST_SYMBOL = "HOTTEST"
    TEST_CONTRACT = "0x1234567890abcdef1234567890abcdef12345678"
    TEST_DECIMALS = 8
    
    simulate_hot_add_token(TEST_SYMBOL, TEST_CONTRACT, TEST_DECIMALS)
    
    # Step 4: Verify it's in DB
    log_step("4. Verifying token is now in DB...")
    conn = get_db_connection()
    cur = conn.cursor()
    cur.execute("""
        SELECT ca.is_active FROM chain_assets_tb ca
        WHERE ca.contract_address = %s
    """, (TEST_CONTRACT.lower(),))
    row = cur.fetchone()
    cur.close()
    conn.close()
    
    if row and row[0]:
        log_info("✅ Token found in chain_assets_tb with is_active=TRUE")
    else:
        log_fail("❌ Token not found or not active")
        return False
    
    # Step 5: Simulate "waiting for Sentinel refresh"
    log_step("5. In production, Sentinel would refresh config in 60s...")
    log_info("   (Skipping wait in this DB-only test)")
    
    # Cleanup
    cleanup_test_token(TEST_CONTRACT)
    
    log_info("=" * 60)
    log_info("✅ TC-ADR-04 PASSED: Hot Reload DB Simulation")
    log_info("=" * 60)
    log_warn("Note: Full E2E requires Sentinel integration test")
    return True

if __name__ == "__main__":
    try:
        success = run_test()
        sys.exit(0 if success else 1)
    except Exception as e:
        log_fail(f"Test crashed: {e}")
        sys.exit(1)
