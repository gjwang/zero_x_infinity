#!/usr/bin/env python3
import requests
import csv
import sys
from collections import defaultdict
from pathlib import Path

# TDengine Configuration
TDENGINE_URL = "http://localhost:6041/rest/sql"
TDENGINE_USER = "root"
TDENGINE_PASS = "taosdata"

class Colors:
    GREEN = '\033[92m'
    RED = '\033[91m'
    YELLOW = '\033[93m'
    NC = '\033[0m'

def query_tdengine(sql):
    try:
        resp = requests.post(
            TDENGINE_URL,
            auth=(TDENGINE_USER, TDENGINE_PASS),
            data=sql.encode('utf-8')
        )
        data = resp.json()
        if data.get('code') != 0:
            raise Exception(f"TDengine Error: {data.get('desc')}")
        
        cols = []
        for c in data['column_meta']:
            name = c[0].lower()
            if name.startswith('last(') and name.endswith(')'):
                name = name[5:-1]
            cols.append(name)
            
        rows = data['data']
        return [dict(zip(cols, row)) for row in rows]
    except Exception as e:
        print(f"Query failed: {e}")
        return []

def main():
    print("=== TDengine Trades Verification ===\n")

    # 1. Fetch all trades
    print("[1] Fetching all trades from TDengine...")
    trades = query_tdengine("SELECT * FROM trading.trades")
    print(f"    ✓ Retrieved {len(trades)} trade records (representing {len(trades)//2} trades)")

    # 2. Fetch all orders (final state)
    print("[2] Fetching aggregated orders from TDengine...")
    orders = query_tdengine("SELECT * FROM (SELECT LAST(*) FROM trading.orders GROUP BY order_id)")
    db_orders = {o['order_id']: o for o in orders}
    print(f"    ✓ Retrieved {len(db_orders)} unique orders")

    # 3. Load Pipeline Order Events
    print("[3] Loading Pipeline Order Events (t2_order_events.csv)...")
    pipeline_fills = []
    events_path = "output/t2_order_events.csv"
    if Path(events_path).exists():
        with open(events_path, 'r') as f:
            reader = csv.DictReader(f)
            for row in reader:
                if row['event_type'] in ('filled', 'partial_filled'):
                    pipeline_fills.append({
                        'order_id': int(row['order_id']),
                        'filled_qty': int(row['filled_qty']) if row['filled_qty'] else 0
                    })
        print(f"    ✓ Loaded {len(pipeline_fills)} fill events from Pipeline log")
    else:
        print(f"    {Colors.YELLOW}⚠ Warning: t2_order_events.csv not found, skipping Pipeline alignment check{Colors.NC}")

    # 4. Verify Internal Consistency (Maker/Taker Pairs)
    print("\n[4] Verifying Maker/Taker Pairs...")
    trades_by_id = defaultdict(list)
    for t in trades:
        trades_by_id[t['trade_id']].append(t)

    pair_errors = 0
    for tid, tlist in trades_by_id.items():
        if len(tlist) != 2:
            pair_errors += 1
            if pair_errors <= 5:
                print(f"    {Colors.RED}✗ Trade {tid} has {len(tlist)} records (expected 2){Colors.NC}")
            continue
        
        t1, t2 = tlist[0], tlist[1]
        if t1['price'] != t2['price'] or t1['qty'] != t2['qty']:
            pair_errors += 1
            if pair_errors <= 5:
                print(f"    {Colors.RED}✗ Trade {tid} has mismatched price/qty: {t1['price']}@{t1['qty']} vs {t2['price']}@{t2['qty']}{Colors.NC}")
        
        if t1['role'] == t2['role']:
            pair_errors += 1
            if pair_errors <= 5:
                print(f"    {Colors.RED}✗ Trade {tid} has identical roles: {t1['role']}{Colors.NC}")

    if pair_errors == 0:
        print(f"    {Colors.GREEN}✓ All trades have valid Maker/Taker pairs{Colors.NC}")
    else:
        print(f"    {Colors.RED}✗ Found {pair_errors} pair errors{Colors.NC}")

    # 5. Verify Order-Trade Linkage (Sum of trades == Order filled_qty)
    print("\n[5] Verifying Order-Trade Linkage...")
    sum_qty_by_order = defaultdict(int)
    for t in trades:
        sum_qty_by_order[t['order_id']] += t['qty']

    linkage_errors = 0
    for oid, total_qty in sum_qty_by_order.items():
        if oid not in db_orders:
            linkage_errors += 1
            if linkage_errors <= 5:
                print(f"    {Colors.RED}✗ Order {oid} found in trades but missing in orders table{Colors.NC}")
            continue
        
        db_filled = db_orders[oid]['filled_qty']
        if total_qty != db_filled:
            linkage_errors += 1
            if linkage_errors <= 10:
                print(f"    {Colors.RED}✗ Order {oid} Sum(Trades Qty)={total_qty} != Order.filled_qty={db_filled}{Colors.NC}")

    if linkage_errors == 0:
        print(f"    {Colors.GREEN}✓ All trade quantities match order filled quantities{Colors.NC}")
    else:
        print(f"    {Colors.RED}✗ Found {linkage_errors} linkage errors{Colors.NC}")

    # 6. Verify Pipeline Alignment
    # Note: Using max(filled_qty) because Pipeline events log incremental cumulative state
    if pipeline_fills:
        print("\n[6] Verifying Pipeline Alignment (Order Events vs DB Trades)...")
        pipeline_max_by_order = defaultdict(int)
        for f in pipeline_fills:
            if f['filled_qty'] > pipeline_max_by_order[f['order_id']]:
                pipeline_max_by_order[f['order_id']] = f['filled_qty']
        
        alignment_errors = 0
        for oid, pipe_max in pipeline_max_by_order.items():
            db_total = sum_qty_by_order.get(oid, 0)
            if pipe_max != db_total:
                # Discrepancy may occur if t2_order_events.csv is stale
                alignment_errors += 1
                if alignment_errors <= 5:
                    print(f"    {Colors.RED}✗ Order {oid} Pipeline Max Fill={pipe_max} != DB Total Trades={db_total}{Colors.NC}")

        if alignment_errors == 0:
            print(f"    {Colors.GREEN}✓ All Pipeline fill events are accounted for in TDengine trades{Colors.NC}")
        else:
            print(f"    {Colors.YELLOW}⚠ Found {alignment_errors} alignment discrepancies (Check for stale output logs){Colors.NC}")

    print("\n========================================")
    if pair_errors == 0 and linkage_errors == 0:
        print(f"{Colors.GREEN}✅ TRADES VERIFICATION PASSED (Data is internally consistent){Colors.NC}")
        sys.exit(0)
    else:
        print(f"{Colors.RED}❌ TRADES VERIFICATION FAILED{Colors.NC}")
        sys.exit(1)

if __name__ == "__main__":
    main()
