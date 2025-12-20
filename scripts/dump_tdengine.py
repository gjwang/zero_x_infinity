#!/usr/bin/env python3
"""
dump_tdengine.py - Dump TDengine Settlement Data to CSV
=========================================================

PURPOSE:
    Export orders, trades, and balances from TDengine to CSV files
    for comparison with Pipeline baseline output.

USAGE:
    # Dump all tables
    python3 scripts/dump_tdengine.py --output /tmp/tdengine_dump

    # Dump specific table
    python3 scripts/dump_tdengine.py --table orders --output /tmp/tdengine_dump

OUTPUT FILES:
    - orders.csv:   order_id,user_id,symbol_id,side,order_type,price,qty,filled_qty,status
    - trades.csv:   trade_id,order_id,user_id,side,price,qty,role
    - balances.csv: user_id,asset_id,avail,frozen,lock_version,settle_version

REQUIREMENTS:
    pip install requests
"""

import argparse
import csv
import json
import os
import subprocess
import sys
from typing import Dict, List, Any, Optional


class TDengineClient:
    """Simple TDengine client using taos CLI via docker exec."""
    
    def __init__(self, container: str = "tdengine", database: str = "trading"):
        self.container = container
        self.database = database
    
    def query(self, sql: str) -> List[Dict[str, Any]]:
        """Execute SQL and return results as list of dicts."""
        # Format SQL for CLI
        cmd = [
            "docker", "exec", self.container, "taos", "-s", sql
        ]
        
        try:
            result = subprocess.run(cmd, capture_output=True, text=True, timeout=60)
            if result.returncode != 0:
                raise Exception(f"Query failed: {result.stderr}")
            
            # Parse output - taos outputs in a specific format
            return self._parse_output(result.stdout)
        except subprocess.TimeoutExpired:
            raise Exception("Query timed out")
    
    def _parse_output(self, output: str) -> List[Dict[str, Any]]:
        """Parse taos CLI output into list of dicts."""
        lines = output.strip().split('\n')
        results = []
        
        # Find header line (contains column names)
        header_idx = -1
        for i, line in enumerate(lines):
            if '|' in line and 'taos>' not in line:
                header_idx = i
                break
        
        if header_idx < 0:
            return []
        
        # Parse header
        header_line = lines[header_idx]
        columns = [col.strip() for col in header_line.split('|') if col.strip()]
        
        # Skip separator line (===)
        data_start = header_idx + 2
        
        # Parse data rows
        for line in lines[data_start:]:
            if '|' not in line or 'Query OK' in line or 'row(s)' in line:
                continue
            values = [val.strip() for val in line.split('|') if val.strip() or val == '']
            if len(values) >= len(columns):
                row = {}
                for i, col in enumerate(columns):
                    row[col] = values[i].strip() if i < len(values) else ''
                results.append(row)
        
        return results


def dump_orders(client: TDengineClient, output_dir: str) -> int:
    """Dump orders table to CSV."""
    print("[Orders] Querying TDengine...")
    
    sql = """
    SELECT order_id, user_id, symbol_id, side, order_type, price, qty, filled_qty, status, remaining_qty
    FROM trading.orders
    ORDER BY order_id
    """
    
    try:
        rows = client.query(sql)
        print(f"[Orders] Retrieved {len(rows)} rows")
        
        if not rows:
            print("[Orders] ⚠️  No data found")
            return 0
        
        output_path = os.path.join(output_dir, "orders.csv")
        with open(output_path, 'w', newline='') as f:
            fieldnames = ['order_id', 'user_id', 'symbol_id', 'side', 'order_type', 
                         'price', 'qty', 'filled_qty', 'status', 'remaining_qty']
            writer = csv.DictWriter(f, fieldnames=fieldnames)
            writer.writeheader()
            for row in rows:
                writer.writerow({k: row.get(k, '') for k in fieldnames})
        
        print(f"[Orders] ✅ Saved to {output_path}")
        return len(rows)
    
    except Exception as e:
        print(f"[Orders] ❌ Error: {e}")
        return -1


def dump_trades(client: TDengineClient, output_dir: str) -> int:
    """Dump trades table to CSV."""
    print("[Trades] Querying TDengine...")
    
    sql = """
    SELECT trade_id, order_id, user_id, symbol_id, side, price, qty, role
    FROM trading.trades
    ORDER BY trade_id, order_id
    """
    
    try:
        rows = client.query(sql)
        print(f"[Trades] Retrieved {len(rows)} rows")
        
        if not rows:
            print("[Trades] ⚠️  No data found")
            return 0
        
        output_path = os.path.join(output_dir, "trades.csv")
        with open(output_path, 'w', newline='') as f:
            fieldnames = ['trade_id', 'order_id', 'user_id', 'symbol_id', 'side', 'price', 'qty', 'role']
            writer = csv.DictWriter(f, fieldnames=fieldnames)
            writer.writeheader()
            for row in rows:
                writer.writerow({k: row.get(k, '') for k in fieldnames})
        
        print(f"[Trades] ✅ Saved to {output_path}")
        return len(rows)
    
    except Exception as e:
        print(f"[Trades] ❌ Error: {e}")
        return -1


def dump_balances(client: TDengineClient, output_dir: str) -> int:
    """Dump balances table to CSV (latest values per user/asset)."""
    print("[Balances] Querying TDengine...")
    
    # Get latest balance per user_id/asset_id (max timestamp)
    sql = """
    SELECT user_id, asset_id, LAST(avail) as avail, LAST(frozen) as frozen, 
           LAST(lock_version) as lock_version, LAST(settle_version) as settle_version
    FROM trading.balances
    GROUP BY user_id, asset_id
    ORDER BY user_id, asset_id
    """
    
    try:
        rows = client.query(sql)
        print(f"[Balances] Retrieved {len(rows)} rows")
        
        if not rows:
            print("[Balances] ⚠️  No data found")
            return 0
        
        output_path = os.path.join(output_dir, "balances.csv")
        with open(output_path, 'w', newline='') as f:
            fieldnames = ['user_id', 'asset_id', 'avail', 'frozen', 'lock_version', 'settle_version']
            writer = csv.DictWriter(f, fieldnames=fieldnames)
            writer.writeheader()
            for row in rows:
                writer.writerow({k: row.get(k, '') for k in fieldnames})
        
        print(f"[Balances] ✅ Saved to {output_path}")
        return len(rows)
    
    except Exception as e:
        print(f"[Balances] ❌ Error: {e}")
        return -1


def main():
    parser = argparse.ArgumentParser(description='Dump TDengine Settlement Data to CSV')
    parser.add_argument('--output', '-o', default='/tmp/tdengine_dump', help='Output directory')
    parser.add_argument('--table', '-t', choices=['orders', 'trades', 'balances', 'all'], 
                       default='all', help='Table to dump')
    parser.add_argument('--container', default='tdengine', help='Docker container name')
    args = parser.parse_args()
    
    print("╔════════════════════════════════════════════════════════════╗")
    print("║    TDengine Data Dump                                     ║")
    print("╚════════════════════════════════════════════════════════════╝")
    print()
    
    # Create output directory
    os.makedirs(args.output, exist_ok=True)
    print(f"Output directory: {args.output}")
    print()
    
    # Initialize client
    client = TDengineClient(container=args.container)
    
    results = {}
    
    if args.table in ['orders', 'all']:
        results['orders'] = dump_orders(client, args.output)
    
    if args.table in ['trades', 'all']:
        results['trades'] = dump_trades(client, args.output)
    
    if args.table in ['balances', 'all']:
        results['balances'] = dump_balances(client, args.output)
    
    # Summary
    print()
    print("═" * 60)
    print("Summary")
    print("═" * 60)
    all_ok = True
    for table, count in results.items():
        status = "✅" if count >= 0 else "❌"
        print(f"  {table}: {count} records {status}")
        if count < 0:
            all_ok = False
    
    print()
    if all_ok:
        print("✅ Dump completed successfully")
        return 0
    else:
        print("❌ Some tables failed")
        return 1


if __name__ == "__main__":
    sys.exit(main())
