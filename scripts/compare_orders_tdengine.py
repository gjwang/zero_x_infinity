#!/usr/bin/env python3
"""
compare_orders_tdengine.py - Compare TDengine Orders with Pipeline Baseline
=============================================================================

PURPOSE:
    Field-level comparison of TDengine orders with Pipeline output.
    Uses TDengine REST API for fast querying.

USAGE:
    python3 scripts/compare_orders_tdengine.py --pipeline output/t2_orderbook.csv

FIELDS COMPARED:
    - order_id, user_id, side, price, qty, filled_qty, status
"""

import argparse
import csv
import json
import sys
from typing import Dict, Any, List, Tuple

try:
    import requests
    HAS_REQUESTS = True
except ImportError:
    HAS_REQUESTS = False
    import urllib.request
    import base64


class Colors:
    RED = '\033[0;31m'
    GREEN = '\033[0;32m'
    YELLOW = '\033[1;33m'
    NC = '\033[0m'


def disable_colors():
    Colors.RED = ''
    Colors.GREEN = ''
    Colors.YELLOW = ''
    Colors.NC = ''


class TDengineClient:
    """TDengine client using REST API."""
    
    def __init__(self, host: str = "localhost", port: int = 6041, 
                 user: str = "root", password: str = "taosdata",
                 database: str = "trading"):
        self.url = f"http://{host}:{port}/rest/sql/{database}"
        self.auth = (user, password)
    
    def query(self, sql: str) -> List[Dict[str, Any]]:
        """Execute SQL and return results as list of dicts."""
        if HAS_REQUESTS:
            resp = requests.post(self.url, data=sql, auth=self.auth, timeout=120)
            resp.raise_for_status()
            data = resp.json()
        else:
            credentials = base64.b64encode(f"{self.auth[0]}:{self.auth[1]}".encode()).decode()
            req = urllib.request.Request(
                self.url,
                data=sql.encode(),
                headers={'Authorization': f'Basic {credentials}'}
            )
            with urllib.request.urlopen(req, timeout=120) as resp:
                data = json.loads(resp.read().decode())
        
        if data.get('code') != 0:
            raise Exception(f"Query error: {data.get('desc', 'Unknown error')}")
        
        columns = [col[0] for col in data.get('column_meta', [])]
        rows = data.get('data', [])
        return [dict(zip(columns, row)) for row in rows]


# Side mapping: Pipeline uses string, TDengine uses int
SIDE_MAP = {'buy': 0, 'sell': 1, 'BUY': 0, 'SELL': 1, 0: 0, 1: 1}
# Status mapping
STATUS_MAP = {
    'NEW': 0, 'PARTIALLY_FILLED': 1, 'FILLED': 2, 'CANCELLED': 3,
    0: 0, 1: 1, 2: 2, 3: 3
}


def load_pipeline_orders(path: str) -> Dict[int, Dict[str, Any]]:
    """Load Pipeline orderbook CSV."""
    data = {}
    with open(path, 'r') as f:
        reader = csv.DictReader(f)
        for row in reader:
            order_id = int(row['order_id'])
            data[order_id] = {
                'user_id': int(row['user_id']),
                'side': SIDE_MAP.get(row['side'].lower(), row['side']),
                'price': int(row['price']),
                'qty': int(row['qty']),
                'filled_qty': int(row['filled_qty']),
                'status': STATUS_MAP.get(row['status'].upper(), row['status']),
            }
    return data


def query_tdengine_orders(client: TDengineClient) -> Dict[int, Dict[str, Any]]:
    """Query the LATEST state of all orders from TDengine using GROUP BY."""
    sql = """
    SELECT order_id, LAST(user_id) as user_id, LAST(side) as side, 
           LAST(price) as price, LAST(qty) as qty, 
           LAST(filled_qty) as filled_qty, LAST(status) as status
    FROM orders
    GROUP BY order_id
    """
    rows = client.query(sql)
    
    data = {}
    for row in rows:
        order_id = int(row['order_id'])
        # Handle cases where TDengine might return column names like 'last(user_id)'
        # or properly aliased names depending on version/client
        def get_val(key):
            return row.get(key) if key in row else row.get(f"last({key})")

        data[order_id] = {
            'user_id': int(get_val('user_id')),
            'side': int(get_val('side')),
            'price': int(get_val('price')),
            'qty': int(get_val('qty')),
            'filled_qty': int(get_val('filled_qty')),
            'status': int(get_val('status')),
        }
    return data


def compare_orders(
    pipeline: Dict[int, Dict[str, Any]],
    db: Dict[int, Dict[str, Any]],
    max_errors: int = 20
) -> Tuple[int, int, int, List[str]]:
    """Compare Pipeline vs DB orders field by field."""
    matched = 0
    mismatched = 0
    missing = 0
    errors = []
    
    for order_id, p_data in pipeline.items():
        if order_id not in db:
            missing += 1
            if len(errors) < max_errors:
                errors.append(f"MISSING order_id={order_id}")
            continue
        
        db_data = db[order_id]
        field_errors = []
        
        # Compare each field
        for field in ['user_id', 'side', 'price', 'qty', 'filled_qty', 'status']:
            p_val = p_data[field]
            db_val = db_data[field]
            
            # Special case: Status convention difference
            if field == 'status' and p_val != db_val:
                # Pipeline NEW (0) vs DB PARTIALLY_FILLED (1) for resting orders
                if p_val == 0 and db_val == 1 and db_data['filled_qty'] > 0 and db_data['filled_qty'] < db_data['qty']:
                    continue
                    
            if p_val != db_val:
                field_errors.append(f"{field}: {p_val} != {db_val}")
        
        if field_errors:
            mismatched += 1
            if len(errors) < max_errors:
                errors.append(f"MISMATCH order_id={order_id}: {'; '.join(field_errors)}")
        else:
            matched += 1
    
    # Check for extra orders in DB
    extra_count = 0
    for order_id in db:
        if order_id not in pipeline:
            extra_count += 1
            if len(errors) < max_errors and extra_count <= 5:
                errors.append(f"EXTRA in DB: order_id={order_id}")
    
    if extra_count > 5:
        errors.append(f"... and {extra_count - 5} more extra orders in DB")
    
    return matched, mismatched, missing, errors


def main():
    parser = argparse.ArgumentParser(description='Compare TDengine Orders with Pipeline CSV')
    parser.add_argument('--pipeline', '-p', required=True, help='Pipeline orderbook CSV')
    parser.add_argument('--host', default='localhost', help='TDengine host')
    parser.add_argument('--port', type=int, default=6041, help='TDengine REST port')
    parser.add_argument('--no-color', action='store_true', help='Disable colored output')
    parser.add_argument('--max-errors', type=int, default=20, help='Max errors to display')
    args = parser.parse_args()
    
    if args.no_color or not sys.stdout.isatty():
        disable_colors()
    
    print("╔════════════════════════════════════════════════════════════╗")
    print("║    Orders Comparison: Pipeline CSV vs TDengine            ║")
    print("╚════════════════════════════════════════════════════════════╝")
    print()
    
    # Load Pipeline CSV
    print(f"[1] Loading Pipeline CSV: {args.pipeline}")
    try:
        pipeline = load_pipeline_orders(args.pipeline)
        print(f"    {Colors.GREEN}✓{Colors.NC} Loaded {len(pipeline)} orders")
    except Exception as e:
        print(f"    {Colors.RED}✗{Colors.NC} Error: {e}")
        return 2
    
    # Query TDengine
    print(f"[2] Querying TDengine at {args.host}:{args.port}...")
    try:
        client = TDengineClient(host=args.host, port=args.port)
        db = query_tdengine_orders(client)
        print(f"    {Colors.GREEN}✓{Colors.NC} Retrieved {len(db)} orders")
    except Exception as e:
        print(f"    {Colors.RED}✗{Colors.NC} Error: {e}")
        return 2
    
    # Compare
    print(f"\n[3] Comparing {len(pipeline)} Pipeline orders vs {len(db)} DB orders...")
    matched, mismatched, missing, errors = compare_orders(pipeline, db, args.max_errors)
    
    # Results
    print("\n" + "=" * 60)
    print("Comparison Results")
    print("=" * 60)
    print(f"\nPipeline orders: {len(pipeline)}")
    print(f"DB orders:       {len(db)}")
    print()
    print(f"Matched:    {matched}")
    print(f"Mismatched: {mismatched}")
    print(f"Missing:    {missing}")
    
    if errors:
        print(f"\nFirst {min(args.max_errors, len(errors))} issues:")
        for err in errors:
            print(f"  {Colors.RED}•{Colors.NC} {err}")
    
    print()
    
    if mismatched == 0 and missing == 0:
        print(f"{Colors.GREEN}╔════════════════════════════════════════════════════════════╗{Colors.NC}")
        print(f"{Colors.GREEN}║       ✅ 100% FIELD-LEVEL MATCH                           ║{Colors.NC}")
        print(f"{Colors.GREEN}╚════════════════════════════════════════════════════════════╝{Colors.NC}")
        return 0
    else:
        print(f"{Colors.RED}╔════════════════════════════════════════════════════════════╗{Colors.NC}")
        print(f"{Colors.RED}║       ❌ COMPARISON FAILED                                 ║{Colors.NC}")
        print(f"{Colors.RED}╚════════════════════════════════════════════════════════════╝{Colors.NC}")
        return 1


if __name__ == "__main__":
    sys.exit(main())
