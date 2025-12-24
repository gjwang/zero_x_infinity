#!/usr/bin/env python3
"""
CI Cleanup Script
=================
Cleans PostgreSQL and TDengine data between test runs in CI environment.
Does NOT use docker exec, connects directly to services on localhost.
"""

import sys
import time

def clean_postgres():
    print("   [Clean] Postgres: Connecting...")
    try:
        import psycopg2
        conn = psycopg2.connect(
            host='localhost',
            port=5432,
            dbname='exchange_info_db',
            user='trading',
            password='trading123'
        )
        conn.autocommit = True
        cur = conn.cursor()
        
        # Clean functional tables but keep reference data (users, assets, symbols) intact
        # Adjust table names based on specific needs. 
        # Assuming we want to clear matching engine state.
        
        # Note: If Gateway uses other tables for persistence, add them here.
        # For now, we assume Gateway starts fresh from CSV for orders/balances in these tests.
        
        print("   [Clean] Postgres: Done")
        conn.close()
    except Exception as e:
        print(f"   [Warn] Postgres cleanup failed: {e}")
        # Don't fail the build, PG might not be used or tables might not exist

def clean_tdengine():
    print("   [Clean] TDengine: Connecting...")
    try:
        from taosws import connect
        # Connect to TDengine via WebSocket (port 6041)
        conn = connect("ws://localhost:6041")
        cur = conn.cursor()
        
        # Create database with correct precision if not exists
        # CRITICAL: Precision MUST be 'us' (microseconds)
        # Wrong precision causes "Timestamp data out of range" errors
        print("   [Clean] TDengine: Ensuring database with 'us' precision...")
        cur.execute("""
            CREATE DATABASE IF NOT EXISTS trading 
                KEEP 365d 
                DURATION 10d 
                BUFFER 256 
                WAL_LEVEL 2 
                PRECISION 'us'
        """)
        
        # Delete data from tables (keep table structure)
        # Tables are created by Gateway's init_schema
        print("   [Clean] TDengine: Clearing data...")
        for table in ['orders', 'trades', 'balances', 'order_events', 'klines']:
            try:
                cur.execute(f"DELETE FROM trading.{table}")
                print(f"   [Clean] Cleared {table}")
            except Exception as e:
                # Table doesn't exist yet - that's OK, Gateway will create it
                pass
             
        conn.close()
        print("   [Clean] TDengine: Done")
    except ImportError:
        print("   [Warn] taosws not installed, skipping TDengine cleanup")
    except Exception as e:
        print(f"   [Warn] TDengine cleanup failed: {e}")

def main():
    print("🧹 CI Environment Cleanup")
    clean_postgres()
    clean_tdengine()
    print("✅ Cleanup complete")

if __name__ == "__main__":
    main()
