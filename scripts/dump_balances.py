#!/usr/bin/env python3
"""
dump_balances.py - Export TDengine Balances to CSV
===================================================

PURPOSE:
    Dump balance data from TDengine to CSV format for comparison.
    Queries via Gateway REST API or directly via TDEngine REST API.

OUTPUT FORMAT:
    user_id,asset_id,avail,frozen,lock_version,settle_version

USAGE:
    # Via Gateway API (default)
    python3 scripts/dump_balances.py --output db_balances.csv
    
    # Specify users/assets to query
    python3 scripts/dump_balances.py --users 0-100 --assets 1,2 --output db_balances.csv

EXIT CODES:
    0 = Success
    1 = Error
"""

import argparse
import csv
import json
import sys
import urllib.request
import urllib.error
from typing import List, Tuple, Optional, Dict

# Configuration
GATEWAY_URL = "http://localhost:8080"
TDENGINE_REST_URL = "http://localhost:6041/rest/sql"

def query_balance_via_gateway(user_id: int, asset_id: int) -> Optional[Dict]:
    """Query single balance via Gateway API."""
    url = f"{GATEWAY_URL}/api/v1/balances?user_id={user_id}&asset_id={asset_id}"
    
    try:
        req = urllib.request.Request(url)
        with urllib.request.urlopen(req, timeout=10) as response:
            data = json.loads(response.read().decode())
            
            if data.get('code') == 0 and data.get('data'):
                return data['data']
            return None
    except urllib.error.HTTPError as e:
        if e.code == 404:
            return None
        raise
    except Exception:
        return None


def query_balances_direct(user_ids: List[int], asset_ids: List[int]) -> List[Dict]:
    """
    Query balances directly from TDengine via REST API.
    Note: TDengine REST API format is POST with SQL in body.
    """
    balances = []
    
    for user_id in user_ids:
        for asset_id in asset_ids:
            table_name = f"balances_{user_id}_{asset_id}"
            sql = f"SELECT LAST(avail), LAST(frozen), LAST(lock_version), LAST(settle_version) FROM {table_name}"
            
            try:
                # TDengine REST API format: POST to /rest/sql/{db}
                url = "http://localhost:6041/rest/sql/exchange"
                data = sql.encode('utf-8')
                req = urllib.request.Request(url, data=data)
                req.add_header('Authorization', 'Basic cm9vdDp0YW9zZGF0YQ==')  # root:taosdata
                
                with urllib.request.urlopen(req, timeout=5) as response:
                    result = json.loads(response.read().decode())
                    
                    if result.get('code') == 0 and result.get('data'):
                        row = result['data'][0]
                        balances.append({
                            'user_id': user_id,
                            'asset_id': asset_id,
                            'avail': int(row[0]) if row[0] else 0,
                            'frozen': int(row[1]) if row[1] else 0,
                            'lock_version': int(row[2]) if row[2] else 0,
                            'settle_version': int(row[3]) if row[3] else 0,
                        })
            except urllib.error.HTTPError:
                pass  # Table may not exist
            except Exception:
                pass
    
    return balances


def parse_range(range_str: str) -> List[int]:
    """Parse range string like '0-100' or '1,2,3' to list of ints."""
    result = []
    for part in range_str.split(','):
        if '-' in part:
            start, end = part.split('-')
            result.extend(range(int(start), int(end) + 1))
        else:
            result.append(int(part))
    return result


def main():
    parser = argparse.ArgumentParser(description='Dump TDengine balances to CSV')
    parser.add_argument('--output', '-o', required=True, help='Output CSV file path')
    parser.add_argument('--users', default='0-100', help='User ID range (e.g., 0-100 or 1,2,3)')
    parser.add_argument('--assets', default='1,2', help='Asset IDs (e.g., 1,2)')
    parser.add_argument('--method', choices=['gateway', 'direct'], default='direct',
                        help='Query method: gateway (via API) or direct (TDengine REST)')
    parser.add_argument('--quiet', '-q', action='store_true', help='Suppress progress output')
    args = parser.parse_args()
    
    user_ids = parse_range(args.users)
    asset_ids = parse_range(args.assets)
    
    if not args.quiet:
        print(f"Dumping balances: {len(user_ids)} users × {len(asset_ids)} assets")
        print(f"Method: {args.method}")
    
    # Query balances
    if args.method == 'direct':
        balances = query_balances_direct(user_ids, asset_ids)
    else:
        balances = []
        total = len(user_ids) * len(asset_ids)
        count = 0
        for user_id in user_ids:
            for asset_id in asset_ids:
                count += 1
                if not args.quiet and count % 50 == 0:
                    print(f"  Progress: {count}/{total}", end='\r')
                
                data = query_balance_via_gateway(user_id, asset_id)
                if data:
                    balances.append({
                        'user_id': user_id,
                        'asset_id': asset_id,
                        'avail': data.get('avail'),
                        'frozen': data.get('frozen'),
                        'lock_version': data.get('lock_version'),
                        'settle_version': data.get('settle_version'),
                    })
        if not args.quiet:
            print()
    
    # Write CSV
    with open(args.output, 'w', newline='') as f:
        writer = csv.DictWriter(f, fieldnames=['user_id', 'asset_id', 'avail', 'frozen', 'lock_version', 'settle_version'])
        writer.writeheader()
        writer.writerows(balances)
    
    if not args.quiet:
        print(f"✅ Exported {len(balances)} records to {args.output}")
    
    return 0


if __name__ == "__main__":
    sys.exit(main())
