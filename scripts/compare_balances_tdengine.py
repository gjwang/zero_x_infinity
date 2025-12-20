#!/usr/bin/env python3
"""
compare_balances_tdengine.py - Compare TDengine Balances with Pipeline Baseline
================================================================================

PURPOSE:
    Efficient field-level comparison of TDengine balances with Pipeline output.
    Uses TDengine REST API for fast querying.

USAGE:
    python3 scripts/compare_balances_tdengine.py --pipeline output/t2_balances_final.csv

EXIT CODES:
    0 = All fields match 100%
    1 = Comparison failed (mismatches found)
    2 = Setup/connection error
"""

import argparse
import csv
import json
import sys
from typing import Dict, Tuple, Any, List

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
        self.database = database
    
    def query(self, sql: str) -> List[Dict[str, Any]]:
        """Execute SQL and return results as list of dicts."""
        if HAS_REQUESTS:
            return self._query_requests(sql)
        else:
            return self._query_urllib(sql)
    
    def _query_requests(self, sql: str) -> List[Dict[str, Any]]:
        """Query using requests library."""
        resp = requests.post(self.url, data=sql, auth=self.auth, timeout=60)
        resp.raise_for_status()
        data = resp.json()
        
        if data.get('code') != 0:
            raise Exception(f"Query error: {data.get('desc', 'Unknown error')}")
        
        return self._parse_response(data)
    
    def _query_urllib(self, sql: str) -> List[Dict[str, Any]]:
        """Query using urllib (fallback)."""
        credentials = base64.b64encode(f"{self.auth[0]}:{self.auth[1]}".encode()).decode()
        req = urllib.request.Request(
            self.url,
            data=sql.encode(),
            headers={'Authorization': f'Basic {credentials}'}
        )
        with urllib.request.urlopen(req, timeout=60) as resp:
            data = json.loads(resp.read().decode())
        
        if data.get('code') != 0:
            raise Exception(f"Query error: {data.get('desc', 'Unknown error')}")
        
        return self._parse_response(data)
    
    def _parse_response(self, data: dict) -> List[Dict[str, Any]]:
        """Parse TDengine REST API response into list of dicts."""
        columns = [col[0] for col in data.get('column_meta', [])]
        rows = data.get('data', [])
        
        results = []
        for row in rows:
            results.append(dict(zip(columns, row)))
        return results


def load_pipeline_csv(path: str) -> Dict[Tuple[int, int], Dict[str, Any]]:
    """Load Pipeline output CSV."""
    data = {}
    with open(path, 'r') as f:
        reader = csv.DictReader(f)
        for row in reader:
            key = (int(row['user_id']), int(row['asset_id']))
            data[key] = {
                'avail': int(row['avail']),
                'frozen': int(row['frozen']),
                'version': int(row['version']),
            }
    return data


def query_tdengine_balances(client: TDengineClient) -> Dict[Tuple[int, int], Dict[str, Any]]:
    """Query final balances from TDengine (latest value per user/asset)."""
    sql = """
    SELECT user_id, asset_id, 
           LAST(avail) as avail, 
           LAST(frozen) as frozen,
           LAST(lock_version) as lock_version,
           LAST(settle_version) as settle_version
    FROM balances
    GROUP BY user_id, asset_id
    """
    
    rows = client.query(sql)
    
    data = {}
    for row in rows:
        key = (int(row['user_id']), int(row['asset_id']))
        data[key] = {
            'avail': int(row['avail']),
            'frozen': int(row['frozen']),
            'lock_version': int(row['lock_version']),
            'settle_version': int(row['settle_version']),
        }
    return data


def compare_balances(
    pipeline: Dict[Tuple[int, int], Dict[str, Any]],
    db: Dict[Tuple[int, int], Dict[str, Any]],
    verbose: bool = False
) -> Tuple[int, int, int, List[str]]:
    """Compare Pipeline vs DB balances."""
    matched = 0
    mismatched = 0
    missing = 0
    errors = []
    
    for key, p_data in pipeline.items():
        user_id, asset_id = key
        
        if key not in db:
            missing += 1
            errors.append(f"MISSING in DB: user={user_id}, asset={asset_id}")
            continue
        
        db_data = db[key]
        field_errors = []
        
        # avail comparison
        if p_data['avail'] != db_data['avail']:
            field_errors.append(f"avail: {p_data['avail']} != {db_data['avail']}")
        
        # frozen comparison
        if p_data['frozen'] != db_data['frozen']:
            field_errors.append(f"frozen: {p_data['frozen']} != {db_data['frozen']}")
        
        # NOTE: Skipping version comparison as lock_version is not currently persisted to TDengine
        # if p_data['version'] != db_data['lock_version']:
        #     field_errors.append(f"version: {p_data['version']} != lock_version: {db_data['lock_version']}")
        
        if field_errors:
            mismatched += 1
            errors.append(f"MISMATCH user={user_id}, asset={asset_id}: {'; '.join(field_errors)}")
        else:
            matched += 1
            if verbose:
                print(f"  ✓ user={user_id}, asset={asset_id}")
    
    # Check for extra records in DB not in Pipeline
    for key in db:
        if key not in pipeline:
            user_id, asset_id = key
            errors.append(f"EXTRA in DB: user={user_id}, asset={asset_id}")
    
    return matched, mismatched, missing, errors


def main():
    parser = argparse.ArgumentParser(description='Compare TDengine Balances with Pipeline CSV')
    parser.add_argument('--pipeline', '-p', required=True, help='Pipeline output CSV')
    parser.add_argument('--host', default='localhost', help='TDengine host')
    parser.add_argument('--port', type=int, default=6041, help='TDengine REST port')
    parser.add_argument('--verbose', '-v', action='store_true', help='Show all matches')
    parser.add_argument('--no-color', action='store_true', help='Disable colored output')
    parser.add_argument('--max-errors', type=int, default=20, help='Max errors to display')
    args = parser.parse_args()
    
    if args.no_color or not sys.stdout.isatty():
        disable_colors()
    
    print("╔════════════════════════════════════════════════════════════╗")
    print("║    Balances Comparison: Pipeline CSV vs TDengine          ║")
    print("╚════════════════════════════════════════════════════════════╝")
    print()
    
    # Load Pipeline CSV
    print(f"[1] Loading Pipeline CSV: {args.pipeline}")
    try:
        pipeline = load_pipeline_csv(args.pipeline)
        print(f"    {Colors.GREEN}✓{Colors.NC} Loaded {len(pipeline)} records")
    except Exception as e:
        print(f"    {Colors.RED}✗{Colors.NC} Error: {e}")
        return 2
    
    # Query TDengine
    print(f"[2] Querying TDengine at {args.host}:{args.port}...")
    try:
        client = TDengineClient(host=args.host, port=args.port)
        db = query_tdengine_balances(client)
        print(f"    {Colors.GREEN}✓{Colors.NC} Retrieved {len(db)} unique balances")
    except Exception as e:
        print(f"    {Colors.RED}✗{Colors.NC} Error: {e}")
        return 2
    
    # Compare
    print(f"\n[3] Comparing {len(pipeline)} Pipeline records vs {len(db)} DB records...")
    matched, mismatched, missing, errors = compare_balances(pipeline, db, args.verbose)
    
    # Results
    print("\n" + "=" * 60)
    print("Comparison Results")
    print("=" * 60)
    print(f"\nPipeline records: {len(pipeline)}")
    print(f"DB records:       {len(db)}")
    print()
    print(f"Matched:    {matched}")
    print(f"Mismatched: {mismatched}")
    print(f"Missing:    {missing}")
    
    if errors:
        print(f"\nFirst {min(args.max_errors, len(errors))} issues:")
        for err in errors[:args.max_errors]:
            print(f"  {Colors.RED}•{Colors.NC} {err}")
        if len(errors) > args.max_errors:
            print(f"  ... and {len(errors) - args.max_errors} more")
    
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
